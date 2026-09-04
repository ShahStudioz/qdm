use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use librqbit::{
    Session, AddTorrent, AddTorrentOptions, AddTorrentResponse,
    ManagedTorrent,
    api::TorrentIdOrHash,
};

use crate::models::download::{DownloadItem, DownloadState};
use crate::services::http::engine::EngineUiEvent;

/// The internal Torrent engine wrapping librqbit
pub struct TorrentEngine {
    session: Arc<Session>,
    event_tx: broadcast::Sender<EngineUiEvent>,
    handles: Arc<RwLock<HashMap<usize, (usize, Arc<ManagedTorrent>, tokio::task::JoinHandle<()>)>>>,
}

impl TorrentEngine {
    pub async fn new(
        default_download_dir: PathBuf,
        event_tx: broadcast::Sender<EngineUiEvent>,
    ) -> Result<Self, String> {
        let session = Session::new(default_download_dir)
            .await
            .map_err(|e| format!("Failed to initialize librqbit Session: {}", e))?;

        Ok(Self {
            session,
            event_tx,
            handles: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineUiEvent> {
        self.event_tx.subscribe()
    }

    /// Resolves a magnet link or .torrent URL to fetch its metadata and file tree
    /// without starting the download.
    pub async fn probe_metadata(&self, magnet: &str) -> Result<AddTorrentResponse, String> {
        let opts = AddTorrentOptions {
            list_only: true,
            overwrite: false,
            ..Default::default()
        };

        let response = self
            .session
            .add_torrent(AddTorrent::Url(magnet.into()), Some(opts))
            .await
            .map_err(|e| e.to_string())?;

        Ok(response)
    }

    /// Starts or resumes a torrent download.
    pub async fn start_or_resume(&self, item: &DownloadItem, play_media: bool) -> Result<(), String> {
        let torrent_meta = item
            .torrent_meta()
            .ok_or_else(|| "Item is not a torrent".to_string())?;

        let item_id = item.id;

        // If the torrent is already managed, unpause it and spawn a new monitor if needed
        {
            let mut handles_guard = self.handles.write().await;
            if let Some((_, handle, monitor_task)) = handles_guard.get_mut(&item_id) {
                let _ = self.session.unpause(handle).await;
                monitor_task.abort();

                let handle_clone = handle.clone();
                let event_tx = self.event_tx.clone();
                let initial_downloaded = item.downloaded_bytes;
                let new_monitor = tokio::spawn(async move {
                    Self::run_monitor_loop(item_id, handle_clone, event_tx, initial_downloaded).await;
                });

                *monitor_task = new_monitor;
                let _ = self.event_tx.send(EngineUiEvent::StateChanged {
                    id: item_id,
                    state: DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    },
                });
                return Ok(());
            }
        }

        let use_staging = !(play_media && item.file_type == crate::models::download::FileType::Media);
        
        let staging_base = std::path::PathBuf::from(&item.save_path).join(format!(".qdmdownload_{}", item.id));
        let final_base = std::path::PathBuf::from(&item.save_path);
        
        let staging_target = staging_base.join(&item.filename);
        let final_target = final_base.join(&item.filename);
        
        // Dynamically move files between staging and final destinations if setting changed
        if use_staging {
            if final_target.exists() && !staging_base.exists() {
                let _ = std::fs::create_dir_all(&staging_base);
                let _ = std::fs::rename(&final_target, &staging_target);
            }
        } else {
            if staging_base.exists() && staging_target.exists() {
                if final_target.exists() {
                    if final_target.is_dir() {
                        let _ = std::fs::remove_dir_all(&final_target);
                    } else {
                        let _ = std::fs::remove_file(&final_target);
                    }
                }
                let _ = std::fs::rename(&staging_target, &final_target);
                let _ = std::fs::remove_dir_all(&staging_base);
            }
        }

        let output_folder = if use_staging {
            if torrent_meta.is_folder {
                staging_target
            } else {
                staging_base
            }
        } else {
            if torrent_meta.is_folder {
                final_target
            } else {
                final_base
            }
        };

        let opts = AddTorrentOptions {
            output_folder: Some(output_folder.to_string_lossy().to_string()),
            overwrite: true,
            only_files: torrent_meta.selected_files.clone(),
            ..Default::default()
        };

        let add_torrent = AddTorrent::Url(torrent_meta.magnet_uri.clone().into());
        let response = self
            .session
            .add_torrent(add_torrent, Some(opts))
            .await
            .map_err(|e| {
                println!("[QDM Torrent] add_torrent failed for item {}: {}", item_id, e);
                e.to_string()
            })?;

        match response {
            AddTorrentResponse::Added(torrent_id, handle) => {
                let handle_clone = handle.clone();
                let event_tx = self.event_tx.clone();
                let initial_downloaded = item.downloaded_bytes;
                let monitor_task = tokio::spawn(async move {
                    Self::run_monitor_loop(item_id, handle_clone, event_tx, initial_downloaded).await;
                });

                self.handles
                    .write()
                    .await
                    .insert(item_id, (torrent_id, handle, monitor_task));

                let _ = self.event_tx.send(EngineUiEvent::StateChanged {
                    id: item_id,
                    state: DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    },
                });
            }
            AddTorrentResponse::AlreadyManaged(torrent_id, handle) => {
                // The torrent was already in the librqbit session (from persistence).
                // It's likely paused/stopped — we must explicitly unpause it.
                let _ = self.session.unpause(&handle).await;

                let handle_clone = handle.clone();
                let event_tx = self.event_tx.clone();
                let initial_downloaded = item.downloaded_bytes;
                let monitor_task = tokio::spawn(async move {
                    Self::run_monitor_loop(item_id, handle_clone, event_tx, initial_downloaded).await;
                });

                self.handles
                    .write()
                    .await
                    .insert(item_id, (torrent_id, handle, monitor_task));

                let _ = self.event_tx.send(EngineUiEvent::StateChanged {
                    id: item_id,
                    state: DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    },
                });
            }
            _ => {
                return Err("Unexpected response when adding torrent".to_string());
            }
        }

        Ok(())
    }

