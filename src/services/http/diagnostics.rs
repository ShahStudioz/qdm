//! # Download Diagnostics Side-Channel
//!
//! Provides a zero-overhead, non-blocking side-channel for emitting
//! fine-grained timestamped events from the worker and task layers.
//!
//! The sender uses `try_send` so it **never** blocks hot download paths.
//! Dropped events (when the receiver is gone) are silently ignored.

#![allow(dead_code)]

use std::time::Instant;
use tokio::sync::mpsc;

// ─── Event types ────────────────────────────────────────────────────────────

/// A precise timestamp relative to the download start.
#[derive(Debug, Clone, Copy)]
pub struct Ts {
    pub micros: u64, // microseconds since the download epoch
}

/// Every kind of event the diagnostics system can emit.
#[derive(Debug, Clone)]
pub enum DiagEvent {
    // ── Worker lifecycle ────────────────────────────────────────────────────
    /// Worker started its HTTP request loop.
    WorkerRequestStart {
        ts: Ts,
        chunk_id: usize,
        range_start: u64,
        range_end: u64,
        retry: u32,
    },
    /// Worker received the first byte of the HTTP response.
    WorkerFirstByte {
        ts: Ts,
        chunk_id: usize,
        status: u16,
    },
    /// Worker pushed N bytes into its write buffer (before disk flush).
    WorkerBytesBuffered {
        ts: Ts,
        chunk_id: usize,
        count: u64,
        current_offset: u64,
        buffer_len: usize,
    },
    /// Worker flushed its write buffer to disk.
    WorkerDiskFlush {
        ts: Ts,
        chunk_id: usize,
        flushed_bytes: usize,
        flush_offset: u64,
    },
    /// Worker's stream returned Ok(None).
    WorkerStreamEnd {
        ts: Ts,
        chunk_id: usize,
        premature: bool, // true if current_offset <= end_byte
        current_offset: u64,
        end_byte: u64,
    },
    /// Worker's stream returned Err.
    WorkerStreamError {
        ts: Ts,
        chunk_id: usize,
        error: String,
        retry: u32,
    },
    /// Worker completed its entire byte range.
    WorkerChunkCompleted {
        ts: Ts,
        chunk_id: usize,
        total_bytes: u64,
    },

    // ── Task controller ─────────────────────────────────────────────────────
    /// Task received a BytesDownloaded event.
    TaskBytesReceived {
        ts: Ts,
        chunk_id: usize,
        count: u64,
        gap_since_last_micros: u64, // µs since previous BytesDownloaded
    },
    /// Task channel depth sampled at the moment of recv.
    TaskChannelDepth {
        ts: Ts,
        depth: usize,
    },
    /// Speed meter calculated a value at a UI tick.
    TaskSpeedCalc {
        ts: Ts,
        samples_in_window: usize,
        oldest_sample_age_micros: u64,
        result_bps: u64,
        used_decay: bool,
    },
    /// A worker was spawned or re-spawned.
    TaskWorkerSpawned {
        ts: Ts,
        chunk_id: usize,
        start_offset: u64,
        end_offset: u64,
    },
    /// A worker was removed (failed or completed).
    TaskWorkerRemoved {
        ts: Ts,
        chunk_id: usize,
        reason: String,
        active_count_after: usize,
    },
}

impl DiagEvent {
    pub fn ts(&self) -> Ts {
        match self {
            DiagEvent::WorkerRequestStart { ts, .. }
            | DiagEvent::WorkerFirstByte { ts, .. }
            | DiagEvent::WorkerBytesBuffered { ts, .. }
            | DiagEvent::WorkerDiskFlush { ts, .. }
            | DiagEvent::WorkerStreamEnd { ts, .. }
            | DiagEvent::WorkerStreamError { ts, .. }
            | DiagEvent::WorkerChunkCompleted { ts, .. }
            | DiagEvent::TaskBytesReceived { ts, .. }
            | DiagEvent::TaskChannelDepth { ts, .. }
            | DiagEvent::TaskSpeedCalc { ts, .. }
            | DiagEvent::TaskWorkerSpawned { ts, .. }
            | DiagEvent::TaskWorkerRemoved { ts, .. } => *ts,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DiagEvent::WorkerRequestStart { .. } => "W:REQ_START",
            DiagEvent::WorkerFirstByte { .. } => "W:FIRST_BYTE",
            DiagEvent::WorkerBytesBuffered { .. } => "W:BUFFERED",
            DiagEvent::WorkerDiskFlush { .. } => "W:DISK_FLUSH",
            DiagEvent::WorkerStreamEnd { .. } => "W:STREAM_END",
            DiagEvent::WorkerStreamError { .. } => "W:STREAM_ERR",
            DiagEvent::WorkerChunkCompleted { .. } => "W:CHUNK_DONE",
            DiagEvent::TaskBytesReceived { .. } => "T:BYTES_RECV",
            DiagEvent::TaskChannelDepth { .. } => "T:CHAN_DEPTH",
            DiagEvent::TaskSpeedCalc { .. } => "T:SPEED_CALC",
            DiagEvent::TaskWorkerSpawned { .. } => "T:SPAWNED",
            DiagEvent::TaskWorkerRemoved { .. } => "T:REMOVED",
        }
    }
}

// ─── Sender handle ──────────────────────────────────────────────────────────

/// A cloneable, non-blocking sender for diagnostic events.
/// `try_send` is used throughout — a full buffer silently drops the event
/// rather than ever blocking a worker.
#[derive(Clone)]
pub struct DiagSender {
    tx: mpsc::Sender<DiagEvent>,
    epoch: Instant,
}

impl DiagSender {
    pub fn new(tx: mpsc::Sender<DiagEvent>, epoch: Instant) -> Self {
        Self { tx, epoch }
    }

    /// Current timestamp relative to the download epoch.
    #[inline]
    pub fn now(&self) -> Ts {
        Ts {
            micros: self.epoch.elapsed().as_micros() as u64,
        }
    }

    /// Fire-and-forget emit — never blocks, silently drops if receiver is gone.
    #[inline]
    pub fn emit(&self, event: DiagEvent) {
        let _ = self.tx.try_send(event);
    }
}

// ─── Channel constructor ─────────────────────────────────────────────────────

/// Creates a diagnostics channel pair. The sender is given to the download
/// engine; the receiver is consumed by the diagnostic binary.
pub fn channel() -> (DiagSender, mpsc::Receiver<DiagEvent>) {
    let (tx, rx) = mpsc::channel(65536); // large buffer — never block workers
    let sender = DiagSender::new(tx, Instant::now());
    (sender, rx)
}
