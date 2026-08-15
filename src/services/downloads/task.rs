//! # Download Task Controller Module
//!
//! Orchestrates the adaptive multi-connection downloading process for an individual file.
//!
//! Advanced Architectural Features:
//! 1. **Adaptive Dynamic Concurrency**: Automatically probes and auto-tunes the optimal number of
//!    parallel connections (starts with 2 safe connections, auto-scales up if server allows).
//! 2. **Instant Anti-429 Rate-Limit Throttling**: If a server enforces per-IP concurrency limits
//!    (HTTP 429), immediately caps concurrency to the server's ceiling without spamming retries.
//! 3. **Dynamic Work-Stealing**: Idle workers automatically split and take over uncompleted ranges
//!    from busy workers, guaranteeing full bandwidth saturation and preventing speed drops to 0.
//! 4. **Exponential Moving Average (EMA) Speed Smoothing**: Prevents abrupt speed jumps or drops
//!    during chunk handoffs or TCP buffer adjustments.
//! 5. **Strict 206 Fallback & Remote File Integrity Checks**: Handles single-stream fallbacks and
//!    validates cryptographic SHA-256 and ZIP EOCD structure upon completion.

use std::collections::HashSet;
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
    active_workers: HashSet<usize>,
    max_allowed_concurrency: usize,
    current_target_concurrency: usize,
    next_scale_up_allowed: Instant,
}

