//! Handles network connectivity monitoring and disk synchronization.
//!
//! This module processes periodic connectivity checks, resumes downloads
//! when the network comes back online, and detects files that were deleted
//! externally from the download directory.

use crate::app::{Message, QdmApp};
use crate::models::download::DownloadState;
use crate::services::storage;
use iced::Task;

/// Triggers an async network connectivity probe.
pub(crate) fn handle_check_connectivity(_app: &mut QdmApp) -> Task<Message> {
    Task::perform(
        crate::services::network::connectivity::ConnectivityMonitor::is_online(),
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
                resume_tasks.push(Task::perform(
                    async move {
                        engine.start_or_resume(item_clone).await;
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
    }
    Task::none()
}

/// Synchronizes the download list with on-disk state. Removes completed
/// downloads whose files were deleted externally, and marks partial downloads
/// as failed if their temp files are missing.
pub(crate) fn handle_sync_with_disk(app: &mut QdmApp) -> Task<Message> {
    let mut changed = false;
    app.downloads.retain(|d| {
        let target = std::path::Path::new(&d.save_path).join(&d.filename);
        if matches!(d.state, DownloadState::Completed)
            && !target.exists()
        {
                println!(
                    "[QDM Disk Sync] Removed externally deleted completed file: {}",
                    d.filename
                );
                changed = true;
                return false;
            }
        true
    });

    for d in &mut app.downloads {
        if !matches!(d.state, DownloadState::Completed) && d.downloaded_bytes > 0 {
            let target = std::path::Path::new(&d.save_path).join(&d.filename);
            let temp_target = std::path::Path::new(&d.save_path)
                .join(format!("{}.qdmdownload", d.filename));
            if !target.exists()
                && !temp_target.exists()
                && !matches!(d.state, DownloadState::Failed { .. })
            {
                println!(
                    "[QDM Disk Sync] Partial download missing on disk: {}",
                    d.filename
                );
                d.state = DownloadState::Failed {
                    downloaded_bytes: 0,
                    total_bytes: d.total_bytes,
                    error: "File removed or missing from destination folder".to_string(),
                };
                d.downloaded_bytes = 0;
                changed = true;
            }
        }
    }

    if changed {
        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
