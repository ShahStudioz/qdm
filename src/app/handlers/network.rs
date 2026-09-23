//! Handles network connectivity monitoring and disk synchronization.
//!
//! This module processes periodic connectivity checks, resumes downloads
//! when the network comes back online, and detects files that were deleted
//! externally from the download directory.

use crate::app::{Message, QdmApp};
use crate::models::download::DownloadState;
use crate::services::shared::storage;
use iced::Task;

/// Triggers an async network connectivity probe.
pub(crate) fn handle_check_connectivity(_app: &mut QdmApp) -> Task<Message> {
    Task::perform(
        crate::services::shared::network::connectivity::ConnectivityMonitor::is_online(),
        Message::NetworkConnectivityResult,
    )
}

/// Handles the result of a connectivity check. If online, resumes any
/// downloads that were waiting for network.
pub(crate) fn handle_connectivity_result(app: &mut QdmApp, is_online: bool) -> Task<Message> {
    if is_online {
        let mut resume_tasks = Vec::new();
        for item in &mut app.downloads {
            if matches!(item.state, DownloadState::WaitingForNetwork { .. }) {
                println!(
                    "[QDM Network] Connectivity restored! Resuming download {}...",
                    item.id
                );
                item.state = DownloadState::Downloading {
                    downloaded_bytes: item.downloaded_bytes,
                    total_bytes: item.total_bytes,
                    speed_bps: 0,
                    eta_secs: None,
                };
                let item_clone = item.clone();
                let engine = app.engine.clone();
                let play_media = app.settings.torrent_play_media_while_downloading;
                resume_tasks.push(Task::perform(
                    async move {
                        engine.start_or_resume(item_clone, play_media).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                ));
            }
        }

        if !resume_tasks.is_empty() {
            let downloads_clone = app.downloads.clone();
            return Task::batch([
                Task::perform(
                    async move { storage::json_store::save_downloads(&downloads_clone) },
                    Message::DownloadsPersisted,
                ),
                Task::batch(resume_tasks),
            ]);
        }
    } else {
        let mut pause_tasks = Vec::new();
        for item in &mut app.downloads {
            if matches!(
                item.state,
                DownloadState::Downloading { .. } | DownloadState::FetchingMetadata
            ) {
                println!(
                    "[QDM Network] Connectivity lost! Setting download {} ({}) to WaitingForNetwork...",
                    item.id, item.filename
                );
                let total_downloaded = item.downloaded_bytes;
                item.state = DownloadState::WaitingForNetwork {
                    downloaded_bytes: total_downloaded,
                    total_bytes: item.total_bytes,
                };
                let id = item.id;
                let engine = app.engine.clone();
                pause_tasks.push(Task::perform(
                    async move {
                        engine.pause_for_network(id).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                ));
            }
        }

        if !pause_tasks.is_empty() {
            let downloads_clone = app.downloads.clone();
            return Task::batch([
                Task::perform(
                    async move { storage::json_store::save_downloads(&downloads_clone) },
                    Message::DownloadsPersisted,
                ),
                Task::batch(pause_tasks),
            ]);
        }
    }
    Task::none()
}

/// Synchronizes the download list with on-disk state. Removes completed
/// downloads whose files were deleted externally — only after a generous
/// grace period to avoid racing with the completion handler.
pub(crate) fn handle_sync_with_disk(app: &mut QdmApp) -> Task<Message> {
    let mut changed = false;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Only remove completed downloads whose target files were deleted externally.
    app.downloads.retain(|d| {
        if !matches!(d.state, DownloadState::Completed) {
            return true; // never touch non-completed items
        }

        // If completed_at is not set, the item literally just completed and the
        // timestamp hasn't been persisted yet — always keep it.
        let completed_at = match d.completed_at {
            Some(ts) => ts,
            None => return true,
        };

        // Grace period: keep any download that completed less than 60 seconds
        // ago.  The disk sync timer fires every 5 s, so 60 s gives 12 full
        // cycles of safety margin.
        if now.saturating_sub(completed_at) < 60 {
            return true;
        }

        // --- File existence checks ---
        let save = std::path::Path::new(&d.save_path);
        let target = save.join(&d.filename);
        let staging_dir = save.join(format!(".qdmdownload_{}", d.id));
        let temp_target = save.join(format!("{}.qdmdownload", d.filename));

        if target.exists() || staging_dir.exists() || temp_target.exists() {
            return true;
        }

        // Torrent filenames often lack an extension (e.g. the torrent "name"
        // field is just a release title) while the actual file on disk has
        // one (e.g. ".mkv").  Do a prefix scan of the directory.
        if !d.filename.contains('.') {
            if let Ok(entries) = std::fs::read_dir(&d.save_path) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(&d.filename) {
                            return true;
                        }
                    }
                }
            }
        }

        println!(
            "[QDM Disk Sync] Removed externally deleted completed file: {}",
            d.filename
        );
        changed = true;
        false
    });

    if changed {
        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