impl DownloadTaskController {
    pub fn new(
        item: DownloadItem,
        client: Client,
        pause_rx: watch::Receiver<bool>,
        task_event_tx: mpsc::Sender<TaskEvent>,
    ) -> Self {
        let max_concurrency = (item.max_connections as usize).clamp(1, 16);
        // Start conservatively with 2 connections (or 1 if non-resumable)
        let initial_target = if item.resumable && item.total_bytes.map(|t| t > 1024 * 1024).unwrap_or(false) {
            2.min(max_concurrency)
        } else {
            1
        };

        Self {
            item,
            client,
            pause_rx,
            task_event_tx,
            active_workers: HashSet::new(),
            max_allowed_concurrency: max_concurrency,
            current_target_concurrency: initial_target,
            next_scale_up_allowed: Instant::now() + Duration::from_secs(3),
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
            self.item.chunks = self.partition_initial_chunks();
        }

        // 3. Worker channels & execution loop
        let (worker_tx, mut worker_rx) = mpsc::channel::<WorkerEvent>(100);

        // Spawn initial safe workers
        self.spawn_available_work(&writer, &worker_tx);

        let mut last_ui_emit = Instant::now();
        let mut last_persist_emit = Instant::now();
        let mut speed_measuring_start = Instant::now();
        let mut bytes_since_last_measure = 0u64;
        let mut smoothed_speed_bps = 0.0f64;

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
                // Periodic maintenance tick for adaptive concurrency scale-up
                _ = tokio::time::sleep(Duration::from_millis(250)) => {
                    // Try to adaptively scale up connections if everything is performing smoothly
                    if self.item.resumable
                        && self.active_workers.len() == self.current_target_concurrency
                        && self.current_target_concurrency < self.max_allowed_concurrency
                        && Instant::now() >= self.next_scale_up_allowed
                    {
                        self.current_target_concurrency += 1;
                        self.next_scale_up_allowed = Instant::now() + Duration::from_secs(4);
                        self.spawn_available_work(&writer, &worker_tx);
                    }

                    // Update EMA speed calculation
                    let elapsed = speed_measuring_start.elapsed();
                    if elapsed >= Duration::from_millis(250) {
                        let secs = elapsed.as_secs_f64();
                        if secs > 0.0 {
                            let sample_bps = (bytes_since_last_measure as f64) / secs;
                            if bytes_since_last_measure > 0 {
                                // Smooth EMA filter
                                smoothed_speed_bps = 0.35 * sample_bps + 0.65 * smoothed_speed_bps;
                            } else {
                                // Graceful decay on network idle
                                smoothed_speed_bps *= 0.75;
                                if smoothed_speed_bps < 512.0 {
                                    smoothed_speed_bps = 0.0;
                                }
                            }
                        }
                        bytes_since_last_measure = 0;
                        speed_measuring_start = Instant::now();
                    }

                    // Emit progress to UI every 200ms
                    if last_ui_emit.elapsed() >= Duration::from_millis(200) {
                        let total_downloaded = self.calculate_total_downloaded();
                        self.item.downloaded_bytes = total_downloaded;
                        let cur_speed = smoothed_speed_bps as u64;

                        let eta_secs = if cur_speed > 0 {
                            self.item.total_bytes.map(|total| {
                                let remaining = total.saturating_sub(total_downloaded);
                                remaining / cur_speed
                            })
                        } else {
                            None
                        };

                        let _ = self.task_event_tx.send(TaskEvent::ProgressUpdated {
                            id: self.item.id,
                            downloaded_bytes: total_downloaded,
                            total_bytes: self.item.total_bytes,
                            speed_bps: cur_speed,
                            eta_secs,
                            chunks: self.item.chunks.clone(),
                        }).await;
                        last_ui_emit = Instant::now();
                    }

                    // Periodic state persistence to JSON
                    if last_persist_emit.elapsed() >= Duration::from_secs(3) {
                        self.item.downloaded_bytes = self.calculate_total_downloaded();
                        let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                            item: self.item.clone(),
                        }).await;
                        last_persist_emit = Instant::now();
                    }
                }
                event = worker_rx.recv() => {
                    match event {
                        Some(WorkerEvent::BytesDownloaded { chunk_id, count, current_offset }) => {
                            if let Some(chunk) = self.item.chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.current_offset = current_offset;
                            }
                            bytes_since_last_measure += count;
                        }
                        Some(WorkerEvent::ChunkCompleted { chunk_id }) => {
                            if let Some(chunk) = self.item.chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.is_completed = true;
                                chunk.current_offset = chunk.end_byte + 1;
                            }
                            self.active_workers.remove(&chunk_id);

                            // Check if all chunks have finished
                            if self.item.chunks.iter().all(|c| c.is_completed) {
                                let _ = writer.sync_data();
                                let total_downloaded = self.calculate_total_downloaded();
                                self.item.downloaded_bytes = total_downloaded;

                                // Post-download integrity verification
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

                            // Keep workers busy: dispatch uncompleted chunk or work-steal
                            self.spawn_available_work(&writer, &worker_tx);
                        }
                        Some(WorkerEvent::RateLimited { chunk_id, retry_after: _ }) => {
                            self.active_workers.remove(&chunk_id);

                            // Smart Concurrency Cap: Lock max concurrency to the active healthy count
                            self.max_allowed_concurrency = self.active_workers.len().max(1);
                            self.current_target_concurrency = self.max_allowed_concurrency;
                            // Block any further scale up for at least 60 seconds
                            self.next_scale_up_allowed = Instant::now() + Duration::from_secs(60);

                            println!(
                                "[QDM Task {}] HTTP 429 rate limit encountered on worker {}. Smartly capped concurrency to {} active connections.",
                                self.item.id, chunk_id, self.max_allowed_concurrency
                            );

                            // If no workers are left running, restart 1 connection after a cooldown
                            if self.active_workers.is_empty() {
                                let mut pause_rx = self.pause_rx.clone();
                                let worker_tx_clone = worker_tx.clone();

                                tokio::spawn(async move {
                                    tokio::select! {
                                        _ = tokio::time::sleep(Duration::from_secs(2)) => {
                                            let _ = worker_tx_clone.send(WorkerEvent::BytesDownloaded { chunk_id: 0, count: 0, current_offset: 0 }).await;
                                        }
                                        _ = pause_rx.changed() => {}
                                    }
                                });
                                self.spawn_available_work(&writer, &worker_tx);
                            }
                        }
                        Some(WorkerEvent::FallbackToSingleStream { chunk_id: _ }) => {
                            // Server returned 200 OK: does not support Range requests!
                            // Fallback gracefully to single-stream download from byte 0.
                            println!("[QDM Task {}] Server rejected HTTP 206 Range. Falling back to single-stream.", self.item.id);
                            self.item.resumable = false;
                            self.max_allowed_concurrency = 1;
                            self.current_target_concurrency = 1;
                            self.active_workers.clear();
                            self.item.chunks = vec![ChunkState::new(
                                0,
                                &self.item.primary_url.url,
                                0,
                                self.item.total_bytes.unwrap_or(u64::MAX),
                            )];
                            self.spawn_available_work(&writer, &worker_tx);
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
                        Some(WorkerEvent::WorkerFailed { chunk_id, error }) => {
                            self.active_workers.remove(&chunk_id);
                            println!("[QDM Task {}] Worker {} stopped: {}", self.item.id, chunk_id, error);

                            // If all workers died, report failure
                            if self.active_workers.is_empty() && self.item.chunks.iter().all(|c| !c.is_completed) {
                                let _ = self.task_event_tx.send(TaskEvent::Failed {
                                    id: self.item.id,
                                    error,
                                    downloaded_bytes: self.calculate_total_downloaded(),
                                }).await;
                                return;
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Partitions the initial file size across chunks.
    fn partition_initial_chunks(&self) -> Vec<ChunkState> {
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

            let url = available_urls[i % available_urls.len()].clone();
            chunks.push(ChunkState::new(i, url, start_byte, end_byte));
        }

        chunks
    }

    /// Spawns workers for unassigned chunks or performs work-stealing from busy workers.
    fn spawn_available_work(&mut self, writer: &PositionalWriter, worker_tx: &mpsc::Sender<WorkerEvent>) {
        while self.active_workers.len() < self.current_target_concurrency {
            // 1. Check for an uncompleted chunk that is NOT currently active
            let next_chunk_idx = self.item.chunks.iter().position(|c| {
                !c.is_completed && c.current_offset <= c.end_byte && !self.active_workers.contains(&c.id)
            });

            if let Some(idx) = next_chunk_idx {
                let chunk = self.item.chunks[idx].clone();
                self.active_workers.insert(chunk.id);

                let worker = ChunkWorker::new(
                    chunk,
                    self.client.clone(),
                    writer.clone(),
                    self.item.etag.clone(),
                    self.item.last_modified.clone(),
                    worker_tx.clone(),
                    self.pause_rx.clone(),
                );

                tokio::spawn(async move {
                    worker.run(Duration::ZERO).await;
                });
            } else {
                // 2. Work-Stealing: Find the active chunk index with the largest remaining byte range (> 2MB)
                let largest_idx = self
                    .item
                    .chunks
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| !c.is_completed && c.end_byte > c.current_offset)
                    .max_by_key(|(_, c)| c.end_byte - c.current_offset)
                    .map(|(idx, _)| idx);

                if let Some(idx) = largest_idx {
                    let busy_chunk = &mut self.item.chunks[idx];
                    let remaining = busy_chunk.end_byte - busy_chunk.current_offset;
                    const MIN_STEAL_SIZE: u64 = 2 * 1024 * 1024; // Minimum 2MB to warrant a split

                    if remaining >= MIN_STEAL_SIZE {
                        let midpoint = busy_chunk.current_offset + (remaining / 2);
                        let stolen_start = midpoint + 1;
                        let stolen_end = busy_chunk.end_byte;
                        let url = busy_chunk.url.clone();

                        // Truncate existing chunk's end
                        busy_chunk.end_byte = midpoint;

                        let new_id = self.item.chunks.len();
                        let new_chunk = ChunkState::new(new_id, url, stolen_start, stolen_end);
                        self.item.chunks.push(new_chunk.clone());
                        self.active_workers.insert(new_id);

                        let worker = ChunkWorker::new(
                            new_chunk,
                            self.client.clone(),
                            writer.clone(),
                            self.item.etag.clone(),
                            self.item.last_modified.clone(),
                            worker_tx.clone(),
                            self.pause_rx.clone(),
                        );

                        tokio::spawn(async move {
                            worker.run(Duration::ZERO).await;
                        });
                        continue;
                    }
                }

                // No more stealable work
                break;
            }
        }
    }

    fn calculate_total_downloaded(&self) -> u64 {
        self.item.chunks.iter().map(|c| c.downloaded_bytes()).sum()
    }

    /// Validates file integrity upon download completion.
    async fn verify_download_integrity(&self, file_path: &PathBuf) -> (bool, Option<String>) {
        let mut computed_sha256 = None;

        // 1. Checksum verification
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

        // 2. Structural verification
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
