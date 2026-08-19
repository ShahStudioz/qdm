//! # Core Download Engine Module
//!
//! Provides the primary orchestrator for the Quick Download Manager.
//!
//! Architectural Highlights:
//! 1. **Single Connection Pool**: Initializes a single `reqwest::Client` with tuned TCP keep-alive,
//!    HTTP/2 multiplexing, and idle connection recycling across all background tasks.
//! 2. **Task Registry**: Tracks running downloads and enables non-blocking, asynchronous start,
//!    pause, resume, and cancellation.
//! 3. **Broadcast UI Event Stream**: Dispatches throttled `EngineUiEvent`s directly to the UI
//!    via `tokio::sync::broadcast` without blocking any worker threads.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use reqwest::Client;
use tokio::sync::{broadcast, mpsc, watch, Mutex};

use crate::models::download::{ChunkState, DownloadItem, DownloadState};
use crate::services::downloads::metadata::{FileMetadata, MetadataService};
use crate::services::downloads::task::{DownloadTaskController, TaskEvent};

/// High-level events emitted by the Download Engine for UI presentation.
#[derive(Debug, Clone)]
pub enum EngineUiEvent {
    ProgressUpdated {
        id: usize,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_secs: Option<u64>,
        chunks: Vec<ChunkState>,
    },
    StateChanged {
        id: usize,
        state: DownloadState,
    },
    DownloadCompleted {
        id: usize,
        sha256: Option<String>,
    },
    DownloadFailed {
        id: usize,
        error: String,
    },
    PersistRequested {
        item: DownloadItem,
    },
}

/// The centralized Download Engine managing all concurrent download tasks and connections.
#[derive(Clone)]
pub struct DownloadEngine {
    client: Client,
    metadata_service: MetadataService,
    active_tasks: Arc<Mutex<HashMap<usize, watch::Sender<bool>>>>,
    ui_event_tx: broadcast::Sender<EngineUiEvent>,
}

impl Default for DownloadEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadEngine {
    /// Creates and initializes the `DownloadEngine` with a shared HTTP connection pool.
    pub fn new() -> Self {
        let client = Client::builder()
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_keepalive(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .user_agent("QDM/0.1.0 (Quick Download Manager; Windows NT 10.0; Win64; x64)")
            .build()
            .unwrap_or_else(|_| Client::new());

        let metadata_service = MetadataService::new(client.clone());
        let (ui_event_tx, _) = broadcast::channel(512);

        Self {
            client,
            metadata_service,
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            ui_event_tx,
        }
    }

    /// Subscribes to the live engine event stream for Iced UI consumption.
    pub fn subscribe(&self) -> broadcast::Receiver<EngineUiEvent> {
        self.ui_event_tx.subscribe()
    }

    /// Queries remote URL headers to retrieve file size, ETag, and resumability metadata.
    pub async fn probe_metadata(&self, url: &str) -> Result<FileMetadata, String> {
        self.metadata_service.probe(url).await
    }

    /// Spawns or resumes a background download task for the given item.
    pub async fn start_or_resume(&self, item: DownloadItem) {
        let item_id = item.id;

        let (pause_tx, pause_rx) = watch::channel(false);
        {
            let mut tasks = self.active_tasks.lock().await;
            if let Some(prev_pause_tx) = tasks.remove(&item_id) {
                let _ = prev_pause_tx.send(true);
            }
            tasks.insert(item_id, pause_tx);
        }

        let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(100);
        let client = self.client.clone();
        let ui_event_tx = self.ui_event_tx.clone();
        let active_tasks = self.active_tasks.clone();

        // Notify UI that download state has transitioned to Downloading
        let _ = ui_event_tx.send(EngineUiEvent::StateChanged {
            id: item_id,
            state: DownloadState::Downloading {
                downloaded_bytes: item.downloaded_bytes,
                total_bytes: item.total_bytes,
                speed_bps: 0,
                eta_secs: None,
            },
        });

        // Spawn task controller on Tokio async runtime
        tokio::spawn(async move {
            let controller = DownloadTaskController::new(item, client, pause_rx, task_event_tx);

            // Spawn the controller runner
            tokio::spawn(async move {
                controller.run().await;
            });

            // Forward task events to UI broadcast channel and handle task cleanup
            while let Some(task_event) = task_event_rx.recv().await {
                match task_event {
                    TaskEvent::ProgressUpdated {
                        id,
                        downloaded_bytes,
                        total_bytes,
                        speed_bps,
                        eta_secs,
                        chunks,
                    } => {
                        let _ = ui_event_tx.send(EngineUiEvent::ProgressUpdated {
                            id,
                            downloaded_bytes,
                            total_bytes,
                            speed_bps,
                            eta_secs,
                            chunks,
                        });
                    }
                    TaskEvent::Completed {
                        id,
                        downloaded_bytes: _,
                        sha256,
                    } => {
                        {
                            let mut tasks = active_tasks.lock().await;
                            tasks.remove(&id);
                        }
                        let _ = ui_event_tx.send(EngineUiEvent::StateChanged {
                            id,
                            state: DownloadState::Completed,
                        });
                        let _ = ui_event_tx.send(EngineUiEvent::DownloadCompleted { id, sha256 });
                        break;
                    }
                    TaskEvent::WaitingForNetwork {
                        id,
                        downloaded_bytes,
                        total_bytes,
                    } => {
                        {
                            let mut tasks = active_tasks.lock().await;
                            tasks.remove(&id);
                        }
                        let _ = ui_event_tx.send(EngineUiEvent::StateChanged {
                            id,
                            state: DownloadState::WaitingForNetwork {
                                downloaded_bytes,
                                total_bytes,
                            },
                        });
                        break;
                    }
                    TaskEvent::Failed {
                        id,
                        error,
                        downloaded_bytes,
                    } => {
                        {
                            let mut tasks = active_tasks.lock().await;
                            tasks.remove(&id);
                        }
                        let _ = ui_event_tx.send(EngineUiEvent::StateChanged {
                            id,
                            state: DownloadState::Failed {
                                downloaded_bytes,
                                total_bytes: None,
                                error: error.clone(),
                            },
                        });
                        let _ = ui_event_tx.send(EngineUiEvent::DownloadFailed { id, error });
                        break;
                    }
                    TaskEvent::StatePersistRequested { item } => {
                        let _ = ui_event_tx.send(EngineUiEvent::PersistRequested { item });
                    }
                }
            }
        });
    }

    /// Pauses an active download task by notifying its worker pause watch channel.
    pub async fn pause(&self, id: usize) {
        let mut tasks = self.active_tasks.lock().await;
        if let Some(pause_tx) = tasks.remove(&id) {
            let _ = pause_tx.send(true);
        }
    }

    /// Cancels a download task and removes it from active registry.
    pub async fn cancel(&self, id: usize) {
        let mut tasks = self.active_tasks.lock().await;
        if let Some(pause_tx) = tasks.remove(&id) {
            let _ = pause_tx.send(true);
        }
    }

    /// Checks if a task is currently active and running.
    #[allow(dead_code)]
    pub async fn is_running(&self, id: usize) -> bool {
        let tasks = self.active_tasks.lock().await;
        tasks.contains_key(&id)
    }
}
