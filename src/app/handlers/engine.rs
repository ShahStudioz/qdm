//! Handles events emitted by the download engine.
//!
//! The download engine sends progress updates, state transitions, completion
//! and failure notifications, and persistence requests through a broadcast
//! channel. This module processes all of those events and updates the
//! application state accordingly.

use crate::app::{Message, QdmApp};
use crate::models::download::DownloadState;
use crate::services::downloads::EngineUiEvent;
use crate::services::storage;
use iced::Task;

/// Processes a single engine UI event and returns any resulting tasks.
///
/// Engine events arrive via the broadcast channel subscription defined in
/// [`super::super::subscriptions`] and drive progress updates, completion/failure
/// transitions, auto-retry logic, and persistence.
pub(crate) fn handle_engine_event(app: &mut QdmApp, event: EngineUiEvent) -> Task<Message> {
    match event {
        EngineUiEvent::ProgressUpdated {
            id,
            downloaded_bytes,
            total_bytes,
            speed_bps,
            eta_secs,
            chunks,
        } => handle_progress_updated(app, id, downloaded_bytes, total_bytes, speed_bps, eta_secs, chunks),

        EngineUiEvent::StateChanged { id, state } => handle_state_changed(app, id, state),

        EngineUiEvent::DownloadCompleted { id, sha256 } => {
            handle_download_completed(app, id, sha256)
        }

        EngineUiEvent::DownloadFailed { id, error } => handle_download_failed(app, id, error),

        EngineUiEvent::PersistRequested { item } => handle_persist_requested(app, item),
    }
}

// ---------------------------------------------------------------------------
// Internal handlers for each engine event variant
// ---------------------------------------------------------------------------

/// Updates download progress, resets retry counters on active transfer,
/// and guards against stale progress events overwriting terminal states.
fn handle_progress_updated(
    app: &mut QdmApp,
    id: usize,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    speed_bps: u64,
    eta_secs: Option<u64>,
    chunks: Vec<crate::models::download::ChunkState>,
) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        item.downloaded_bytes = downloaded_bytes;
        if total_bytes.is_some() {
            item.total_bytes = total_bytes;
        }
        item.chunks = chunks;

        // If stream is actively transferring bytes, reset per-failure auto-retry counter
        if speed_bps > 0 && app.retry_counts.contains_key(&id) {
            app.retry_counts.remove(&id);
        }

        // Guard: Never allow trailing ProgressUpdated events to revert a terminal
        // or queue-managed state
        if matches!(
            item.state,
            DownloadState::Completed
                | DownloadState::Failed { .. }
                | DownloadState::Paused { .. }
                | DownloadState::WaitingForNetwork { .. }
                | DownloadState::Queued
                | DownloadState::Scheduled
        ) {
            return Task::none();
        }

        // If all bytes have been downloaded, transition to Completed
        if let Some(total) = item.total_bytes {
            if total > 0 && downloaded_bytes >= total {
                item.state = DownloadState::Completed;
                return Task::none();
            }
        }

        item.state = DownloadState::Downloading {
            downloaded_bytes,
            total_bytes: item.total_bytes,
            speed_bps,
            eta_secs,
        };
    }
    Task::none()
}

/// Applies a state change from the engine, guarding Queued/Scheduled states
/// that are managed by the UI queue system.
fn handle_state_changed(
    app: &mut QdmApp,
    id: usize,
    state: DownloadState,
) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        println!("[QDM UI] Item {} state changed -> {:?}", id, state);
        // Do not allow engine pause/resume to overwrite Queued or Scheduled state
        // managed by UI queue
        if matches!(item.state, DownloadState::Queued | DownloadState::Scheduled) {
            if matches!(state, DownloadState::Completed | DownloadState::Failed { .. }) {
                item.state = state;
            }
        } else {
            item.state = state;
        }
    }
    Task::none()
}

/// Handles successful download completion: updates state, records SHA-256 hash,
/// timestamps completion, and promotes the next queued download.
fn handle_download_completed(
    app: &mut QdmApp,
    id: usize,
    sha256: Option<String>,
) -> Task<Message> {
    app.retry_counts.remove(&id);
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        println!(
            "[QDM UI] Item {} completed successfully! SHA-256: {:?}",
            id, sha256
        );
        item.state = DownloadState::Completed;
        if let Some(total) = item.total_bytes {
            item.downloaded_bytes = total;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        item.completed_at = Some(now);
        if let Some(hash) = sha256 {
            item.sha256_hash = Some(hash);
        }

        // Automatically promote next queued download
        return app.synchronize_and_persist_queue();
    }
    Task::none()
}

/// Handles download failure with auto-retry support. If retries are enabled
/// and budget remains, schedules a retry after 2 seconds. Otherwise marks
/// the download as failed and persists state.
fn handle_download_failed(app: &mut QdmApp, id: usize, error: String) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        println!("[QDM UI] Item {} failed: {}", id, error);

        // Check auto-retry budget
        if app.settings.auto_retry_downloads {
            let count = app.retry_counts.entry(id).or_insert(0);
            if *count < app.settings.max_auto_retries {
                *count += 1;
                let current_retry = *count;
                println!(
                    "[QDM Auto-Retry] Download {} encountered error, triggering retry {}/{} in 2s...",
                    id, current_retry, app.settings.max_auto_retries
                );

                item.state = DownloadState::Downloading {
                    downloaded_bytes: item.downloaded_bytes,
                    total_bytes: item.total_bytes,
                    speed_bps: 0,
                    eta_secs: None,
                };

                let item_clone = item.clone();
                let engine = app.engine.clone();
                return Task::perform(
                    async move {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        engine.start_or_resume(item_clone).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );
            }
        }

        item.state = DownloadState::Failed {
            downloaded_bytes: item.downloaded_bytes,
            total_bytes: item.total_bytes,
            error,
        };

        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}

/// Handles a persistence request from the engine: updates chunk positions,
/// metadata (etag, last_modified, sha256), and conditionally updates state
/// while guarding queue-managed states.
fn handle_persist_requested(
    app: &mut QdmApp,
    item: crate::models::download::DownloadItem,
) -> Task<Message> {
    if let Some(target) = app.downloads.iter_mut().find(|d| d.id == item.id) {
        // Update persistent metadata and chunk positions
        target.downloaded_bytes = item.downloaded_bytes;
        if item.total_bytes.is_some() {
            target.total_bytes = item.total_bytes;
        }
        target.chunks = item.chunks;
        target.etag = item.etag;
        target.last_modified = item.last_modified;
        target.sha256_hash = item.sha256_hash;

        // Only update state if not currently Queued or Scheduled by the queue manager
        if !matches!(target.state, DownloadState::Queued | DownloadState::Scheduled) {
            match &item.state {
                DownloadState::Paused { .. }
                | DownloadState::Failed { .. }
                | DownloadState::Completed => {
                    target.state = item.state;
                }
                _ => {}
            }
        }

        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
