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

use std::time::Duration;
use reqwest::header::{HeaderMap, IF_MATCH, IF_UNMODIFIED_SINCE, RANGE, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use tokio::sync::{mpsc, watch};

use crate::models::download::ChunkState;
use crate::services::downloads::diagnostics::{DiagEvent, DiagSender};
use crate::services::downloads::writer::PositionalWriter;

/// Events reported by individual chunk workers back to the download task controller.
#[derive(Debug)]
#[allow(dead_code)]
pub enum WorkerEvent {
    BytesDownloaded { chunk_id: usize, count: u64, current_offset: u64 },
    ChunkCompleted { chunk_id: usize },
    FallbackToSingleStream { chunk_id: usize },
    ServerFileChanged { chunk_id: usize, new_etag: Option<String> },
    RateLimited { chunk_id: usize, retry_after: Option<Duration> },
    WorkerFailed { chunk_id: usize, error: String },
}

pub struct ChunkWorker {
    pub chunk: ChunkState,
    pub client: Client,
    pub writer: PositionalWriter,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub event_tx: mpsc::Sender<WorkerEvent>,
    pub pause_rx: watch::Receiver<bool>,
    pub diag: Option<DiagSender>,
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
        etag: Option<String>,
        last_modified: Option<String>,
        event_tx: mpsc::Sender<WorkerEvent>,
        pause_rx: watch::Receiver<bool>,
    ) -> Self {
        Self { chunk, client, writer, etag, last_modified, event_tx, pause_rx, diag: None }
    }

    pub fn with_diag(mut self, diag: DiagSender) -> Self {
        self.diag = Some(diag);
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

        if self.chunk.current_offset > self.chunk.end_byte {
            let _ = self.event_tx.send(WorkerEvent::ChunkCompleted { chunk_id: self.chunk.id }).await;
            return;
        }

        let mut retry_count = 0u32;
        const MAX_RETRIES: u32 = 8;

        loop {
            if *self.pause_rx.borrow() { let _ = self.writer.sync_data(); return; }

            let range_header = format!("bytes={}-{}", self.chunk.current_offset, self.chunk.end_byte);
            let mut req = self.client.get(&self.chunk.url).header(RANGE, range_header);
            if let Some(ref etag) = self.etag { req = req.header(IF_MATCH, etag.clone()); }
            else if let Some(ref lm) = self.last_modified { req = req.header(IF_UNMODIFIED_SINCE, lm.clone()); }

            if let Some(ref d) = self.diag {
                d.emit(DiagEvent::WorkerRequestStart {
                    ts: d.now(), chunk_id: self.chunk.id,
                    range_start: self.chunk.current_offset, range_end: self.chunk.end_byte, retry: retry_count,
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
                        d.emit(DiagEvent::WorkerFirstByte { ts: d.now(), chunk_id: self.chunk.id, status: status.as_u16() });
                    }

                    if status == StatusCode::TOO_MANY_REQUESTS || status == StatusCode::SERVICE_UNAVAILABLE {
                        let retry_after = parse_retry_after(resp.headers());
                        let _ = self.event_tx.send(WorkerEvent::RateLimited { chunk_id: self.chunk.id, retry_after }).await;
                        let _ = self.writer.sync_data();
                        return;
                    }
                    if status == StatusCode::OK {
                        let _ = self.event_tx.send(WorkerEvent::FallbackToSingleStream { chunk_id: self.chunk.id }).await;
                        return;
                    }
                    if status == StatusCode::PRECONDITION_FAILED {
                        let _ = self.event_tx.send(WorkerEvent::ServerFileChanged { chunk_id: self.chunk.id, new_etag: None }).await;
                        return;
                    }
                    if status == StatusCode::RANGE_NOT_SATISFIABLE {
                        if self.chunk.current_offset >= self.chunk.end_byte {
                            let _ = self.event_tx.send(WorkerEvent::ChunkCompleted { chunk_id: self.chunk.id }).await;
                            return;
                        }
                    }
                    if !status.is_success() {
                        let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                            chunk_id: self.chunk.id, error: format!("Server returned error HTTP {}", status),
                        }).await;
                        return;
                    }
                    if let Some(resp_etag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
                        if let Some(ref saved_etag) = self.etag {
                            if saved_etag != resp_etag {
                                let _ = self.event_tx.send(WorkerEvent::ServerFileChanged {
                                    chunk_id: self.chunk.id, new_etag: Some(resp_etag.to_string()),
                                }).await;
                                return;
                            }
                        }
                    }

                    retry_count = 0;
                    let mut stream = resp;
                    let mut write_buffer: Vec<u8> = Vec::with_capacity(256 * 1024);
                    let mut buffer_start_offset = self.chunk.current_offset;
                    let mut total_bytes_this_stream: u64 = 0;
                    const WRITE_BUFFER_THRESHOLD: usize = 256 * 1024;

                    loop {
                        if *self.pause_rx.borrow() {
                            if !write_buffer.is_empty() { let _ = self.writer.write_at(buffer_start_offset, &write_buffer); }
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
                                let max_writable = if self.chunk.current_offset + len > self.chunk.end_byte + 1 {
                                    (self.chunk.end_byte + 1).saturating_sub(self.chunk.current_offset) as usize
                                } else { bytes.len() };

                                if max_writable > 0 {
                                    let slice = &bytes[..max_writable];
                                    write_buffer.extend_from_slice(slice);
                                    self.chunk.current_offset += max_writable as u64;
                                    total_bytes_this_stream += max_writable as u64;

                                    if let Some(ref d) = self.diag {
                                        d.emit(DiagEvent::WorkerBytesBuffered {
                                            ts: d.now(), chunk_id: self.chunk.id,
                                            count: max_writable as u64, current_offset: self.chunk.current_offset,
                                            buffer_len: write_buffer.len(),
                                        });
                                    }

                                    let _ = self.event_tx.send(WorkerEvent::BytesDownloaded {
                                        chunk_id: self.chunk.id, count: max_writable as u64,
                                        current_offset: self.chunk.current_offset,
                                    }).await;

                                    let chunk_done = self.chunk.current_offset > self.chunk.end_byte;
                                    if write_buffer.len() >= WRITE_BUFFER_THRESHOLD || chunk_done {
                                        let flush_offset = buffer_start_offset;
                                        let flush_len = write_buffer.len();
                                        if let Err(err) = self.writer.write_at(flush_offset, &write_buffer) {
                                            let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                                chunk_id: self.chunk.id, error: format!("Disk write failure: {}", err),
                                            }).await;
                                            return;
                                        }
                                        if let Some(ref d) = self.diag {
                                            d.emit(DiagEvent::WorkerDiskFlush {
                                                ts: d.now(), chunk_id: self.chunk.id,
                                                flushed_bytes: flush_len, flush_offset,
                                            });
                                        }
                                        buffer_start_offset = self.chunk.current_offset;
                                        write_buffer.clear();
                                    }
                                }

                                if self.chunk.current_offset > self.chunk.end_byte { break; }
                            }
                            Ok(None) => {
                                if !write_buffer.is_empty() {
                                    if let Err(err) = self.writer.write_at(buffer_start_offset, &write_buffer) {
                                        let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                            chunk_id: self.chunk.id, error: format!("Disk write failure on stream-end flush: {}", err),
                                        }).await;
                                        return;
                                    }
                                    write_buffer.clear();
                                }
                                let premature = self.chunk.current_offset <= self.chunk.end_byte;
                                if let Some(ref d) = self.diag {
                                    d.emit(DiagEvent::WorkerStreamEnd {
                                        ts: d.now(), chunk_id: self.chunk.id, premature,
                                        current_offset: self.chunk.current_offset, end_byte: self.chunk.end_byte,
                                    });
                                }
                                if !premature { break; }
                                retry_count += 1;
                                if retry_count > MAX_RETRIES {
                                    let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                        chunk_id: self.chunk.id,
                                        error: format!("Stream ended prematurely at offset {} (expected end {})",
                                            self.chunk.current_offset, self.chunk.end_byte),
                                    }).await;
                                    return;
                                }
                                tokio::time::sleep(Duration::from_millis(200 * (1u64 << retry_count.min(3)))).await;
                                break;
                            }
                            Err(err) => {
                                if let Some(ref d) = self.diag {
                                    d.emit(DiagEvent::WorkerStreamError {
                                        ts: d.now(), chunk_id: self.chunk.id, error: err.to_string(), retry: retry_count,
                                    });
                                }
                                if !write_buffer.is_empty() {
                                    if let Err(write_err) = self.writer.write_at(buffer_start_offset, &write_buffer) {
                                        let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                            chunk_id: self.chunk.id, error: format!("Disk write failure on retry flush: {}", write_err),
                                        }).await;
                                        return;
                                    }
                                    write_buffer.clear();
                                }
                                retry_count += 1;
                                if retry_count > MAX_RETRIES {
                                    let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                                        chunk_id: self.chunk.id, error: format!("Stream interrupted: {}", err),
                                    }).await;
                                    return;
                                }
                                tokio::time::sleep(Duration::from_millis(500 * (1 << retry_count.min(4)))).await;
                                break;
                            }
                        }
                    }

                    if self.chunk.current_offset > self.chunk.end_byte {
                        self.chunk.is_completed = true;
                        if let Some(ref d) = self.diag {
                            d.emit(DiagEvent::WorkerChunkCompleted {
                                ts: d.now(), chunk_id: self.chunk.id, total_bytes: total_bytes_this_stream,
                            });
                        }
                        let _ = self.event_tx.send(WorkerEvent::ChunkCompleted { chunk_id: self.chunk.id }).await;
                        return;
                    }
                }
                Err(err) => {
                    if let Some(ref d) = self.diag {
                        d.emit(DiagEvent::WorkerStreamError {
                            ts: d.now(), chunk_id: self.chunk.id,
                            error: format!("Connection error: {}", err), retry: retry_count,
                        });
                    }
                    retry_count += 1;
                    if retry_count > MAX_RETRIES {
                        let _ = self.event_tx.send(WorkerEvent::WorkerFailed {
                            chunk_id: self.chunk.id, error: format!("Connection error: {}", err),
                        }).await;
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(500 * (1 << retry_count.min(4)))).await;
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
