//! Handles all dialog-related messages.
//!
//! This module processes messages from the add download dialog, conflict
//! resolution dialog, delete confirmation dialog, and mirror management dialog.
//! It also provides a [`build_download_item`] helper for constructing new
//! download items, eliminating duplicated construction code across submission paths.

use crate::app::{Message, QdmApp};
use crate::models::download::{DownloadItem, DownloadState, DownloadUrl, FileType};
use crate::services::storage;
use crate::views::dialogues::{add_dialogue, conflict_dialogue, delete_dialogue, mirror_dialogue};
use crate::views::settings::settings;
use iced::Task;

// ---------------------------------------------------------------------------
// Download Item Builder
// ---------------------------------------------------------------------------

/// Parameters for constructing a new [`DownloadItem`].
///
/// Used by [`build_download_item`] to create download items with consistent
/// defaults across the submit, quick-add, schedule, and conflict-resolution paths.
pub(crate) struct NewDownloadParams {
    pub url: String,
    pub filename: String,
    pub mirror_urls: Vec<String>,
    pub save_path: String,
    pub total_bytes: Option<u64>,
    pub state: DownloadState,
    pub resumable: bool,
    pub is_scheduled: bool,
    pub max_connections: u32,
    pub speed_limit_bps: Option<u64>,
}