    /// Background polling loop for active torrent statistics.
    async fn run_monitor_loop(
        item_id: usize,
        handle: Arc<ManagedTorrent>,
        event_tx: broadcast::Sender<EngineUiEvent>,
        initial_downloaded_bytes: u64,
    ) {
        let mut last_fetched_bytes = 0;
        let mut smooth_downloaded_bytes = initial_downloaded_bytes;
        
        let mut speed_meter = crate::services::http::task::SlidingSpeedMeter::new(
            std::time::Duration::from_millis(2500)
        );
        let mut upload_speed_meter = crate::services::http::task::SlidingSpeedMeter::new(
            std::time::Duration::from_millis(2500)
        );
        let mut last_uploaded_bytes = 0;
        let mut has_gone_live = false; // tracks if we've completed hash verification

        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(250));
        loop {
            interval.tick().await;

            let stats = handle.stats();
            
            // Detect the hash verification phase:
            // stats.live is None while librqbit is checking existing file integrity.
            // During this phase, progress_bytes climbs 0→total as pieces are verified,
            // then drops to the actual verified amount once checking finishes.
            if stats.live.is_none() && stats.total_bytes > 0 {
                // We're in the checking phase — show verification progress
                let check_pct = ((stats.progress_bytes as f64 / stats.total_bytes as f64) * 100.0).min(100.0) as u8;
                let _ = event_tx.send(EngineUiEvent::StateChanged {
                    id: item_id,
                    state: DownloadState::Checking {
                        progress_pct: check_pct,
                        downloaded_bytes: initial_downloaded_bytes,
                        total_bytes: Some(stats.total_bytes),
                    },
                });
                continue; // Don't update smooth_downloaded_bytes during check
            }

            if stats.live.is_some() && !has_gone_live {
                // First time going live after hash check — reset to verified value
                has_gone_live = true;
                smooth_downloaded_bytes = stats.progress_bytes;
                last_fetched_bytes = 0;
                last_uploaded_bytes = 0;
                // Transition back to Downloading state
                let _ = event_tx.send(EngineUiEvent::StateChanged {
                    id: item_id,
                    state: DownloadState::Downloading {
                        downloaded_bytes: stats.progress_bytes,
                        total_bytes: if stats.total_bytes > 0 { Some(stats.total_bytes) } else { None },
                        speed_bps: 0,
                        eta_secs: None,
                    },
                });
            }

            // Normal progress tracking (only when live)
            let progress = stats.progress_bytes;
            if smooth_downloaded_bytes > progress && smooth_downloaded_bytes - progress > 10 * 1024 * 1024 {
                smooth_downloaded_bytes = progress;
            } else {
                smooth_downloaded_bytes = smooth_downloaded_bytes.max(progress);
            }
            
            if let Some(live) = &stats.live {
                let fetched = live.snapshot.fetched_bytes;
                if fetched > last_fetched_bytes {
                    let delta = fetched - last_fetched_bytes;
                    smooth_downloaded_bytes += delta;
                    last_fetched_bytes = fetched;
                    speed_meter.record_bytes(delta);
                }
                
                let uploaded = live.snapshot.uploaded_bytes;
                if uploaded > last_uploaded_bytes {
                    let delta = uploaded - last_uploaded_bytes;
                    last_uploaded_bytes = uploaded;
                    upload_speed_meter.record_bytes(delta);
                }
            }
            
            let downloaded_bytes = if stats.total_bytes > 0 {
                smooth_downloaded_bytes.min(stats.total_bytes)
            } else {
                smooth_downloaded_bytes
            };

            let total_bytes = if stats.total_bytes > 0 {
                Some(stats.total_bytes)
            } else {
                None
            };

            let (speed_bps, upload_speed_bps, peers, seeds, eta_secs) = if let Some(live) = stats.live {
                let down_speed = speed_meter.calculate_speed_bps();
                let up_speed = upload_speed_meter.calculate_speed_bps();
                let peers = live.snapshot.peer_stats.live;
                let seeds = live.snapshot.peer_stats.seen;
                let eta = if down_speed > 0 && total_bytes.is_some() && total_bytes.unwrap() > downloaded_bytes {
                    Some((total_bytes.unwrap() - downloaded_bytes) / down_speed)
                } else {
                    None
                };
                (down_speed, up_speed, peers, seeds, eta)
            } else {
                (0, 0, 0, 0, None)
            };

            if let Some(err) = stats.error {
                let _ = event_tx.send(EngineUiEvent::DownloadFailed {
                    id: item_id,
                    error: err,
                });
                break;
            }
            // Completion check: only trust the hash-verified progress_bytes from librqbit,
            // NOT our smooth UI counter. stats.finished can briefly be true from stale
            // fastresume data before hash check corrects it, so also verify progress.
            let truly_finished = stats.finished 
                && stats.total_bytes > 0 
                && stats.progress_bytes >= stats.total_bytes;
            
            if truly_finished {
                let _ = event_tx.send(EngineUiEvent::TorrentProgressUpdated {
                    id: item_id,
                    downloaded_bytes: total_bytes.unwrap_or(downloaded_bytes),
                    total_bytes,
                    speed_bps: 0,
                    upload_speed_bps: 0,
                    peers,
                    seeds,
                    eta_secs: Some(0),
                });
                let _ = event_tx.send(EngineUiEvent::DownloadCompleted {
                    id: item_id,
                    sha256: None,
                });
                break;
            }

            let _ = event_tx.send(EngineUiEvent::TorrentProgressUpdated {
                id: item_id,
                downloaded_bytes,
                total_bytes,
                speed_bps,
                upload_speed_bps,
                peers,
                seeds,
                eta_secs,
            });
        }
    }

    /// Pauses an active torrent.
    pub async fn pause(&self, id: usize) {
        let handles_guard = self.handles.read().await;
        if let Some((_, handle, monitor_task)) = handles_guard.get(&id) {
            monitor_task.abort();
            let _ = self.session.pause(handle).await;
            let stats = handle.stats();
            let _ = self.event_tx.send(EngineUiEvent::StateChanged {
                id,
                state: DownloadState::Paused {
                    downloaded_bytes: stats.progress_bytes,
                    total_bytes: if stats.total_bytes > 0 { Some(stats.total_bytes) } else { None },
                },
            });
        }
    }

    /// Cancels and removes a torrent.
    pub async fn cancel(&self, id: usize) {
        let mut handles_guard = self.handles.write().await;
        if let Some((torrent_id, _, monitor_task)) = handles_guard.remove(&id) {
            monitor_task.abort();
            let _ = self.session.delete(TorrentIdOrHash::Id(torrent_id), false).await;
        }
    }

    /// Updates global torrent speed limit.
    pub async fn update_speed_limit(&self, _limit: Option<u64>) {
        // librqbit speed limits can be added here if needed
    }
}
