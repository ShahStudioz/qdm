//! # Download Task Controller Module
//!
//! Orchestrates the adaptive multi-connection downloading process for an individual file.
//!
//! True Multi-Stream Architecture (FDM / aria2 Model):
//! ===================================================
//! 1. **Long-Lived Continuous HTTP Streams**: Divides the file into long-lived streaming
//!    ranges (e.g. 2 massive halves for 2 connections). Each connection makes **ONE**
//!    HTTP request that streams continuously for the entire download duration.
//! 2. **Zero TCP/HTTP Re-negotiation Overhead**: Eliminates repeated HTTP GET requests,
//!    TCP slow-start congestion resets, and roundtrip request latency during active transfer.
//! 3. **Dynamic Tail Work-Stealing**: When an active connection completes its entire half early,
//!    it splits the remaining bytes of the slowest active stream once at the tail end.
//! 4. **Adaptive Scale-Up & Anti-429 Ceiling**: Starts safely with 2 connections, probes
//!    for capability, and locks concurrency ceiling instantly upon receiving HTTP 429.
//! 5. **Sliding Window Speed Meter**: Computes smooth speed over a rolling 1.5-second window.

use reqwest::Client;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch};

use crate::models::download::{ChunkState, DownloadItem, DownloadState, FileType};
use crate::services::http::diagnostics::{DiagEvent, DiagSender};
use crate::services::http::integrity;
use crate::services::http::throttler::Throttler;
use crate::services::http::worker::{ChunkWorker, WorkerEvent};
use crate::services::http::writer::PositionalWriter;