/// Builds a new [`DownloadItem`] from the given parameters.
///
/// Automatically sets `id` to 0 (assigned by storage on insert), derives
/// `file_type` from the filename, converts mirror URL strings to [`DownloadUrl`]
/// values, and timestamps `created_at`/`updated_at` to now.
pub(crate) fn build_download_item(params: NewDownloadParams) -> DownloadItem {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_type = FileType::from_filename(&params.filename);
    let mirror_urls: Vec<DownloadUrl> = params
        .mirror_urls
        .into_iter()
        .map(|u| DownloadUrl::new(&u))
        .collect();

    DownloadItem {
        id: 0,
        filename: params.filename,
        primary_url: DownloadUrl::new(&params.url),
        mirror_urls,
        save_path: params.save_path,
        downloaded_bytes: 0,
        total_bytes: params.total_bytes,
        state: params.state,
        file_type,
        resumable: params.resumable,
        is_scheduled: params.is_scheduled,
        max_connections: params.max_connections,
        speed_limit_bps: params.speed_limit_bps,
        etag: None,
        last_modified: None,
        sha256_hash: None,
        chunks: Vec::new(),
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

// ---------------------------------------------------------------------------
// Add Download Dialog
// ---------------------------------------------------------------------------

/// Dispatches messages from the "Add Download" dialog.
pub(crate) fn handle_add_dialog_message(
    app: &mut QdmApp,
    msg: add_dialogue::AddDialogueModalMessage,
) -> Task<Message> {
    match msg {
        add_dialogue::AddDialogueModalMessage::SubmitNewDownload => {
            handle_submit_new_download(app)
        }
        add_dialogue::AddDialogueModalMessage::QuickAddDownload => {
            handle_quick_add_download(app)
        }
        add_dialogue::AddDialogueModalMessage::ScheduleNewDownload => {
            handle_schedule_new_download(app)
        }
        other => add_dialogue::update(&mut app.add_dialog, other)
            .map(Message::AddDialogueModalMessages),
    }
}

/// Handles full download submission with pre-fetched metadata.
fn handle_submit_new_download(app: &mut QdmApp) -> Task<Message> {
    let url = app.add_dialog.url.trim().to_string();
    if url.is_empty() {
        return Task::none();
    }

    let mut filename = if app.add_dialog.filename.trim().is_empty() {
        add_dialogue::extract_filename(&url, None)
    } else {
        app.add_dialog.filename.trim().to_string()
    };

    let speed_limit_bps = if !app.add_dialog.speed_limit.trim().is_empty() {
        app.add_dialog
            .speed_limit
            .trim()
            .parse::<u64>()
            .ok()
            .map(|v| app.add_dialog.speed_unit.to_bps(v))
    } else {
        app.settings.global_speed_limit_bps()
    };

    let save_path = app.add_dialog.save_to.clone();

    // --- File conflict resolution ---
    let file_conflict =
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);
    if file_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = crate::core::utils::paths::generate_unique_filename(
                    &save_path, &filename,
                );
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename as is
            }
            None => {
                let parsed_mirrors = app.add_dialog.parsed_mirrors();
                let max_connections = app
                    .add_dialog
                    .max_connections
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(8);
                app.conflict_dialog
                    .open(conflict_dialogue::ConflictPendingDownload {
                        url,
                        filename,
                        save_to: save_path,
                        max_connections,
                        speed_limit: speed_limit_bps,
                        mirror_urls: parsed_mirrors,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

    let total_bytes = app
        .add_dialog
        .download_file_metadata
        .as_ref()
        .and_then(|m| m.content_length);

    let resumable = app
        .add_dialog
        .download_file_metadata
        .as_ref()
        .map(|m| m.supports_resume)
        .unwrap_or(false);

    let max_connections = app
        .add_dialog
        .max_connections
        .trim()
        .parse::<u32>()
        .unwrap_or(8);

    let new_item = build_download_item(NewDownloadParams {
        url,
        filename,
        mirror_urls: app.add_dialog.parsed_mirrors(),
        save_path,
        total_bytes,
        state: DownloadState::Downloading {
            downloaded_bytes: 0,
            total_bytes,
            speed_bps: 0,
            eta_secs: None,
        },
        resumable,
        is_scheduled: false,
        max_connections,
        speed_limit_bps,
    });

    app.add_dialog.is_open = false;
    app.add_dialog.reset();

    Task::perform(
        async move { storage::json_store::insert_download(new_item) },
        Message::DownloadSaved,
    )
}

/// Handles quick-add download (no metadata pre-fetch, starts with FetchingMetadata state).
fn handle_quick_add_download(app: &mut QdmApp) -> Task<Message> {
    let url = app.add_dialog.url.trim().to_string();
    if url.is_empty() {
        return Task::none();
    }

    let mut filename = add_dialogue::extract_filename(&url, None);
    let save_path = app.add_dialog.save_to.clone();
    let speed_limit_bps = app.settings.global_speed_limit_bps();

    // --- File conflict resolution ---
    let file_conflict =
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);
    if file_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = crate::core::utils::paths::generate_unique_filename(
                    &save_path, &filename,
                );
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename
            }
            None => {
                let parsed_mirrors = app.add_dialog.parsed_mirrors();
                app.conflict_dialog
                    .open(conflict_dialogue::ConflictPendingDownload {
                        url,
                        filename,
                        save_to: save_path,
                        max_connections: app.settings.max_connections,
                        speed_limit: speed_limit_bps,
                        mirror_urls: parsed_mirrors,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

    let new_item = build_download_item(NewDownloadParams {
        url,
        filename,
        mirror_urls: app.add_dialog.parsed_mirrors(),
        save_path,
        total_bytes: None,
        state: DownloadState::FetchingMetadata,
        resumable: false,
        is_scheduled: false,
        max_connections: app.settings.max_connections as u32,
        speed_limit_bps,
    });

    app.add_dialog.is_open = false;
    app.add_dialog.reset();

    Task::perform(
        async move { storage::json_store::insert_download(new_item) },
        Message::DownloadSaved,
    )
}

/// Handles scheduling a new download for a future time window.
fn handle_schedule_new_download(app: &mut QdmApp) -> Task<Message> {
    let url = app.add_dialog.url.trim().to_string();
    if url.is_empty() {
        return Task::none();
    }

    let mut filename = if app.add_dialog.filename.trim().is_empty() {
        add_dialogue::extract_filename(&url, None)
    } else {
        app.add_dialog.filename.trim().to_string()
    };

    let speed_limit_bps = if !app.add_dialog.speed_limit.trim().is_empty() {
        app.add_dialog
            .speed_limit
            .trim()
            .parse::<u64>()
            .ok()
            .map(|v| app.add_dialog.speed_unit.to_bps(v))
    } else {
        app.settings.global_speed_limit_bps()
    };

    let save_path = app.add_dialog.save_to.clone();

    // --- File conflict resolution ---
    let file_conflict =
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);
    if file_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = crate::core::utils::paths::generate_unique_filename(
                    &save_path, &filename,
                );
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename as is
            }
            None => {
                let parsed_mirrors = app.add_dialog.parsed_mirrors();
                let max_connections = app
                    .add_dialog
                    .max_connections
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(8);
                app.conflict_dialog
                    .open(conflict_dialogue::ConflictPendingDownload {
                        url,
                        filename,
                        save_to: save_path,
                        max_connections,
                        speed_limit: speed_limit_bps,
                        mirror_urls: parsed_mirrors,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

    let total_bytes = app
        .add_dialog
        .download_file_metadata
        .as_ref()
        .and_then(|m| m.content_length);

    let resumable = app
        .add_dialog
        .download_file_metadata
        .as_ref()
        .map(|m| m.supports_resume)
        .unwrap_or(false);

    let max_connections = app
        .add_dialog
        .max_connections
        .trim()
        .parse::<u32>()
        .unwrap_or(8);

    // Determine initial state based on whether we're in the active schedule window
    let now_dt = chrono::Local::now().naive_local();
    let initial_state = if app.settings.schedule.is_in_active_window(now_dt) {
        DownloadState::Queued
    } else {
        DownloadState::Scheduled
    };

    let new_item = build_download_item(NewDownloadParams {
        url,
        filename,
        mirror_urls: app.add_dialog.parsed_mirrors(),
        save_path,
        total_bytes,
        state: initial_state,
        resumable,
        is_scheduled: true,
        max_connections,
        speed_limit_bps,
    });

    app.add_dialog.is_open = false;
    app.add_dialog.reset();

    Task::perform(
        async move { storage::json_store::insert_download(new_item) },
        Message::DownloadSaved,
    )
}

// ---------------------------------------------------------------------------
// Mirror Dialog
// ---------------------------------------------------------------------------

/// Dispatches messages from the mirror management dialog.
pub(crate) fn handle_mirror_dialog_message(
    app: &mut QdmApp,
    msg: mirror_dialogue::MirrorDialogueMessage,
) -> Task<Message> {
    match msg {
        mirror_dialogue::MirrorDialogueMessage::CloseMirrorDialog
        | mirror_dialogue::MirrorDialogueMessage::SaveAndClose => {
            let target_id = app.mirror_dialog.download_id;
            if let Some(item) = app.downloads.iter_mut().find(|d| d.id == target_id) {
                item.primary_url = app.mirror_dialog.primary_url.clone();
                item.mirror_urls = app.mirror_dialog.mirror_urls.clone();
            }
            app.mirror_dialog.close();

            let downloads_clone = app.downloads.clone();
            Task::perform(
                async move { storage::json_store::save_downloads(&downloads_clone) },
                Message::DownloadsPersisted,
            )
        }
        other => {
            let task = mirror_dialogue::update(&mut app.mirror_dialog, other)
                .map(Message::MirrorDialogueMessages);

            let target_id = app.mirror_dialog.download_id;
            if let Some(item) = app.downloads.iter_mut().find(|d| d.id == target_id) {
                item.primary_url = app.mirror_dialog.primary_url.clone();
                item.mirror_urls = app.mirror_dialog.mirror_urls.clone();
            }

            task
        }
    }
}

// ---------------------------------------------------------------------------
// Conflict Resolution Dialog
// ---------------------------------------------------------------------------

/// Dispatches messages from the file conflict resolution dialog.
pub(crate) fn handle_conflict_dialog_message(
    app: &mut QdmApp,
    msg: conflict_dialogue::ConflictDialogMessage,
) -> Task<Message> {
    match msg {
        conflict_dialogue::ConflictDialogMessage::Close => {
            app.conflict_dialog.close();
            Task::none()
        }
        conflict_dialogue::ConflictDialogMessage::ToggleRemember(val) => {
            app.conflict_dialog.remember_choice = val;
            Task::none()
        }
        conflict_dialogue::ConflictDialogMessage::AutoRenameChosen => {
            if app.conflict_dialog.remember_choice {
                app.settings.file_conflict_action =
                    Some(settings::FileConflictAction::AutoRename);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.conflict_dialog.pending.take() {
                let new_filename = crate::core::utils::paths::generate_unique_filename(
                    &pending.save_to,
                    &pending.filename,
                );
                let new_item = build_download_item(NewDownloadParams {
                    url: pending.url,
                    filename: new_filename,
                    mirror_urls: pending.mirror_urls,
                    save_path: pending.save_to,
                    total_bytes: None,
                    state: DownloadState::FetchingMetadata,
                    resumable: false,
                    is_scheduled: false,
                    max_connections: pending.max_connections as u32,
                    speed_limit_bps: pending.speed_limit,
                });
                app.conflict_dialog.close();
                return Task::perform(
                    async move { storage::json_store::insert_download(new_item) },
                    Message::DownloadSaved,
                );
            }
            app.conflict_dialog.close();
            Task::none()
        }
        conflict_dialogue::ConflictDialogMessage::OverwriteChosen => {
            if app.conflict_dialog.remember_choice {
                app.settings.file_conflict_action =
                    Some(settings::FileConflictAction::Overwrite);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.conflict_dialog.pending.take() {
                let new_item = build_download_item(NewDownloadParams {
                    url: pending.url,
                    filename: pending.filename,
                    mirror_urls: pending.mirror_urls,
                    save_path: pending.save_to,
                    total_bytes: None,
                    state: DownloadState::FetchingMetadata,
                    resumable: false,
                    is_scheduled: false,
                    max_connections: pending.max_connections as u32,
                    speed_limit_bps: pending.speed_limit,
                });
                app.conflict_dialog.close();
                return Task::perform(
                    async move { storage::json_store::insert_download(new_item) },
                    Message::DownloadSaved,
                );
            }
            app.conflict_dialog.close();
            Task::none()
        }
    }
}

// ---------------------------------------------------------------------------
// Delete Confirmation Dialog
// ---------------------------------------------------------------------------

/// Dispatches messages from the delete confirmation dialog.
pub(crate) fn handle_delete_dialog_message(
    app: &mut QdmApp,
    msg: delete_dialogue::DeleteDialogMessage,
) -> Task<Message> {
    match msg {
        delete_dialogue::DeleteDialogMessage::Close => {
            app.delete_dialog.close();
            Task::none()
        }
        delete_dialogue::DeleteDialogMessage::ToggleRemember(val) => {
            app.delete_dialog.remember_choice = val;
            Task::none()
        }
        delete_dialogue::DeleteDialogMessage::RemoveFromListChosen => {
            if app.delete_dialog.remember_choice {
                app.settings.delete_action = Some(settings::DeleteAction::RemoveFromList);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.delete_dialog.pending.take() {
                app.downloads.retain(|d| d.id != pending.id);
                let engine = app.engine.clone();
                app.delete_dialog.close();
                let cancel_task = Task::perform(
                    async move {
                        engine.cancel(pending.id).await;
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );
                let sync_task = app.synchronize_and_persist_queue();
                return Task::batch([cancel_task, sync_task]);
            }
            app.delete_dialog.close();
            Task::none()
        }
        delete_dialogue::DeleteDialogMessage::DeleteFromDiskChosen => {
            if app.delete_dialog.remember_choice {
                app.settings.delete_action = Some(settings::DeleteAction::DeleteFromDisk);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.delete_dialog.pending.take() {
                app.downloads.retain(|d| d.id != pending.id);
                let engine = app.engine.clone();
                app.delete_dialog.close();
                let cancel_task = Task::perform(
                    async move {
                        engine.cancel(pending.id).await;
                        let target = std::path::Path::new(&pending.save_path)
                            .join(&pending.filename);
                        let temp_target = std::path::Path::new(&pending.save_path)
                            .join(format!("{}.qdmdownload", pending.filename));
                        if target.exists() {
                            let _ = std::fs::remove_file(target);
                        }
                        if temp_target.exists() {
                            let _ = std::fs::remove_file(temp_target);
                        }
                        Ok(())
                    },
                    |_: Result<(), String>| Message::Tick,
                );
                let sync_task = app.synchronize_and_persist_queue();
                return Task::batch([cancel_task, sync_task]);
            }
            app.delete_dialog.close();
            Task::none()
        }
    }
}
