//! Handles direct user actions on downloads.
//!
//! This module processes pause/resume toggles, cancellations, folder opening,
//! mirror modal opening, and post-save/metadata-fetch lifecycle logic.

use crate::app::{Message, QdmApp};
use crate::models::download::{DownloadItem, DownloadState};
use crate::services::shared::storage;
use crate::views::dialogues::delete_dialogue;
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
                let play_media = app.settings.torrent_play_media_while_downloading;
                let start_task = Task::perform(
                    async move {
                        engine.start_or_resume(item_clone, play_media).await;
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
                let play_media = app.settings.torrent_play_media_while_downloading;
                let start_task = Task::perform(
                    async move {
                        engine.start_or_resume(item_clone, play_media).await;
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
                        let target = std::path::Path::new(&item.save_path)
                            .join(&item.filename);
                        let temp_target = std::path::Path::new(&item.save_path)
                            .join(format!("{}.qdmdownload", item.filename));
                        let staging_dir = std::path::Path::new(&item.save_path)
                            .join(format!(".qdmdownload_{}", item.id));
                            
                        let paths_to_remove = [target, temp_target, staging_dir];
                        for path in &paths_to_remove {
                            if path.exists() {
                                if path.is_dir() {
                                    let _ = std::fs::remove_dir_all(path);
                                } else {
                                    let _ = std::fs::remove_file(path);
                                }
                            }
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

/// Copies the download URL to clipboard and marks the item as recently copied
/// so the UI can flash a checkmark icon for 2 seconds.
pub(crate) fn handle_copy_link(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter().find(|d| d.id == id) {
        let url = item.get_url().to_string();
        app.copied_link_ids.insert(id);
        return iced::clipboard::write(url).map(|_: ()| Message::Tick);
    }
    Task::none()
}

/// Opens the mirror management dialog for a download.
pub(crate) fn handle_open_mirrors_modal(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter().find(|d| d.id == id) {
        app.mirror_dialog.open(
            item.id,
            item.filename.clone(),
            item.http_meta().unwrap().primary_url.clone(),
            item.http_meta().unwrap().mirror_urls.clone(),
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
            let url = inserted_item.get_url().to_string();
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
            } else if matches!(inserted_item.state, DownloadState::Downloading { .. }) {
                // Item already has metadata (e.g. torrent via "Inspect and Configure").
                // The queue sync won't start it because it thinks it's already running.
                // We must explicitly kick off the engine.
                let item_clone = inserted_item.clone();
                let engine = app.engine.clone();
                let play_media = app.settings.torrent_play_media_while_downloading;
                let start_task = Task::perform(
                    async move {
                        engine.start_or_resume(item_clone, play_media).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );
                let sync_task = app.synchronize_and_persist_queue();
                return Task::batch([start_task, sync_task]);
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
    result: Result<crate::services::engine::ProbeResult, String>,
) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        match result {
            Ok(probe) => {
                match probe {
                    crate::services::engine::ProbeResult::Http(meta) => {
                        if let Some(len) = meta.content_length {
                            item.total_bytes = Some(len);
                        }
                        if let Some(http) = item.http_meta_mut() {
                            http.resumable = meta.supports_resume;
                            http.etag = meta.etag;
                            http.last_modified = meta.last_modified;
                        }

                        // Try to improve generic filenames using Content-Disposition
                        if item.filename == "download.file" {
                            if let Some(ref cd) = meta.content_disposition {
                                let better_name =
                                    crate::views::dialogues::add_dialogue::extract_filename(item.get_url(), Some(cd));
                                if better_name != "download.file" {
                                    item.filename = better_name;
                                    item.file_type = crate::models::download::FileType::from_filename(&item.filename);
                                }
                            }
                        }
                    }
                    crate::services::engine::ProbeResult::Torrent(info) => {
                        item.filename = info.name;
                        let total_size: u64 = info.files.iter().map(|f| f.size).sum();
                        item.total_bytes = Some(total_size);
                        if let Some(tmeta) = item.torrent_meta_mut() {
                            tmeta.is_folder = info.is_folder;
                        }
                    }
                }
                
                if matches!(item.state, crate::models::download::DownloadState::FetchingMetadata) {
                    item.state = crate::models::download::DownloadState::Downloading {
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

        let play_media = app.settings.torrent_play_media_while_downloading;
        return Task::perform(
            async move {
                let _ = storage::json_store::save_downloads(&downloads_clone);
                engine.start_or_resume(item_clone, play_media).await;
                Ok(())
            },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
