//! QDM Speed Diagnostic Binary
//!
//! Downloads http://212.183.159.230/512MB.zip with full diagnostic instrumentation
//! and writes a timestamped event log to speed_debug.log.
//!
//! Run: cargo run --bin speed_debug --release

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::{Duration, Instant};

use reqwest::Client;
use tokio::sync::{mpsc, watch};

use qdm::models::download::{DownloadItem, DownloadState, DownloadUrl, FileType};
use qdm::services::http::diagnostics::{self, DiagEvent};
use qdm::services::http::metadata::MetadataService;
use qdm::services::http::task::{DownloadTaskController, TaskEvent};

const URL: &str = "http://212.183.159.230/512MB.zip";
const SAVE_DIR: &str = ".";
const LOG_PATH: &str = "speed_debug.log";
const STALL_THRESHOLD_MS: u64 = 500;

#[tokio::main]
async fn main() {
    println!("=== QDM Speed Debugger ===");
    println!("URL : {}", URL);
    println!("Log : {}", LOG_PATH);
    println!();

    let client = Client::builder()
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_keepalive(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(15))
        .user_agent("QDM-SpeedDebug/0.1")
        .build().unwrap();

    let meta_service = MetadataService::new(client.clone());
    println!("Probing server metadata...");
    let meta = match meta_service.probe(URL).await {
        Ok(m) => m,
        Err(e) => { eprintln!("Metadata probe failed: {}", e); return; }
    };
    println!("  Content-Length : {:?}", meta.content_length);
    println!("  Resumable      : {}", meta.supports_resume);
    println!("  ETag           : {:?}", meta.etag);
    println!("  Last-Modified  : {:?}", meta.last_modified);
    println!();

    let item = DownloadItem {
        id: 1,
        filename: "512MB_debug.zip".to_string(),
        download_type: qdm::models::download::DownloadType::Http(qdm::models::download::HttpMetadata {
            primary_url: DownloadUrl::new(URL),
            mirror_urls: vec![],
            resumable: meta.supports_resume,
            etag: meta.etag,
            last_modified: meta.last_modified,
            chunks: vec![],
        }),
        save_path: SAVE_DIR.to_string(),
        downloaded_bytes: 0,
        total_bytes: meta.content_length,
        state: DownloadState::Downloading {
            downloaded_bytes: 0, total_bytes: meta.content_length, speed_bps: 0, eta_secs: None
        },
        file_type: FileType::Archive,
        is_scheduled: false,
        max_connections: 8,
        speed_limit_bps: None,
        sha256_hash: None,
        created_at: 0, updated_at: 0, completed_at: None,
    };

    let (diag_sender, mut diag_rx) = diagnostics::channel();
    let (pause_tx, pause_rx) = watch::channel(false);
    let _keep_pause_tx = pause_tx;
    let (task_event_tx, mut task_event_rx) = mpsc::channel::<TaskEvent>(512);

    let controller = DownloadTaskController::new(item, client, pause_rx, task_event_tx)
        .with_diag(diag_sender);

    let run_start = Instant::now();
    println!("Starting download...");
    tokio::spawn(async move { controller.run().await; });

    // Open log file — use std::sync::Mutex but NEVER hold it across .await
    let log_path = LOG_PATH.to_string();
    let diag_task = tokio::spawn(async move {
        let mut log_file = OpenOptions::new()
            .create(true).write(true).truncate(true)
            .open(&log_path)
            .expect("cannot open log file");

        let mut stall_events: Vec<(u64, u64, String)> = Vec::new();
        let mut last_bytes_ts: Option<u64> = None;
        let mut total_events = 0usize;
        let mut stream_end_premature = 0u32;
        let mut stream_errors = 0u32;
        let mut disk_flushes = 0u32;
        let mut speed_zero_ticks = 0u32;
        let mut speed_decay_ticks = 0u32;
        let mut worker_spawns: HashMap<usize, u64> = HashMap::new();
        let mut worker_last_buf: HashMap<usize, u64> = HashMap::new();

        writeln!(log_file, "=== QDM Speed Debug Log ===").unwrap();
        writeln!(log_file, "URL: {}", URL).unwrap();
        writeln!(log_file, "Format: [T+us] LABEL | fields...").unwrap();
        writeln!(log_file, "").unwrap();

        // Consume events — this task owns log_file exclusively, no lock needed
        while let Some(event) = diag_rx.recv().await {
            total_events += 1;
            let label = event.label();

            match &event {
                DiagEvent::WorkerRequestStart { ts, chunk_id, range_start, range_end, retry } => {
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} range={}-{} retry={}",
                        ts.micros, label, chunk_id, range_start, range_end, retry).unwrap();
                    worker_spawns.entry(*chunk_id).or_insert(ts.micros);
                }
                DiagEvent::WorkerFirstByte { ts, chunk_id, status } => {
                    let spawn_ts = worker_spawns.get(chunk_id).copied().unwrap_or(ts.micros);
                    let latency_ms = ts.micros.saturating_sub(spawn_ts) / 1000;
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} status={} req_latency={}ms",
                        ts.micros, label, chunk_id, status, latency_ms).unwrap();
                }
                DiagEvent::WorkerBytesBuffered { ts, chunk_id, count, current_offset, buffer_len } => {
                    let prev = worker_last_buf.get(chunk_id).copied().unwrap_or(ts.micros);
                    let gap_ms = ts.micros.saturating_sub(prev) / 1000;
                    worker_last_buf.insert(*chunk_id, ts.micros);
                    if gap_ms > 100 {
                        writeln!(log_file,
                            "[T+{:>10}us] {:12} | chunk={} count={} offset={} buf={}B  IN-WORKER-GAP={}ms",
                            ts.micros, label, chunk_id, count, current_offset, buffer_len, gap_ms).unwrap();
                    }
                }
                DiagEvent::WorkerDiskFlush { ts, chunk_id, flushed_bytes, flush_offset } => {
                    disk_flushes += 1;
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} flushed={}B at_offset={}",
                        ts.micros, label, chunk_id, flushed_bytes, flush_offset).unwrap();
                }
                DiagEvent::WorkerStreamEnd { ts, chunk_id, premature, current_offset, end_byte } => {
                    if *premature { stream_end_premature += 1; }
                    writeln!(log_file,
                        "[T+{:>10}us] {:12} | chunk={} premature={} offset={} end_byte={} remaining={}B",
                        ts.micros, label, chunk_id, premature,
                        current_offset, end_byte, end_byte.saturating_sub(*current_offset)).unwrap();
                }
                DiagEvent::WorkerStreamError { ts, chunk_id, error, retry } => {
                    stream_errors += 1;
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} retry={} error=\"{}\"",
                        ts.micros, label, chunk_id, retry, error).unwrap();
                }
                DiagEvent::WorkerChunkCompleted { ts, chunk_id, total_bytes } => {
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} total_bytes={}",
                        ts.micros, label, chunk_id, total_bytes).unwrap();
                }
                DiagEvent::TaskBytesReceived { ts, chunk_id, count, gap_since_last_micros } => {
                    let gap_ms = gap_since_last_micros / 1000;
                    if gap_ms >= STALL_THRESHOLD_MS && last_bytes_ts.is_some() {
                        let cause = format!(
                            "no BytesDownloaded for {}ms before chunk {} delivered {} bytes",
                            gap_ms, chunk_id, count);
                        stall_events.push((ts.micros, *gap_since_last_micros, cause));
                        writeln!(log_file, "[T+{:>10}us] *** STALL {:>5}ms *** chunk={} count={}",
                            ts.micros, gap_ms, chunk_id, count).unwrap();
                    }
                    last_bytes_ts = Some(ts.micros);
                    if gap_ms > 50 {
                        writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} count={}B gap={}ms",
                            ts.micros, label, chunk_id, count, gap_ms).unwrap();
                    }
                }
                DiagEvent::TaskSpeedCalc { ts, samples_in_window, oldest_sample_age_micros, result_bps, used_decay } => {
                    if *result_bps == 0 { speed_zero_ticks += 1; }
                    if *used_decay { speed_decay_ticks += 1; }
                    let oldest_ms = oldest_sample_age_micros / 1000;
                    let bps_kb = result_bps / 1024;
                    let flag = if *result_bps == 0 { " <-- ZERO" } else if *used_decay { " <-- DECAY" } else { "" };
                    writeln!(log_file,
                        "[T+{:>10}us] {:12} | samples={} oldest={}ms speed={}KB/s decay={}{}",
                        ts.micros, label, samples_in_window, oldest_ms, bps_kb, used_decay, flag).unwrap();
                }
                DiagEvent::TaskWorkerSpawned { ts, chunk_id, start_offset, end_offset } => {
                    writeln!(log_file, "[T+{:>10}us] {:12} | chunk={} start={} end={}",
                        ts.micros, label, chunk_id, start_offset, end_offset).unwrap();
                }
                _ => {}
            }
        }

        // Summary
        writeln!(log_file, "").unwrap();
        writeln!(log_file, "=== SUMMARY ===").unwrap();
        writeln!(log_file, "Total diag events    : {}", total_events).unwrap();
        writeln!(log_file, "Disk flushes         : {}", disk_flushes).unwrap();
        writeln!(log_file, "Premature stream ends: {}", stream_end_premature).unwrap();
        writeln!(log_file, "Stream errors        : {}", stream_errors).unwrap();
        writeln!(log_file, "Speed=0 ticks        : {}", speed_zero_ticks).unwrap();
        writeln!(log_file, "Speed=decay ticks    : {}", speed_decay_ticks).unwrap();
        writeln!(log_file, "").unwrap();
        writeln!(log_file, "--- Stall Events (gap > {}ms) ---", STALL_THRESHOLD_MS).unwrap();
        if stall_events.is_empty() {
            writeln!(log_file, "  None detected.").unwrap();
        } else {
            for (ts_us, gap_us, cause) in &stall_events {
                writeln!(log_file, "  T+{:.3}s  gap={:.1}ms  {}",
                    *ts_us as f64 / 1e6, *gap_us as f64 / 1000.0, cause).unwrap();
            }
        }
        writeln!(log_file, "").unwrap();
        writeln!(log_file, "DIAGNOSIS:").unwrap();
        if stream_end_premature > 0 {
            writeln!(log_file, "  [!] {} premature stream ends.", stream_end_premature).unwrap();
            writeln!(log_file, "      Server closes connection before end_byte.").unwrap();
            writeln!(log_file, "      Worker re-requests from current_offset -> data gap.").unwrap();
        }
        if stream_errors > 0 {
            writeln!(log_file, "  [!] {} stream errors (network instability).", stream_errors).unwrap();
        }
        if speed_zero_ticks > 0 && stream_end_premature == 0 && stream_errors == 0 {
            writeln!(log_file, "  [!] Speed=0 on {} ticks with no stream errors.", speed_zero_ticks).unwrap();
            writeln!(log_file, "      -> Speed meter decay window issue or channel backpressure.").unwrap();
        }
        if stall_events.is_empty() && speed_zero_ticks == 0 {
            writeln!(log_file, "  [OK] No issues found — download appears healthy.").unwrap();
        }
    });

    // Live progress
    loop {
        match task_event_rx.recv().await {
            Some(TaskEvent::ProgressUpdated { downloaded_bytes, total_bytes, speed_bps, .. }) => {
                let pct = total_bytes.map(|t| downloaded_bytes as f64 / t as f64 * 100.0).unwrap_or(0.0);
                let kb = speed_bps / 1024;
                let elapsed = run_start.elapsed().as_secs_f64();
                print!("\r  [{:>5.1}s] {:.1}%  {:>6} KB/s  {:>5} MB / {:?} MB     ",
                    elapsed, pct, kb, downloaded_bytes / (1024*1024),
                    total_bytes.map(|t| t / (1024*1024)));
                let _ = std::io::stdout().flush();
            }
            Some(TaskEvent::Completed { downloaded_bytes, .. }) => {
                println!("\n  [DONE] {} MB in {:.1}s",
                    downloaded_bytes / (1024*1024), run_start.elapsed().as_secs_f64());
                break;
            }
            Some(TaskEvent::Failed { error, .. }) => {
                println!("\n  [FAILED] {}", error);
                break;
            }
            Some(TaskEvent::WaitingForNetwork { .. }) => {
                println!("\n  [WAITING FOR NETWORK]");
                break;
            }
            None | Some(TaskEvent::StatePersistRequested { .. }) => {}
        }
    }

    println!("\nWaiting for log flush...");
    diag_task.await.unwrap();

    println!("Log: {}", LOG_PATH);
    println!("Search for:");
    println!("  *** STALL       -> exact moment and duration of data gap");
    println!("  premature=true  -> server closed connection before end_byte");
    println!("  <-- ZERO        -> why speed reported 0 (samples/decay state)");
    println!("  W:STREAM_ERR    -> raw error text from network layer");
}
