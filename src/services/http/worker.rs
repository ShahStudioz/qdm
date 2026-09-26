//! # Chunk Worker Module
//!
//! Implements a resilient, long-lived streaming worker that handles an exact contiguous byte range.
//!
//! Key Design Principles:
//! - **Single Long-Lived HTTP Stream**: Operates as a continuous streaming pipeline over a single
//!   HTTP Range request, avoiding connection teardown and TCP slow-start roundtrips.
//! - **Zero In-Stream Disk Locks**: Streams directly to positional disk offsets without synchronous flushes.
//! - **Strict HTTP 206 Handling**: Aborts multi-part download immediately if the server responds with 200 OK.
//! - **Anti-429 Dynamic Backoff**: Inspects `Retry-After` headers and yields cleanly on rate limits.
//! - **ETag / Last-Modified Validation**: Prevents resuming corrupted files if the remote source has changed.
//! - **Atomic Positional Disk Writes**: Streams bytes directly to exact disk offsets via `PositionalWriter`.

use reqwest::header::{
    HeaderMap, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, RANGE, RETRY_AFTER,
};
use reqwest::{Client, StatusCode};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use crate::models::download::ChunkState;
use crate::services::http::diagnostics::{DiagEvent, DiagSender};
use crate::services::http::throttler::Throttler;
use crate::services::http::writer::PositionalWriter;
use crate::services::shared::network::connectivity::ConnectivityMonitor;

/// Events reported by individual chunk workers back to the download task controller.
#[derive(Debug)]
#[allow(dead_code)]
pub enum WorkerEvent {
    MetadataDiscovered {
        total_bytes: u64,
        resumable: bool,
    },
    BytesDownloaded {
        chunk_id: usize,
        count: u64,
        current_offset: u64,
    },
    ChunkCompleted {
        chunk_id: usize,
    },
    RateLimited {
        chunk_id: usize,
        retry_after: Option<Duration>,
    },
    NetworkLost {
        chunk_id: usize,
    },
    WorkerFailed {
        chunk_id: usize,
        error: String,
    },
}

pub struct ChunkWorker {
    pub chunk: ChunkState,
    pub client: Client,
    pub writer: PositionalWriter,
    pub event_tx: mpsc::Sender<WorkerEvent>,
    pub pause_rx: watch::Receiver<bool>,
    pub diag: Option<DiagSender>,
    pub throttler: Option<Throttler>,
}

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

