//! # Download Task Controller Module
//!
//! Orchestrates the multi-connection downloading process for an individual file.
//!
//! Responsibilities:
//! 1. Partitioning total file size into contiguous byte ranges (chunks).
//! 2. Distributing chunks across primary and mirror URLs.
//! 3. Spawning long-lived `ChunkWorker` instances with staggered jitter.
//! 4. Seamlessly falling back to single-stream downloading if server rejects HTTP 206.
//! 5. Throttling and rolling-average calculation for accurate UI speed (B/s) and ETA.
//! 6. Running post-download cryptographic and structural integrity verifications.

use std::path::PathBuf;
use std::time::{Duration, Instant};
use reqwest::Client;
use tokio::sync::{mpsc, watch};

use crate::models::download::{ChunkState, DownloadItem, FileType};
use crate::services::downloads::integrity;
use crate::services::downloads::worker::{ChunkWorker, WorkerEvent};
use crate::services::downloads::writer::PositionalWriter;

/// Events transmitted from the Task Controller back to the Download Engine.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TaskEvent {
    ProgressUpdated {
        id: usize,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_secs: Option<u64>,
        chunks: Vec<ChunkState>,
    },
    Completed {
        id: usize,
        downloaded_bytes: u64,
        sha256: Option<String>,
    },
    Failed {
        id: usize,
        error: String,
        downloaded_bytes: u64,
    },
    StatePersistRequested {
        item: DownloadItem,
    },
}

/// Coordinates all chunk workers for a single active download item.
pub struct DownloadTaskController {
    pub item: DownloadItem,
    pub client: Client,
    pub pause_rx: watch::Receiver<bool>,
    pub task_event_tx: mpsc::Sender<TaskEvent>,
}

impl DownloadTaskController {
    pub fn new(
        item: DownloadItem,
        client: Client,
        pause_rx: watch::Receiver<bool>,
        task_event_tx: mpsc::Sender<TaskEvent>,
    ) -> Self {
        Self {
            item,
            client,
            pause_rx,
            task_event_tx,
        }
    }

    /// Primary execution lifecycle for the download task.
    pub async fn run(mut self) {
        let save_path = PathBuf::from(&self.item.save_path);
        let target_file_path = save_path.join(&self.item.filename);

        // 1. Initialize Positional File Writer with disk pre-allocation
        let writer = match PositionalWriter::create_preallocated(&target_file_path, self.item.total_bytes) {
            Ok(w) => w,
            Err(e) => {
                let _ = self.task_event_tx.send(TaskEvent::Failed {
                    id: self.item.id,
                    error: format!("Failed to create output file {:?}: {}", target_file_path, e),
                    downloaded_bytes: self.item.downloaded_bytes,
                }).await;
                return;
            }
        };

        // 2. Partition chunks or reuse existing chunk state
        if self.item.chunks.is_empty() {
            self.item.chunks = self.partition_chunks();
        }

        // 3. Worker channels & execution loop
        let (worker_tx, mut worker_rx) = mpsc::channel::<WorkerEvent>(100);

        self.spawn_workers(&writer, &worker_tx);

        let mut last_ui_emit = Instant::now();
        let mut last_persist_emit = Instant::now();
        let mut speed_measuring_start = Instant::now();
        let mut bytes_since_last_measure = 0u64;
        let mut current_speed_bps = 0u64;

        loop {
            tokio::select! {
                _ = self.pause_rx.changed() => {
                    if *self.pause_rx.borrow() {
                        let _ = writer.sync_data();
                        self.item.downloaded_bytes = self.calculate_total_downloaded();
                        let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                            item: self.item.clone(),
                        }).await;
                        return;
                    }
                }
                event = worker_rx.recv() => {
                    match event {
                        Some(WorkerEvent::BytesDownloaded { chunk_id, count, current_offset }) => {
                            if let Some(chunk) = self.item.chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.current_offset = current_offset;
                            }
                            bytes_since_last_measure += count;

                            // Calculate rolling speed over a 500ms sliding interval
                            let elapsed = speed_measuring_start.elapsed();
                            if elapsed >= Duration::from_millis(500) {
                                let secs = elapsed.as_secs_f64();
                                if secs > 0.0 {
                                    current_speed_bps = (bytes_since_last_measure as f64 / secs) as u64;
                                }
                                bytes_since_last_measure = 0;
                                speed_measuring_start = Instant::now();
                            }

                            // Emit throttled UI updates (every ~200ms) to ensure smooth 60fps rendering
                            if last_ui_emit.elapsed() >= Duration::from_millis(200) {
                                let total_downloaded = self.calculate_total_downloaded();
                                self.item.downloaded_bytes = total_downloaded;

                                let eta_secs = if current_speed_bps > 0 {
                                    self.item.total_bytes.map(|total| {
                                        let remaining = total.saturating_sub(total_downloaded);
                                        remaining / current_speed_bps
                                    })
                                } else {
                                    None
                                };

                                let _ = self.task_event_tx.send(TaskEvent::ProgressUpdated {
                                    id: self.item.id,
                                    downloaded_bytes: total_downloaded,
                                    total_bytes: self.item.total_bytes,
                                    speed_bps: current_speed_bps,
                                    eta_secs,
                                    chunks: self.item.chunks.clone(),
                                }).await;
                                last_ui_emit = Instant::now();
                            }

                            // Periodic state persistence to JSON (every 3 seconds)
                            if last_persist_emit.elapsed() >= Duration::from_secs(3) {
                                self.item.downloaded_bytes = self.calculate_total_downloaded();
                                let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                                    item: self.item.clone(),
                                }).await;
                                last_persist_emit = Instant::now();
                            }
                        }
                        Some(WorkerEvent::ChunkCompleted { chunk_id }) => {
                            if let Some(chunk) = self.item.chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.is_completed = true;
                                chunk.current_offset = chunk.end_byte + 1;
                            }

