//! # Chunk Worker Module
//!
//! Implements a resilient, long-lived streaming worker that handles an exact contiguous byte range.
//!
//! Key Safeguards:
//! - **Strict HTTP 206 Handling**: Aborts multi-part download immediately if the server responds with 200 OK.
//! - **Anti-429 Dynamic Backoff**: Inspects `Retry-After` headers and backs off smoothly on rate limits.
//! - **ETag / Last-Modified Validation**: Prevents resuming corrupted files if the remote source has changed.
//! - **Staggered Spawning**: Introduces configurable delay to prevent thundering-herd CDN/DDoS triggers.
//! - **Atomic Positional Disk Writes**: Streams bytes directly to exact disk offsets via `PositionalWriter`.

use std::time::Duration;
use reqwest::header::{HeaderMap, IF_MATCH, IF_UNMODIFIED_SINCE, RANGE, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use tokio::sync::{mpsc, watch};

use crate::models::download::ChunkState;
use crate::services::downloads::writer::PositionalWriter;

/// Events reported by individual chunk workers back to the download task controller.
#[derive(Debug)]
#[allow(dead_code)]
pub enum WorkerEvent {
    /// Worker wrote `count` bytes to disk starting at its current offset.
    BytesDownloaded { chunk_id: usize, count: u64, current_offset: u64 },
    /// Worker successfully reached the end of its allocated byte range.
    ChunkCompleted { chunk_id: usize },
    /// Server returned 200 OK instead of 206 Partial Content: must fall back to single-stream.
    FallbackToSingleStream { chunk_id: usize },
    /// Server ETag or Last-Modified header changed mid-download: remote file was modified.
    ServerFileChanged { chunk_id: usize, new_etag: Option<String> },
    /// Encountered HTTP 429 Too Many Requests: worker entered backoff.
    RateLimited { chunk_id: usize, retry_after: Option<Duration> },
    /// Worker encountered an unrecoverable failure.
    WorkerFailed { chunk_id: usize, error: String },
}

/// A dedicated worker responsible for downloading a contiguous slice `[start_byte, end_byte]` of a file.
pub struct ChunkWorker {
    pub chunk: ChunkState,
    pub client: Client,
    pub writer: PositionalWriter,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub event_tx: mpsc::Sender<WorkerEvent>,
    pub pause_rx: watch::Receiver<bool>,
}

impl ChunkWorker {
    pub fn new(
        chunk: ChunkState,
        client: Client,
        writer: PositionalWriter,
        etag: Option<String>,
        last_modified: Option<String>,
        event_tx: mpsc::Sender<WorkerEvent>,
        pause_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            chunk,
            client,
            writer,
            etag,
            last_modified,
            event_tx,
            pause_rx,
        }
    }

    /// Executes the worker's long-lived streaming loop with staggered spawn delay.
    pub async fn run(mut self, spawn_delay: Duration) {
        // 1. Staggered Spawning Delay: Jitter between worker start times avoids DDoS tripping
        if !spawn_delay.is_zero() {
            tokio::select! {
                _ = tokio::time::sleep(spawn_delay) => {}
                _ = self.pause_rx.changed() => {
                    if *self.pause_rx.borrow() {
                        return; // Paused before starting
                    }
                }
            }
        }

        // Check if chunk is already fully downloaded
        if self.chunk.current_offset > self.chunk.end_byte {
            let _ = self
                .event_tx
                .send(WorkerEvent::ChunkCompleted {
                    chunk_id: self.chunk.id,
                })
                .await;
            return;
        }

        let mut retry_count = 0u32;
        const MAX_RETRIES: u32 = 8;
        const SYNC_INTERVAL_BYTES: u64 = 2 * 1024 * 1024; // 2MB sync intervals for crash safety

        loop {
            // Check if download was paused or cancelled by user
            if *self.pause_rx.borrow() {
                let _ = self.writer.sync_data();
                return;
            }

            let range_header = format!("bytes={}-{}", self.chunk.current_offset, self.chunk.end_byte);
            let mut req = self.client.get(&self.chunk.url).header(RANGE, range_header);

            // Guard against remote file updates mid-download using HTTP preconditions
            if let Some(ref etag) = self.etag {
                req = req.header(IF_MATCH, etag.clone());
            } else if let Some(ref lm) = self.last_modified {
                req = req.header(IF_UNMODIFIED_SINCE, lm.clone());
            }

            let response_result = tokio::select! {
                res = req.send() => res,
                _ = self.pause_rx.changed() => {
                    if *self.pause_rx.borrow() {
                        let _ = self.writer.sync_data();
                        return;
                    }
                    continue;
                }
            };

            match response_result {
                Ok(resp) => {
                    let status = resp.status();

                    // --- 1. Anti-429 Rate Limiting & Dynamic Backoff ---
                    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
                        let retry_after = parse_retry_after(resp.headers());
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::RateLimited {
                                chunk_id: self.chunk.id,
                                retry_after,
                            })
                            .await;

                        // Gracefully terminate worker and let the task controller adapt concurrency
                        let _ = self.writer.sync_data();
                        return;
                    }

                    // --- 2. Strict HTTP 206 Partial Content Check ---
                    if status == StatusCode::OK {
                        // The server ignored the Range header and sent the whole file starting from byte 0.
                        // Writing byte 0 data at chunk.current_offset would corrupt the file offsets!
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::FallbackToSingleStream {
                                chunk_id: self.chunk.id,
                            })
                            .await;
                        return;
                    }

                    // --- 3. Precondition Failed Check (ETag / Last-Modified changed) ---
                    if status == StatusCode::PRECONDITION_FAILED {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::ServerFileChanged {
                                chunk_id: self.chunk.id,
                                new_etag: None,
                            })
                            .await;
                        return;
                    }

                    // Range Not Satisfiable: chunk might be finished
                    if status == StatusCode::RANGE_NOT_SATISFIABLE {
                        if self.chunk.current_offset >= self.chunk.end_byte {
                            let _ = self
                                .event_tx
                                .send(WorkerEvent::ChunkCompleted {
                                    chunk_id: self.chunk.id,
                                })
                                .await;
                            return;
                        }
                    }

                    if !status.is_success() {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::WorkerFailed {
                                chunk_id: self.chunk.id,
                                error: format!("Server returned error HTTP {}", status),
                            })
                            .await;
                        return;
                    }

                    // --- 4. Verify ETag header on 206 response if provided ---
                    if let Some(resp_etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
                        if let Some(ref saved_etag) = self.etag {
                            if saved_etag != resp_etag {
                                let _ = self
                                    .event_tx
                                    .send(WorkerEvent::ServerFileChanged {
                                        chunk_id: self.chunk.id,
                                        new_etag: Some(resp_etag.to_string()),
                                    })
                                    .await;
                                return;
                            }
                        }
                    }

                    // Reset retry count upon establishing successful stream
                    retry_count = 0;

                    // --- 5. Long-Lived Byte Streaming Loop ---
                    let mut stream = resp;
                    let mut unpersisted_bytes = 0u64;

                    loop {
                        if *self.pause_rx.borrow() {
                            let _ = self.writer.sync_data();
                            return;
                        }

                        let chunk_result = tokio::select! {
                            res = stream.chunk() => res,
                            _ = self.pause_rx.changed() => {
                                if *self.pause_rx.borrow() {
                                    let _ = self.writer.sync_data();
                                    return;
                                }
                                continue;
                            }
                        };

                        match chunk_result {
                            Ok(Some(bytes)) => {
                                let len = bytes.len() as u64;

                                // Prevent writing beyond the designated end_byte of this chunk
                                let max_writable = if self.chunk.current_offset + len > self.chunk.end_byte + 1 {
                                    (self.chunk.end_byte + 1).saturating_sub(self.chunk.current_offset) as usize
                                } else {
                                    bytes.len()
                                };

                                if max_writable > 0 {
                                    let slice = &bytes[..max_writable];
                                    if let Err(err) = self.writer.write_at(self.chunk.current_offset, slice) {
                                        let _ = self
                                            .event_tx
                                            .send(WorkerEvent::WorkerFailed {
                                                chunk_id: self.chunk.id,
                                                error: format!("Disk write failure: {}", err),
                                            })
                                            .await;
                                        return;
                                    }

                                    self.chunk.current_offset += max_writable as u64;
                                    unpersisted_bytes += max_writable as u64;

                                    let _ = self
                                        .event_tx
                                        .send(WorkerEvent::BytesDownloaded {
                                            chunk_id: self.chunk.id,
                                            count: max_writable as u64,
                                            current_offset: self.chunk.current_offset,
                                        })
                                        .await;

                                    // Periodic disk sync before recording progress
                                    if unpersisted_bytes >= SYNC_INTERVAL_BYTES {
                                        let _ = self.writer.sync_data();
                                        unpersisted_bytes = 0;
                                    }
                                }

                                if self.chunk.current_offset > self.chunk.end_byte {
                                    // Chunk completed
                                    break;
                                }
                            }
                            Ok(None) => {
                                // Stream ended
                                break;
                            }
                            Err(err) => {
                                let _ = self.writer.sync_data();
                                retry_count += 1;
                                if retry_count > MAX_RETRIES {
                                    let _ = self
                                        .event_tx
                                        .send(WorkerEvent::WorkerFailed {
                                            chunk_id: self.chunk.id,
                                            error: format!("Stream interrupted: {}", err),
                                        })
                                        .await;
                                    return;
                                }
                                tokio::time::sleep(Duration::from_millis(500 * (1 << retry_count.min(4)))).await;
                                break; // Will trigger re-request from chunk.current_offset
                            }
                        }
                    }

                    let _ = self.writer.sync_data();

                    if self.chunk.current_offset > self.chunk.end_byte {
                        self.chunk.is_completed = true;
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::ChunkCompleted {
                                chunk_id: self.chunk.id,
                            })
                            .await;
                        return;
                    }
                }
                Err(err) => {
                    retry_count += 1;
                    if retry_count > MAX_RETRIES {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::WorkerFailed {
                                chunk_id: self.chunk.id,
                                error: format!("Connection error: {}", err),
                            })
                            .await;
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(500 * (1 << retry_count.min(4)))).await;
                }
            }
        }
    }
}

/// Parses the standard HTTP `Retry-After` header value (seconds).
fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    if let Some(val) = headers.get(RETRY_AFTER).and_then(|v| v.to_str().ok()) {
        if let Ok(secs) = val.trim().parse::<u64>() {
            return Some(Duration::from_secs(secs));
        }
    }
    None
}
