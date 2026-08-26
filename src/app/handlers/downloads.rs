//! Handles direct user actions on downloads.
//!
//! This module processes pause/resume toggles, cancellations, folder opening,
//! mirror modal opening, and post-save/metadata-fetch lifecycle logic.

use crate::app::{Message, QdmApp};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::services::downloads::FileMetadata;
use crate::services::storage;
use crate::views::dialogues::{add_dialogue, delete_dialogue};
use crate::views::settings::settings;
use iced::Task;

/// Handles toggling pause/resume for a download, including queue synchronization.
///
/// The behavior depends on the current download state:
/// - **Downloading/WaitingForNetwork/FetchingMetadata** → Pause and promote next queued
/// - **Paused/Failed/Scheduled** → Resume and synchronize (may preempt lowest-priority)
/// - **Queued** → Force-resume overriding queue order (protects from self-preemption)
/// - **Completed** → No-op
pub(crate) fn handle_toggle_pause(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        match &item.state {
            DownloadState::Downloading { .. }
            | DownloadState::WaitingForNetwork { .. }
            | DownloadState::FetchingMetadata => {
                let bytes = item.downloaded_bytes;
                let total = item.total_bytes;
                item.state = DownloadState::Paused {
                    downloaded_bytes: bytes,
                    total_bytes: total,
                };
                let engine = app.engine.clone();
                let pause_task = Task::perform(
                    async move {
                        engine.pause(id).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );

                // Promote next queued item into the free slot
                let sync_task = app.synchronize_and_persist_queue();
                return Task::batch([pause_task, sync_task]);
            }
            DownloadState::Paused { .. }
            | DownloadState::Failed { .. }
            | DownloadState::Scheduled => {
                let bytes = item.downloaded_bytes;
                let total = item.total_bytes;
                item.state = DownloadState::Downloading {
                    downloaded_bytes: bytes,
                    total_bytes: total,
                    speed_bps: 0,
                    eta_secs: None,
                };
                let item_clone = item.clone();
                let engine = app.engine.clone();
                let start_task = Task::perform(
                    async move {
                        engine.start_or_resume(item_clone).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );

                // Synchronize queue: if limit exceeded, preempts lowest-priority running item
                let sync_task = app.synchronize_and_persist_queue();
                return Task::batch([start_task, sync_task]);
            }
            DownloadState::Queued => {
                // User explicitly force-resumes a Queued download (override queue order)
                let bytes = item.downloaded_bytes;
                let total = item.total_bytes;
                item.state = DownloadState::Downloading {
                    downloaded_bytes: bytes,
                    total_bytes: total,
                    speed_bps: 0,
                    eta_secs: None,
                };
                let item_clone = item.clone();
                let engine = app.engine.clone();
                let start_task = Task::perform(
                    async move {
                        engine.start_or_resume(item_clone).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );

                // Protect this item so it won't preempt itself; preempt lowest of
                // the OTHER running items instead
                let sync_task = app.synchronize_and_persist_queue_with_protected(Some(id));
                return Task::batch([start_task, sync_task]);
            }
            DownloadState::Completed => {}
        }
    }
    Task::none()
}

/// Handles download cancellation, either using a remembered preference or
/// opening the delete confirmation dialog.
pub(crate) fn handle_cancel_download(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(action) = app.settings.delete_action {
        // Apply remembered preference immediately
        let item_opt = app.downloads.iter().find(|d| d.id == id).cloned();
        app.downloads.retain(|d| d.id != id);
        let engine = app.engine.clone();
        let cancel_task = Task::perform(
            async move {
                engine.cancel(id).await;
                if action == settings::DeleteAction::DeleteFromDisk {
                    if let Some(item) = item_opt {
                        let target =
                            std::path::Path::new(&item.save_path).join(&item.filename);
                        let temp_target = std::path::Path::new(&item.save_path)
                            .join(format!("{}.qdmdownload", item.filename));
                        if target.exists() {
                            let _ = std::fs::remove_file(target);
                        }
                        if temp_target.exists() {
                            let _ = std::fs::remove_file(temp_target);
                        }
                    }
                }
                Ok(())
            },
            |_: Result<(), String>| Message::Tick,
        );
        let sync_task = app.synchronize_and_persist_queue();
        return Task::batch([cancel_task, sync_task]);
    }

    // If not remembered, open the confirmation modal
    if let Some(item) = app.downloads.iter().find(|d| d.id == id) {
        app.delete_dialog.open(delete_dialogue::DeletePendingItem {
            id: item.id,
            filename: item.filename.clone(),
            save_path: item.save_path.clone(),
        });
    }
    Task::none()
}

/// Opens the save folder for a download using the platform's native file manager.
pub(crate) fn handle_open_folder(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter().find(|d| d.id == id) {
        let folder_path = item.save_path.clone();
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("explorer")
                .arg(&folder_path)
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open")
                .arg(&folder_path)
                .spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(&folder_path)
                .spawn();
        }
    }
    Task::none()
}

/// Opens the mirror management dialog for a download.
pub(crate) fn handle_open_mirrors_modal(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter().find(|d| d.id == id) {
        app.mirror_dialog.open(
            item.id,
            item.filename.clone(),
            item.primary_url.clone(),
            item.mirror_urls.clone(),
        );
    }
    Task::none()
}

/// Handles the result of saving a new download to storage, adding it to the
/// download list and starting it or fetching metadata as appropriate.
pub(crate) fn handle_download_saved(
    app: &mut QdmApp,
    result: Result<DownloadItem, String>,
) -> Task<Message> {
    match result {
        Ok(inserted_item) => {
            let item_id = inserted_item.id;
            let url = inserted_item.primary_url.url.clone();
            let is_scheduled = inserted_item.is_scheduled;
            let needs_bg_meta = matches!(inserted_item.state, DownloadState::FetchingMetadata)
                || (inserted_item.total_bytes.is_none() && !is_scheduled);

            app.downloads.push(inserted_item.clone());

            if is_scheduled {
                let now = chrono::Local::now().naive_local();
                if app.settings.schedule.is_in_active_window(now) {
                    return app.synchronize_and_persist_queue();
                } else {
                    return Task::none();
                }
            } else if needs_bg_meta {
                let engine = app.engine.clone();
                return Task::perform(
                    async move { (item_id, engine.probe_metadata(&url).await) },
                    |(id, res)| Message::BackgroundMetadataFetched(id, res),
                );
            } else {
                return app.synchronize_and_persist_queue();
            }
        }
        Err(err) => {
            println!("[QDM Storage Error] Failed to save download: {}", err);
        }
    }
    Task::none()
}

/// Handles the result of a background metadata fetch, applying the metadata
/// to the download item and starting the download engine.
pub(crate) fn handle_background_metadata_fetched(
    app: &mut QdmApp,
    id: usize,
    result: Result<FileMetadata, String>,
) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        match result {
            Ok(meta) => {
                if let Some(len) = meta.content_length {
                    item.total_bytes = Some(len);
                }
                item.resumable = meta.supports_resume;
                item.etag = meta.etag;
                item.last_modified = meta.last_modified;

                // Try to improve generic filenames using Content-Disposition
                if item.filename == "download.file" {
                    if let Some(ref cd) = meta.content_disposition {
                        let better_name =
                            add_dialogue::extract_filename(&item.primary_url.url, Some(cd));
                        if better_name != "download.file" {
                            item.filename = better_name;
                            item.file_type = FileType::from_filename(&item.filename);
                        }
                    }
                }
                if matches!(item.state, DownloadState::FetchingMetadata) {
                    item.state = DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    };
                }
            }
            Err(err) => {
                println!("[QDM Background Metadata] Warning for item {}: {}", id, err);
                if matches!(item.state, DownloadState::FetchingMetadata) {
                    item.state = DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: None,
                        speed_bps: 0,
                        eta_secs: None,
                    };
                }
            }
        }

        let item_clone = item.clone();
        let engine = app.engine.clone();
        let downloads_clone = app.downloads.clone();

        return Task::perform(
            async move {
                let _ = storage::json_store::save_downloads(&downloads_clone);
                engine.start_or_resume(item_clone).await;
                Ok(())
            },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