                            // Check if all chunks have finished
                            if self.item.chunks.iter().all(|c| c.is_completed) {
                                let _ = writer.sync_data();
                                let total_downloaded = self.calculate_total_downloaded();
                                self.item.downloaded_bytes = total_downloaded;

                                // Perform post-download integrity verification
                                let (is_valid, computed_sha256) = self.verify_download_integrity(&target_file_path).await;

                                if is_valid {
                                    let _ = self.task_event_tx.send(TaskEvent::Completed {
                                        id: self.item.id,
                                        downloaded_bytes: total_downloaded,
                                        sha256: computed_sha256,
                                    }).await;
                                } else {
                                    let _ = self.task_event_tx.send(TaskEvent::Failed {
                                        id: self.item.id,
                                        error: "Post-download integrity check failed: file corrupted or truncated".to_string(),
                                        downloaded_bytes: total_downloaded,
                                    }).await;
                                }
                                return;
                            }
                        }
                        Some(WorkerEvent::FallbackToSingleStream { chunk_id: _ }) => {
                            // Server returned 200 OK: does not support Range requests!
                            // Fallback gracefully to single-stream download from byte 0.
                            println!("[QDM Task {}] Server rejected HTTP 206 Range. Falling back to single-stream.", self.item.id);
                            self.item.resumable = false;
                            self.item.chunks = vec![ChunkState::new(
                                0,
                                &self.item.primary_url.url,
                                0,
                                self.item.total_bytes.unwrap_or(u64::MAX),
                            )];
                            // Re-spawn single worker
                            self.spawn_workers(&writer, &worker_tx);
                        }
                        Some(WorkerEvent::ServerFileChanged { chunk_id: _, new_etag }) => {
                            let error_msg = format!(
                                "Server file modified remotely mid-download (ETag: {:?}). Fresh download required.",
                                new_etag
                            );
                            let _ = self.task_event_tx.send(TaskEvent::Failed {
                                id: self.item.id,
                                error: error_msg,
                                downloaded_bytes: self.item.downloaded_bytes,
                            }).await;
                            return;
                        }
                        Some(WorkerEvent::RateLimited { chunk_id, retry_after }) => {
                            println!(
                                "[QDM Task {}] Worker {} received HTTP 429. Backing off for {:?}",
                                self.item.id, chunk_id, retry_after
                            );
                        }
                        Some(WorkerEvent::WorkerFailed { chunk_id, error }) => {
                            let error_msg = format!("Worker {} encountered fatal error: {}", chunk_id, error);
                            let _ = self.task_event_tx.send(TaskEvent::Failed {
                                id: self.item.id,
                                error: error_msg,
                                downloaded_bytes: self.calculate_total_downloaded(),
                            }).await;
                            return;
                        }
                        None => {
                            // Channel closed
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Partitions the total file size into contiguous byte ranges distributed across available mirrors.
    fn partition_chunks(&self) -> Vec<ChunkState> {
        let total = match self.item.total_bytes {
            Some(t) if t > 0 => t,
            _ => return vec![ChunkState::new(0, &self.item.primary_url.url, 0, u64::MAX)],
        };

        // Don't chunk small files (< 1MB) or non-resumable servers
        if !self.item.resumable || total < 1024 * 1024 {
            return vec![ChunkState::new(0, &self.item.primary_url.url, 0, total - 1)];
        }

        let num_connections = (self.item.max_connections as usize).clamp(1, 16);
        let mut chunks = Vec::with_capacity(num_connections);

        // Gather all active URLs (primary + active mirrors) for mirror distribution
        let mut available_urls = vec![self.item.primary_url.url.clone()];
        for mirror in &self.item.mirror_urls {
            if mirror.is_active {
                available_urls.push(mirror.url.clone());
            }
        }

        let chunk_size = total / (num_connections as u64);
        for i in 0..num_connections {
            let start_byte = (i as u64) * chunk_size;
            let end_byte = if i == num_connections - 1 {
                total - 1
            } else {
                ((i + 1) as u64) * chunk_size - 1
            };

            // Distribute URLs across chunks round-robin
            let url = available_urls[i % available_urls.len()].clone();
            chunks.push(ChunkState::new(i, url, start_byte, end_byte));
        }

        chunks
    }

    /// Spawns Tokio tasks for uncompleted chunks with staggered jitter delay.
    fn spawn_workers(&self, writer: &PositionalWriter, worker_tx: &mpsc::Sender<WorkerEvent>) {
        for (i, chunk) in self.item.chunks.iter().enumerate() {
            if chunk.is_completed || chunk.current_offset > chunk.end_byte {
                continue;
            }

            let worker = ChunkWorker::new(
                chunk.clone(),
                self.client.clone(),
                writer.clone(),
                self.item.etag.clone(),
                self.item.last_modified.clone(),
                worker_tx.clone(),
                self.pause_rx.clone(),
            );

            // Stagger spawning by 60ms * worker_index to eliminate synchronized connection spikes
            let spawn_delay = Duration::from_millis(60 * (i as u64));

            tokio::spawn(async move {
                worker.run(spawn_delay).await;
            });
        }
    }

    fn calculate_total_downloaded(&self) -> u64 {
        self.item.chunks.iter().map(|c| c.downloaded_bytes()).sum()
    }

    /// Validates file integrity upon download completion.
    async fn verify_download_integrity(&self, file_path: &PathBuf) -> (bool, Option<String>) {
        let mut computed_sha256 = None;

        // 1. If an expected SHA-256 hash was configured, verify it
        if let Some(ref expected_hash) = self.item.sha256_hash {
            match integrity::verify_sha256(file_path, expected_hash).await {
                Ok(true) => {
                    computed_sha256 = Some(expected_hash.clone());
                }
                Ok(false) => {
                    println!("[QDM Integrity] SHA-256 hash mismatch for {:?}", file_path);
                    return (false, None);
                }
                Err(err) => {
                    println!("[QDM Integrity] SHA-256 check error: {}", err);
                    return (false, None);
                }
            }
        }

        // 2. Structural verification (e.g. ZIP EOCD header)
        if self.item.file_type == FileType::Archive {
            if let Ok(is_valid_structure) = integrity::verify_structure(file_path, self.item.file_type).await {
                if !is_valid_structure {
                    println!("[QDM Integrity] Archive structure check failed for {:?}", file_path);
                    return (false, None);
                }
            }
        }

        (true, computed_sha256)
    }
}