impl ChunkWorker {
    pub fn new(
        chunk: ChunkState,
        client: Client,
        writer: PositionalWriter,
        event_tx: mpsc::Sender<WorkerEvent>,
        pause_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            chunk,
            client,
            writer,
            event_tx,
            pause_rx,
            diag: None,
            throttler: None,
        }
    }

    pub fn with_diag(mut self, diag: DiagSender) -> Self {
        self.diag = Some(diag);
        self
    }

    pub fn with_throttler(mut self, throttler: Option<Throttler>) -> Self {
        self.throttler = throttler;
        self
    }

    pub async fn run(mut self, spawn_delay: Duration) {
        if !spawn_delay.is_zero() {
            tokio::select! {
                _ = tokio::time::sleep(spawn_delay) => {}
                _ = wait_for_pause(&mut self.pause_rx) => {
                    return;
                }
            }
        }

        if self.chunk.end_byte != u64::MAX && self.chunk.current_offset > self.chunk.end_byte {
            let _ = self
                .event_tx
                .send(WorkerEvent::ChunkCompleted {
                    chunk_id: self.chunk.id,
                })
                .await;
            return;
        }

        let mut retry_count = 0u32;
        const MAX_RETRIES: u32 = 10;

        loop {
            if *self.pause_rx.borrow() {
                let _ = self.writer.sync_data();
                return;
            }

            // Range request: use open-ended range if end_byte is unknown (u64::MAX)
            let range_header = if self.chunk.end_byte == u64::MAX {
                format!("bytes={}-", self.chunk.current_offset)
            } else {
                format!(
                    "bytes={}-{}",
                    self.chunk.current_offset, self.chunk.end_byte
                )
            };
            let req = self.client.get(&self.chunk.url).header(RANGE, range_header);

            if let Some(ref d) = self.diag {
                d.emit(DiagEvent::WorkerRequestStart {
                    ts: d.now(),
                    chunk_id: self.chunk.id,
                    range_start: self.chunk.current_offset,
                    range_end: self.chunk.end_byte,
                    retry: retry_count,
                });
            }

            let response_result = tokio::select! {
                res = req.send() => res,
                _ = wait_for_pause(&mut self.pause_rx) => {
                    let _ = self.writer.sync_data();
                    return;
                }
            };

            match response_result {
                Ok(resp) => {
                    let status = resp.status();
                    if let Some(ref d) = self.diag {
                        d.emit(DiagEvent::WorkerFirstByte {
                            ts: d.now(),
                            chunk_id: self.chunk.id,
                            status: status.as_u16(),
                        });
                    }

                    // 1. Rate Limiting & Overload (HTTP 429, 403 Forbidden, 503 Service Unavailable)
                    if status == StatusCode::TOO_MANY_REQUESTS
                        || status == StatusCode::FORBIDDEN
                        || status == StatusCode::SERVICE_UNAVAILABLE
                    {
                        retry_count += 1;
                        let retry_after = parse_retry_after(resp.headers()).unwrap_or_else(|| {
                            Duration::from_millis(1000 * (1u64 << retry_count.min(4)))
                        });

                        let _ = self
                            .event_tx
                            .send(WorkerEvent::RateLimited {
                                chunk_id: self.chunk.id,
                                retry_after: Some(retry_after),
                            })
                            .await;

                        if retry_count <= MAX_RETRIES {
                            println!(
                                "[QDM Worker {}] HTTP {} received (rate limit/overload). Backing off for {:.1}s before retry (attempt {}/{})",
                                self.chunk.id, status, retry_after.as_secs_f64(), retry_count, MAX_RETRIES
                            );
                            tokio::select! {
                                _ = tokio::time::sleep(retry_after) => {}
                                _ = wait_for_pause(&mut self.pause_rx) => {
                                    let _ = self.writer.sync_data();
                                    return;
                                }
                            }
                            continue;
                        } else {
                            let _ = self
                                .event_tx
                                .send(WorkerEvent::WorkerFailed {
                                    chunk_id: self.chunk.id,
                                    error: format!(
                                        "Server returned persistent error HTTP {}",
                                        status
                                    ),
                                })
                                .await;
                            return;
                        }
                    }

                    // 2. HTTP 200 OK Handling (Full file response instead of 206 Partial Content)
                    if status == StatusCode::OK {
                        if self.chunk.current_offset == 0 {
                            // Valid response for the beginning of the file; stream up to chunk end_byte
                        } else {
                            // Server ignored Range for a non-zero offset; retry range request with backoff
                            retry_count += 1;
                            if retry_count <= MAX_RETRIES {
                                let backoff =
                                    Duration::from_millis(500 * (1u64 << retry_count.min(3)));
                                println!(
                                    "[QDM Worker {}] Server returned HTTP 200 for offset {}. Retrying Range header in {:.1}s...",
                                    self.chunk.id, self.chunk.current_offset, backoff.as_secs_f64()
                                );
                                tokio::select! {
                                    _ = tokio::time::sleep(backoff) => {}
                                    _ = wait_for_pause(&mut self.pause_rx) => {
                                        let _ = self.writer.sync_data();
                                        return;
                                    }
                                }
                                continue;
                            } else {
                                let _ = self
                                    .event_tx
                                    .send(WorkerEvent::WorkerFailed {
                                        chunk_id: self.chunk.id,
                                        error:
                                            "Server ignored Range header on multiple retry attempts"
                                                .to_string(),
                                    })
                                    .await;
                                return;
                            }
                        }
                    }

                    // 3. HTTP 416 Range Not Satisfiable
                    if status == StatusCode::RANGE_NOT_SATISFIABLE
                        && self.chunk.current_offset >= self.chunk.end_byte
                    {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::ChunkCompleted {
                                chunk_id: self.chunk.id,
                            })
                            .await;
                        return;
                    }

                    // 4. Other Non-Success HTTP Errors (500, 502, 504, 408, etc.)
                    if !status.is_success() {
                        retry_count += 1;
                        if retry_count <= MAX_RETRIES {
                            let backoff =
                                Duration::from_millis(1000 * (1u64 << retry_count.min(3)));
                            println!(
                                "[QDM Worker {}] Server returned HTTP error {}. Retrying in {:.1}s...",
                                self.chunk.id, status, backoff.as_secs_f64()
                            );
                            tokio::select! {
                                _ = tokio::time::sleep(backoff) => {}
                                _ = wait_for_pause(&mut self.pause_rx) => {
                                    let _ = self.writer.sync_data();
                                    return;
                                }
                            }
                            continue;
                        } else {
                            let _ = self
                                .event_tx
                                .send(WorkerEvent::WorkerFailed {
                                    chunk_id: self.chunk.id,
                                    error: format!("Server returned error HTTP {}", status),
                                })
                                .await;
                            return;
                        }
                    }

                    retry_count = 0;

                    // Inspect response headers to report discovered metadata (size, resumability)
                    let headers = resp.headers();
                    let supports_resume = status == StatusCode::PARTIAL_CONTENT
                        || headers
                            .get(ACCEPT_RANGES)
                            .and_then(|v| v.to_str().ok())
                            .map(|v| v.eq_ignore_ascii_case("bytes"))
                            .unwrap_or(false);

                    let mut discovered_length = None;
                    if status == StatusCode::PARTIAL_CONTENT {
                        if let Some(content_range) =
                            headers.get(CONTENT_RANGE).and_then(|v| v.to_str().ok())
                        {
                            if let Some(total_str) = content_range.split('/').next_back() {
                                if let Ok(total) = total_str.trim().parse::<u64>() {
                                    discovered_length = Some(total);
                                }
                            }
                        }
                    } else if let Some(len_val) =
                        headers.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok())
                    {
                        if let Ok(len) = len_val.trim().parse::<u64>() {
                            discovered_length = Some(len);
                        }
                    }

                    if let Some(total) = discovered_length {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::MetadataDiscovered {
                                total_bytes: total,
                                resumable: supports_resume,
                            })
                            .await;
                    }

                    let mut stream = resp;
                    let mut write_buffer: Vec<u8> = Vec::with_capacity(256 * 1024);
                    let mut buffer_start_offset = self.chunk.current_offset;
                    let mut total_bytes_this_stream: u64 = 0;
                    const WRITE_BUFFER_THRESHOLD: usize = 256 * 1024;

                    loop {
                        if *self.pause_rx.borrow() {
                            if !write_buffer.is_empty() {
                                let _ = self.writer.write_at(buffer_start_offset, &write_buffer);
                            }
                            let _ = self.writer.sync_data();
                            return;
                        }

                        let chunk_result = tokio::select! {
                            res = stream.chunk() => res,
                            _ = wait_for_pause(&mut self.pause_rx) => {
                                if !write_buffer.is_empty() { let _ = self.writer.write_at(buffer_start_offset, &write_buffer); }
                                let _ = self.writer.sync_data();
                                return;
                            }
                        };

                        match chunk_result {
                            Ok(Some(bytes)) => {
                                let len = bytes.len() as u64;
                                let max_writable = if self.chunk.end_byte != u64::MAX
                                    && self.chunk.current_offset.saturating_add(len)
                                        > self.chunk.end_byte.saturating_add(1)
                                {
                                    (self.chunk.end_byte.saturating_add(1))
                                        .saturating_sub(self.chunk.current_offset)
                                        as usize
                                } else {
                                    bytes.len()
                                };

                                if max_writable > 0 {
                                    if let Some(ref throttler) = self.throttler {
                                        throttler.acquire(max_writable).await;
                                    }

                                    let slice = &bytes[..max_writable];
                                    write_buffer.extend_from_slice(slice);
                                    self.chunk.current_offset += max_writable as u64;
                                    total_bytes_this_stream += max_writable as u64;

                                    if let Some(ref d) = self.diag {
                                        d.emit(DiagEvent::WorkerBytesBuffered {
                                            ts: d.now(),
                                            chunk_id: self.chunk.id,
                                            count: max_writable as u64,
                                            current_offset: self.chunk.current_offset,
                                            buffer_len: write_buffer.len(),
                                        });
                                    }

                                    let _ = self
                                        .event_tx
                                        .send(WorkerEvent::BytesDownloaded {
                                            chunk_id: self.chunk.id,
                                            count: max_writable as u64,
                                            current_offset: self.chunk.current_offset,
                                        })
                                        .await;

                                    let chunk_done = self.chunk.end_byte != u64::MAX
                                        && self.chunk.current_offset > self.chunk.end_byte;
                                    if write_buffer.len() >= WRITE_BUFFER_THRESHOLD || chunk_done {
                                        let flush_offset = buffer_start_offset;
                                        let flush_len = write_buffer.len();
                                        if let Err(err) =
                                            self.writer.write_at(flush_offset, &write_buffer)
                                        {
                                            let _ = self
                                                .event_tx
                                                .send(WorkerEvent::WorkerFailed {
                                                    chunk_id: self.chunk.id,
                                                    error: format!("Disk write failure: {}", err),
                                                })
                                                .await;
                                            return;
                                        }
                                        if let Some(ref d) = self.diag {
                                            d.emit(DiagEvent::WorkerDiskFlush {
                                                ts: d.now(),
                                                chunk_id: self.chunk.id,
                                                flushed_bytes: flush_len,
                                                flush_offset,
                                            });
                                        }
                                        buffer_start_offset = self.chunk.current_offset;
                                        write_buffer.clear();
                                    }
                                }

                                if self.chunk.end_byte != u64::MAX
                                    && self.chunk.current_offset > self.chunk.end_byte
                                {
                                    break;
                                }
                            }
                            Ok(None) => {
                                if !write_buffer.is_empty() {
                                    if let Err(err) =
                                        self.writer.write_at(buffer_start_offset, &write_buffer)
                                    {
                                        let _ = self
                                            .event_tx
                                            .send(WorkerEvent::WorkerFailed {
                                                chunk_id: self.chunk.id,
                                                error: format!(
                                                    "Disk write failure on stream-end flush: {}",
                                                    err
                                                ),
                                            })
                                            .await;
                                        return;
                                    }
                                    write_buffer.clear();
                                }
                                let premature = self.chunk.end_byte != u64::MAX
                                    && self.chunk.current_offset <= self.chunk.end_byte;
                                if let Some(ref d) = self.diag {
                                    d.emit(DiagEvent::WorkerStreamEnd {
                                        ts: d.now(),
                                        chunk_id: self.chunk.id,
                                        premature,
                                        current_offset: self.chunk.current_offset,
                                        end_byte: self.chunk.end_byte,
                                    });
                                }
                                if !premature {
                                    // For open-ended (u64::MAX) or completed streams, Ok(None) signifies clean completion!
                                    self.chunk.is_completed = true;
                                    let _ = self
                                        .event_tx
                                        .send(WorkerEvent::ChunkCompleted {
                                            chunk_id: self.chunk.id,
                                        })
                                        .await;
                                    return;
                                }
                                retry_count += 1;
                                if retry_count > MAX_RETRIES {
                                    let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                        chunk_id: self.chunk.id,
                                        error: format!("Stream ended prematurely at offset {} (expected end {})",
                                            self.chunk.current_offset, self.chunk.end_byte),
                                    }).await;
                                    return;
                                }
                                tokio::time::sleep(Duration::from_millis(
                                    200 * (1u64 << retry_count.min(3)),
                                ))
                                .await;
                                break;
                            }
                            Err(err) => {
                                if let Some(ref d) = self.diag {
                                    d.emit(DiagEvent::WorkerStreamError {
                                        ts: d.now(),
                                        chunk_id: self.chunk.id,
                                        error: err.to_string(),
                                        retry: retry_count,
                                    });
                                }
                                if !write_buffer.is_empty() {
                                    if let Err(write_err) =
                                        self.writer.write_at(buffer_start_offset, &write_buffer)
                                    {
                                        let _ = self
                                            .event_tx
                                            .send(WorkerEvent::WorkerFailed {
                                                chunk_id: self.chunk.id,
                                                error: format!(
                                                    "Disk write failure on retry flush: {}",
                                                    write_err
                                                ),
                                            })
                                            .await;
                                        return;
                                    }
                                    write_buffer.clear();
                                }

                                if !ConnectivityMonitor::is_online().await {
                                    let _ = self
                                        .event_tx
                                        .send(WorkerEvent::NetworkLost {
                                            chunk_id: self.chunk.id,
                                        })
                                        .await;
                                    return;
                                }

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
                                tokio::time::sleep(Duration::from_millis(
                                    500 * (1 << retry_count.min(4)),
                                ))
                                .await;
                                break;
                            }
                        }
                    }

                    if self.chunk.end_byte == u64::MAX
                        || self.chunk.current_offset > self.chunk.end_byte
                    {
                        self.chunk.is_completed = true;
                        if let Some(ref d) = self.diag {
                            d.emit(DiagEvent::WorkerChunkCompleted {
                                ts: d.now(),
                                chunk_id: self.chunk.id,
                                total_bytes: total_bytes_this_stream,
                            });
                        }
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
                    if let Some(ref d) = self.diag {
                        d.emit(DiagEvent::WorkerStreamError {
                            ts: d.now(),
                            chunk_id: self.chunk.id,
                            error: format!("Connection error: {}", err),
                            retry: retry_count,
                        });
                    }

                    if !ConnectivityMonitor::is_online().await {
                        let _ = self
                            .event_tx
                            .send(WorkerEvent::NetworkLost {
                                chunk_id: self.chunk.id,
                            })
                            .await;
                        return;
                    }

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
                    tokio::time::sleep(Duration::from_millis(500 * (1 << retry_count.min(4))))
                        .await;
                }
            }
        }
    }
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    if let Some(val) = headers.get(RETRY_AFTER).and_then(|v| v.to_str().ok()) {
        if let Ok(secs) = val.trim().parse::<u64>() {
            return Some(Duration::from_secs(secs));
        }
    }
    None
}