/// Events transmitted from the Task Controller back to the Download Engine.
#[derive(Debug, Clone)]
#[allow(dead_code, clippy::large_enum_variant)]
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
    WaitingForNetwork {
        id: usize,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
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

/// Rolling window speed meter that aggregates bytes over a 1.5-second window.
pub(crate) struct SlidingSpeedMeter {
    pub(crate) samples: VecDeque<(Instant, u64)>,
    window_duration: Duration,
    pub(crate) last_nonzero_speed: u64,
    pub(crate) last_nonzero_time: Instant,
}

impl SlidingSpeedMeter {
    pub(crate) fn new(window_duration: Duration) -> Self {
        Self {
            samples: VecDeque::with_capacity(64),
            window_duration,
            last_nonzero_speed: 0,
            last_nonzero_time: Instant::now(),
        }
    }

    pub(crate) fn record_bytes(&mut self, count: u64) {
        if count > 0 {
            let now = Instant::now();
            self.samples.push_back((now, count));
            self.prune(now);
        }
    }

    pub(crate) fn calculate_speed_bps(&mut self) -> u64 {
        let now = Instant::now();
        self.prune(now);

        if self.samples.is_empty() {
            let age_secs = now.duration_since(self.last_nonzero_time).as_secs_f64();
            if age_secs < 5.0 {
                // Smoothly decay from last known speed to 0 over 3.5s after the
                // 1.5s rolling window empties. This prevents the hard cliff-drop to 0
                // during HTTP re-request dead zones and transient network stalls.
                let decay = if age_secs <= 1.5 {
                    1.0_f64
                } else {
                    (1.0 - ((age_secs - 1.5) / 3.5)).clamp(0.0, 1.0)
                };
                return (self.last_nonzero_speed as f64 * decay) as u64;
            }
            return 0;
        }

        let total_bytes: u64 = self.samples.iter().map(|(_, bytes)| *bytes).sum();
        if total_bytes == 0 {
            return 0;
        }

        let oldest_time = self.samples.front().map(|(t, _)| *t).unwrap_or(now);
        let elapsed_secs = (now - oldest_time).as_secs_f64().max(0.5);

        let speed = (total_bytes as f64 / elapsed_secs) as u64;
        if speed > 0 {
            self.last_nonzero_speed = speed;
            self.last_nonzero_time = now;
        }

        speed
    }

    fn prune(&mut self, now: Instant) {
        while let Some(&(t, _)) = self.samples.front() {
            if now.saturating_duration_since(t) > self.window_duration {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }
}

/// Coordinates all chunk workers for a single active download item.
pub struct DownloadTaskController {
    pub item: DownloadItem,
    pub client: Client,
    pub pause_rx: watch::Receiver<bool>,
    pub task_event_tx: mpsc::Sender<TaskEvent>,
    active_workers: HashSet<usize>,
    worker_tasks: HashMap<usize, tokio::task::JoinHandle<()>>,
    max_allowed_concurrency: usize,
    current_target_concurrency: usize,
    next_scale_up_allowed: Instant,
    pub throttler: Option<Throttler>,
    /// Optional diagnostic side-channel. None in normal downloads, Some in speed_debug.
    pub diag: Option<DiagSender>,
}

impl DownloadTaskController {
    fn http_meta(&self) -> &crate::models::download::HttpMetadata {
        self.item
            .http_meta()
            .expect("DownloadTaskController only handles HTTP downloads")
    }

    fn http_meta_mut(&mut self) -> &mut crate::models::download::HttpMetadata {
        self.item
            .http_meta_mut()
            .expect("DownloadTaskController only handles HTTP downloads")
    }

    pub fn new(
        item: DownloadItem,
        client: Client,
        pause_rx: watch::Receiver<bool>,
        task_event_tx: mpsc::Sender<TaskEvent>,
    ) -> Self {
        let http = item
            .http_meta()
            .expect("DownloadTaskController only handles HTTP downloads");
        let max_concurrency = (item.max_connections as usize).clamp(1, 16);
        // Start conservatively with 2 connections (or 1 if non-resumable)
        let initial_target =
            if http.resumable && item.total_bytes.map(|t| t > 1024 * 1024).unwrap_or(false) {
                2.min(max_concurrency)
            } else {
                1
            };

        let throttler = item
            .speed_limit_bps
            .filter(|&bps| bps > 0)
            .map(Throttler::new);

        Self {
            item,
            client,
            pause_rx,
            task_event_tx,
            active_workers: HashSet::new(),
            worker_tasks: HashMap::new(),
            max_allowed_concurrency: max_concurrency,
            current_target_concurrency: initial_target,
            next_scale_up_allowed: Instant::now() + Duration::from_secs(4),
            throttler,
            diag: None,
        }
    }

    /// Attaches a diagnostic side-channel to this task controller.
    #[allow(dead_code)]
    pub fn with_diag(mut self, diag: DiagSender) -> Self {
        self.diag = Some(diag);
        self
    }

    /// Primary execution lifecycle for the download task.
    pub async fn run(mut self) {
        let clean_filename = crate::core::utils::paths::sanitize_filename(&self.item.filename);
        if clean_filename != self.item.filename {
            self.item.filename = clean_filename;
        }
        let save_path = PathBuf::from(&self.item.save_path);
        let final_file_path = save_path.join(&self.item.filename);
        let temp_file_path = save_path.join(format!("{}.qdmdownload", self.item.filename));

        // 1. Initialize Positional File Writer with disk pre-allocation on temporary .qdmdownload file
        let writer =
            match PositionalWriter::create_preallocated(&temp_file_path, self.item.total_bytes) {
                Ok(w) => w,
                Err(e) => {
                    let _ = self
                        .task_event_tx
                        .send(TaskEvent::Failed {
                            id: self.item.id,
                            error: format!(
                                "Failed to create output file {:?}: {}",
                                temp_file_path, e
                            ),
                            downloaded_bytes: self.item.downloaded_bytes,
                        })
                        .await;
                    return;
                }
            };

        // 2. Partition initial long-lived chunks or reuse existing chunk state
        if self.http_meta().chunks.is_empty() {
            let chunks = self.partition_initial_chunks();
            self.http_meta_mut().chunks = chunks;
        }

        // 3. Worker channels & execution loop
        // Channel sized for burst: a 4MB chunk at 16KB segments = ~256 events;
        // 2048 slots prevents backpressure from blocking workers during high-throughput bursts.
        let (worker_tx, mut worker_rx) = mpsc::channel::<WorkerEvent>(2048);

        // Spawn initial long-lived workers
        self.spawn_available_work(&writer, &worker_tx);

        let mut ui_ticker = tokio::time::interval(Duration::from_millis(150));
        ui_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut speed_meter = SlidingSpeedMeter::new(Duration::from_millis(2500));
        let mut last_persist_emit = Instant::now();
        // For TaskBytesReceived gap measurement
        let mut last_bytes_recv_at: Option<Instant> = None;

        /// Asynchronously waits until the pause receiver becomes `true`.
        /// If the sender is dropped or if the value is `false`, it stays pending indefinitely
        /// so it never spuriously interrupts other select! arms.
        async fn wait_for_pause(pause_rx: &mut watch::Receiver<bool>) {
            if *pause_rx.borrow() {
                return;
            }
            while let Ok(()) = pause_rx.changed().await {
                if *pause_rx.borrow_and_update() {
                    return;
                }
            }
            futures_util::future::pending::<()>().await;
        }

        loop {
            tokio::select! {
                // biased: pause_rx is polled first on every iteration, guaranteeing
                // that a pause/cancel signal is never delayed behind queued worker events.
                biased;
                _ = wait_for_pause(&mut self.pause_rx) => {
                    let _ = writer.sync_data();
                    let total_downloaded = self.calculate_total_downloaded();
                    self.item.downloaded_bytes = total_downloaded;
                    self.item.state = DownloadState::Paused {
                        downloaded_bytes: total_downloaded,
                        total_bytes: self.item.total_bytes,
                    };
                    let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                        item: self.item.clone(),
                    }).await;
                    return;
                }
                // Periodic UI progress update (steady 150ms intervals)
                _ = ui_ticker.tick() => {
                    let total_downloaded = self.calculate_total_downloaded();
                    self.item.downloaded_bytes = total_downloaded;
                    let cur_speed_bps = speed_meter.calculate_speed_bps();

                    let eta_secs = if cur_speed_bps > 0 {
                        self.item.total_bytes.map(|total| {
                            let remaining = total.saturating_sub(total_downloaded);
                            remaining / cur_speed_bps
                        })
                    } else {
                        None
                    };

                    // Diagnostic: emit speed calculation event with full meter state
                    if let Some(ref d) = self.diag {
                        let samples_count = speed_meter.samples.len();
                        let oldest_age = speed_meter.samples.front()
                            .map(|(t, _)| t.elapsed().as_micros() as u64)
                            .unwrap_or(0);
                        let age_secs = speed_meter.last_nonzero_time.elapsed().as_secs_f64();
                        let used_decay = speed_meter.samples.is_empty() && age_secs <= 5.0;
                        d.emit(DiagEvent::TaskSpeedCalc {
                            ts: d.now(),
                            samples_in_window: samples_count,
                            oldest_sample_age_micros: oldest_age,
                            result_bps: cur_speed_bps,
                            used_decay,
                        });
                    }

                    self.item.state = DownloadState::Downloading {
                        downloaded_bytes: total_downloaded,
                        total_bytes: self.item.total_bytes,
                        speed_bps: cur_speed_bps,
                        eta_secs,
                    };

                    let _ = self.task_event_tx.send(TaskEvent::ProgressUpdated {
                        id: self.item.id,
                        downloaded_bytes: total_downloaded,
                        total_bytes: self.item.total_bytes,
                        speed_bps: cur_speed_bps,
                        eta_secs,
                        chunks: self.http_meta().chunks.clone(),
                    }).await;

                    // Adaptive scaling check (attempt +1 connection after 4s if healthy)
                    if self.http_meta().resumable
                        && self.active_workers.len() == self.current_target_concurrency
                        && self.current_target_concurrency < self.max_allowed_concurrency
                        && Instant::now() >= self.next_scale_up_allowed
                    {
                        self.current_target_concurrency += 1;
                        self.next_scale_up_allowed = Instant::now() + Duration::from_secs(5);
                        self.spawn_available_work(&writer, &worker_tx);
                    }

                    // Periodic state persistence to JSON (every 3s)
                    if last_persist_emit.elapsed() >= Duration::from_secs(3) {
                        let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                            item: self.item.clone(),
                        }).await;
                        last_persist_emit = Instant::now();
                    }
                }
                event = worker_rx.recv() => {
                    match event {
                        Some(WorkerEvent::MetadataDiscovered { total_bytes, resumable }) => {
                            let mut changed = false;
                            if self.item.total_bytes.is_none() || self.item.total_bytes == Some(0) {
                                self.item.total_bytes = Some(total_bytes);
                                changed = true;
                                println!("[QDM Task {}] In-flight metadata discovered: total size = {} bytes", self.item.id, total_bytes);
                            }
                            if !self.http_meta().resumable && resumable {
                                self.http_meta_mut().resumable = true;
                                changed = true;
                            }
                            if changed {
                                if self.http_meta().chunks.len() == 1 && self.http_meta_mut().chunks[0].end_byte == u64::MAX {
                                    self.http_meta_mut().chunks[0].end_byte = total_bytes.saturating_sub(1);
                                }
                                let _ = self.task_event_tx.send(TaskEvent::ProgressUpdated {
                                    id: self.item.id,
                                    downloaded_bytes: self.item.downloaded_bytes,
                                    total_bytes: self.item.total_bytes,
                                    speed_bps: 0,
                                    eta_secs: None,
                                    chunks: self.http_meta().chunks.clone(),
                                }).await;
                                let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                                    item: self.item.clone(),
                                }).await;
                            }
                        }
                        Some(WorkerEvent::BytesDownloaded { chunk_id, count, current_offset }) => {
                            if let Some(chunk) = self.http_meta_mut().chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.current_offset = current_offset;
                            }
                            speed_meter.record_bytes(count);

                            // Diagnostic: emit receive event with gap measurement
                            if let Some(ref d) = self.diag {
                                let now = Instant::now();
                                let gap_micros = last_bytes_recv_at
                                    .map(|prev| now.duration_since(prev).as_micros() as u64)
                                    .unwrap_or(0);
                                d.emit(DiagEvent::TaskBytesReceived {
                                    ts: d.now(),
                                    chunk_id,
                                    count,
                                    gap_since_last_micros: gap_micros,
                                });
                                last_bytes_recv_at = Some(now);
                            } else {
                                last_bytes_recv_at = Some(Instant::now());
                            }
                        }
                        Some(WorkerEvent::ChunkCompleted { chunk_id }) => {
                            if let Some(chunk) = self.http_meta_mut().chunks.iter_mut().find(|c| c.id == chunk_id) {
                                chunk.is_completed = true;
                                chunk.current_offset = chunk.end_byte.saturating_add(1);
                            }
                            self.active_workers.remove(&chunk_id);
                            self.worker_tasks.remove(&chunk_id);

                            let total_downloaded = self.calculate_total_downloaded();
                            let all_chunks_done = self.http_meta().chunks.iter().all(|c| c.is_completed || (c.end_byte != u64::MAX && c.current_offset > c.end_byte));
                            let size_satisfied = self.item.total_bytes.map(|t| t > 0 && total_downloaded >= t).unwrap_or(false);

                            // Check if all chunks have finished or total size reached
                            if all_chunks_done || size_satisfied {
                                for (_, handle) in self.worker_tasks.drain() {
                                    handle.abort();
                                }
                                tokio::task::yield_now().await;
                                let _ = writer.sync_data();
                                drop(writer);

                                for c in &mut self.http_meta_mut().chunks {
                                    c.is_completed = true;
                                    c.current_offset = c.end_byte.saturating_add(1);
                                }
                                self.item.downloaded_bytes = total_downloaded;

                                // Rename temporary .qdmdownload file to final filename upon completion
                                if temp_file_path.exists() {
                                    if final_file_path.exists() {
                                        if final_file_path.is_dir() {
                                            let _ = std::fs::remove_dir_all(&final_file_path);
                                        } else {
                                            let _ = std::fs::remove_file(&final_file_path);
                                        }
                                    }
                                    let mut rename_success = false;
                                    for attempt in 0..10 {
                                        match std::fs::rename(&temp_file_path, &final_file_path) {
                                            Ok(_) => {
                                                rename_success = true;
                                                break;
                                            }
                                            Err(err) => {
                                                if attempt == 9 {
                                                    println!(
                                                        "[QDM Task {}] Error: Failed to rename {:?} to {:?}: {}",
                                                        self.item.id, temp_file_path, final_file_path, err
                                                    );
                                                } else {
                                                    tokio::time::sleep(Duration::from_millis(50)).await;
                                                }
                                            }
                                        }
                                    }

                                    if !rename_success {
                                        let _ = self.task_event_tx.send(TaskEvent::Failed {
                                            id: self.item.id,
                                            error: format!("Failed to finalize file: could not rename to {:?}", final_file_path.file_name().unwrap_or_default()),
                                            downloaded_bytes: total_downloaded,
                                        }).await;
                                        return;
                                    }
                                }

                                // Post-download integrity verification on final file
                                let (is_valid, computed_sha256) = self.verify_download_integrity(&final_file_path).await;

                                if is_valid {
                                    println!("[QDM Task {}] Download fully completed! (Total {} bytes)", self.item.id, total_downloaded);
                                    let _ = self.task_event_tx.send(TaskEvent::Completed {
                                        id: self.item.id,
                                        downloaded_bytes: total_downloaded,
                                        sha256: computed_sha256,
                                    }).await;
                                } else {
                                    let _ = self.task_event_tx.send(TaskEvent::Failed {
                                        id: self.item.id,
                                        error: "Post-download SHA-256 verification failed".to_string(),
                                        downloaded_bytes: total_downloaded,
                                    }).await;
                                }
                                return;
                            }

                            // Keep workers busy: dispatch uncompleted chunk or tail work-steal
                            self.spawn_available_work(&writer, &worker_tx);
                        }
                        Some(WorkerEvent::NetworkLost { chunk_id }) => {
                            self.active_workers.remove(&chunk_id);
                            self.worker_tasks.remove(&chunk_id);
                            let total_downloaded = self.calculate_total_downloaded();
                            self.item.downloaded_bytes = total_downloaded;
                            self.item.state = DownloadState::WaitingForNetwork {
                                downloaded_bytes: total_downloaded,
                                total_bytes: self.item.total_bytes,
                            };
                            println!("[QDM Task {}] Network lost on worker {}. Pausing in WaitingForNetwork state...", self.item.id, chunk_id);
                            let _ = writer.sync_data();
                            let _ = self.task_event_tx.send(TaskEvent::WaitingForNetwork {
                                id: self.item.id,
                                downloaded_bytes: total_downloaded,
                                total_bytes: self.item.total_bytes,
                            }).await;
                            let _ = self.task_event_tx.send(TaskEvent::StatePersistRequested {
                                item: self.item.clone(),
                            }).await;
                            return;
                        }
                        Some(WorkerEvent::RateLimited { chunk_id, retry_after: _ }) => {
                            // Smart Concurrency Reduction: Throttle concurrency down to ease server load
                            if self.current_target_concurrency > 1 {
                                self.current_target_concurrency = (self.current_target_concurrency / 2).max(1);
                                self.max_allowed_concurrency = self.current_target_concurrency;
                                self.next_scale_up_allowed = Instant::now() + Duration::from_secs(60);
                                println!(
                                    "[QDM Task {}] Server rate limit / 403 on worker {}. Dynamically reduced concurrency to {} connections.",
                                    self.item.id, chunk_id, self.current_target_concurrency
                                );
                            }
                        }
                        Some(WorkerEvent::WorkerFailed { chunk_id, error }) => {
                            self.active_workers.remove(&chunk_id);
                            self.worker_tasks.remove(&chunk_id);
                            println!(
                                "[QDM Task {}] Worker {} stopped: {}. Rescheduling chunk from last saved offset...",
                                self.item.id, chunk_id, error
                            );

                            // Auto-recovery: If all workers dropped (e.g. transient network outage or IP rate limit),
                            // wait 3 seconds and trigger an automatic reconnect attempt without losing any chunk progress!
                            if self.active_workers.is_empty() && self.http_meta().chunks.iter().any(|c| !c.is_completed) {
                                let mut pause_rx = self.pause_rx.clone();
                                let worker_tx_clone = worker_tx.clone();

                                tokio::spawn(async move {
                                    tokio::select! {
                                        _ = tokio::time::sleep(Duration::from_secs(3)) => {
                                            let _ = worker_tx_clone.send(WorkerEvent::BytesDownloaded {
                                                chunk_id: 0,
                                                count: 0,
                                                current_offset: 0,
                                            }).await;
                                        }
                                        _ = wait_for_pause(&mut pause_rx) => {}
                                    }
                                });
                            }

                            // Re-dispatch work immediately for any remaining/retryable chunks
                            self.spawn_available_work(&writer, &worker_tx);
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Partitions the initial file size into massive contiguous long-lived ranges.
    fn partition_initial_chunks(&self) -> Vec<ChunkState> {
        let total = match self.item.total_bytes {
            Some(t) if t > 0 => t,
            _ => {
                return vec![ChunkState::new(
                    0,
                    &self.http_meta().primary_url.url,
                    0,
                    u64::MAX,
                )]
            }
        };

        // Don't chunk small files (< 1MB) or non-resumable servers
        if !self.http_meta().resumable || total < 1024 * 1024 {
            return vec![ChunkState::new(
                0,
                &self.http_meta().primary_url.url,
                0,
                total - 1,
            )];
        }

        // Long-Lived Streams: create initial_target massive chunks (e.g. 2 chunks for 2 initial connections)
        let num_connections = self.current_target_concurrency.clamp(1, 16);
        let mut chunks = Vec::with_capacity(num_connections);

        let mut available_urls = vec![self.http_meta().primary_url.url.clone()];
        for mirror in &self.http_meta().mirror_urls {
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

    /// Spawns workers for unassigned chunks or performs dynamic tail work-stealing from busy workers.
    fn spawn_available_work(
        &mut self,
        writer: &PositionalWriter,
        worker_tx: &mpsc::Sender<WorkerEvent>,
    ) {
        while self.active_workers.len() < self.current_target_concurrency {
            // 1. Check for an uncompleted chunk that is NOT currently active
            let next_chunk_idx = self.http_meta().chunks.iter().position(|c| {
                !c.is_completed
                    && c.current_offset <= c.end_byte
                    && !self.active_workers.contains(&c.id)
            });

            if let Some(idx) = next_chunk_idx {
                let chunk = self.http_meta().chunks[idx].clone();
                let chunk_id = chunk.id;
                self.active_workers.insert(chunk_id);

                // Emit: worker spawned
                if let Some(ref d) = self.diag {
                    d.emit(DiagEvent::TaskWorkerSpawned {
                        ts: d.now(),
                        chunk_id,
                        start_offset: chunk.current_offset,
                        end_offset: chunk.end_byte,
                    });
                }

                let mut worker = ChunkWorker::new(
                    chunk,
                    self.client.clone(),
                    writer.clone(),
                    worker_tx.clone(),
                    self.pause_rx.clone(),
                )
                .with_throttler(self.throttler.clone());
                if let Some(ref d) = self.diag {
                    worker = worker.with_diag(d.clone());
                }

                let handle = tokio::spawn(async move {
                    worker.run(Duration::ZERO).await;
                });
                self.worker_tasks.insert(chunk_id, handle);
            } else {
                // 2. Dynamic Work-Stealing: Find the active chunk with the largest remaining byte range (> 4MB)
                let largest_idx = self
                    .http_meta()
                    .chunks
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| {
                        !c.is_completed && c.end_byte != u64::MAX && c.end_byte > c.current_offset
                    })
                    .max_by_key(|(_, c)| c.end_byte - c.current_offset)
                    .map(|(idx, _)| idx);

                if let Some(idx) = largest_idx {
                    let busy_chunk = &mut self.http_meta_mut().chunks[idx];
                    let remaining = busy_chunk.end_byte - busy_chunk.current_offset;
                    const MIN_STEAL_SIZE: u64 = 4 * 1024 * 1024; // Minimum 4MB to warrant a split

                    if remaining >= MIN_STEAL_SIZE {
                        let midpoint = busy_chunk.current_offset + (remaining / 2);
                        let stolen_start = midpoint + 1;
                        let stolen_end = busy_chunk.end_byte;
                        let url = busy_chunk.url.clone();

                        // Truncate existing chunk's end
                        busy_chunk.end_byte = midpoint;

                        let new_id = self.http_meta().chunks.len();
                        let new_chunk = ChunkState::new(new_id, url, stolen_start, stolen_end);
                        self.http_meta_mut().chunks.push(new_chunk.clone());
                        self.active_workers.insert(new_id);

                        // Emit: stolen worker spawned
                        if let Some(ref d) = self.diag {
                            d.emit(DiagEvent::TaskWorkerSpawned {
                                ts: d.now(),
                                chunk_id: new_id,
                                start_offset: stolen_start,
                                end_offset: stolen_end,
                            });
                        }

                        let mut worker = ChunkWorker::new(
                            new_chunk,
                            self.client.clone(),
                            writer.clone(),
                            worker_tx.clone(),
                            self.pause_rx.clone(),
                        )
                        .with_throttler(self.throttler.clone());
                        if let Some(ref d) = self.diag {
                            worker = worker.with_diag(d.clone());
                        }

                        let handle = tokio::spawn(async move {
                            worker.run(Duration::ZERO).await;
                        });
                        self.worker_tasks.insert(new_id, handle);
                        continue;
                    }
                }

                // No more stealable work
                break;
            }
        }
    }

    fn calculate_total_downloaded(&self) -> u64 {
        self.http_meta()
            .chunks
            .iter()
            .map(|c| c.downloaded_bytes())
            .sum()
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

        // 2. Structural verification (advisory logging)
        if self.item.file_type == FileType::Archive {
            if let Ok(is_valid_structure) =
                integrity::verify_structure(file_path, self.item.file_type).await
            {
                if !is_valid_structure {
                    println!("[QDM Integrity] Note: Archive structure check did not match standard ZIP headers for {:?}", file_path);
                }
            }
        }

        (true, computed_sha256)
    }
}
