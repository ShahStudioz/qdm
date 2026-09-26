//! Handles all dialog-related messages.
//!
//! This module processes messages from the add download dialog, conflict
//! resolution dialog, delete confirmation dialog, and mirror management dialog.
//! It also provides a [`build_download_item`] helper for constructing new
//! download items, eliminating duplicated construction code across submission paths.

use crate::app::{Message, QdmApp};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::services::shared::storage;
use crate::views::dialogues::{
    add_dialogue, conflict_dialogue, delete_dialogue, detail_dialogue, mirror_dialogue,
};
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
    pub _url: String,
    pub filename: String,
    pub _mirror_urls: Vec<String>,
    pub save_path: String,
    pub total_bytes: Option<u64>,
    pub state: DownloadState,
    pub is_scheduled: bool,
    pub max_connections: u32,
    pub speed_limit_bps: Option<u64>,
    pub download_type: crate::models::download::DownloadType,
    pub has_media: bool,
}

pub(crate) fn build_download_item(params: NewDownloadParams) -> DownloadItem {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut file_type = FileType::from_filename(&params.filename);
    if params.has_media {
        file_type = FileType::Media;
    }
    DownloadItem {
        id: 0,
        filename: params.filename,
        download_type: params.download_type,
        save_path: params.save_path,
        downloaded_bytes: 0,
        total_bytes: params.total_bytes,
        state: params.state,
        file_type,
        is_scheduled: params.is_scheduled,
        max_connections: params.max_connections,
        speed_limit_bps: params.speed_limit_bps,
        sha256_hash: None,
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
        add_dialogue::AddDialogueModalMessage::SubmitNewDownload => handle_submit_new_download(app),
        add_dialogue::AddDialogueModalMessage::QuickAddDownload => handle_quick_add_download(app),
        add_dialogue::AddDialogueModalMessage::ScheduleNewDownload => {
            handle_schedule_new_download(app)
        }
        other => {
            add_dialogue::update(&mut app.add_dialog, other).map(Message::AddDialogueModalMessages)
        }
    }
}

/// Checks if a download already exists in the app listing by URL/magnet or destination path.
fn find_existing_download<'a>(
    app: &'a QdmApp,
    url: &str,
    save_path: &str,
    filename: &str,
) -> Option<&'a crate::models::download::DownloadItem> {
    let trimmed_url = url.trim();
    let is_magnet = trimmed_url.starts_with("magnet:");

    let magnet_xt = if is_magnet {
        reqwest::Url::parse(trimmed_url).ok().and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "xt")
                .map(|(_, v)| v.to_lowercase())
        })
    } else {
        None
    };

    app.downloads.iter().find(|d| {
        // 1. Check exact URL/magnet match
        let d_url = d.get_url().trim();
        if !d_url.is_empty() {
            if is_magnet {
                if let Some(ref xt) = magnet_xt {
                    if let Ok(u) = reqwest::Url::parse(d_url) {
                        if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "xt") {
                            if v.to_lowercase() == *xt {
                                return true;
                            }
                        }
                    }
                }
                if d_url.eq_ignore_ascii_case(trimmed_url) {
                    return true;
                }
            } else if d_url
                .trim_end_matches('/')
                .eq_ignore_ascii_case(trimmed_url.trim_end_matches('/'))
            {
                return true;
            }
        }

        // 2. Check destination file/folder match
        if !filename.is_empty()
            && filename != "download.file"
            && filename != "Torrent Download"
            && d.save_path == save_path
            && d.filename.eq_ignore_ascii_case(filename)
        {
            return true;
        }

        false
    })
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
        crate::core::utils::paths::sanitize_filename(app.add_dialog.filename.trim())
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
    let parsed_mirrors = app.add_dialog.parsed_mirrors();
    let probe = app.add_dialog.probe_result.clone();

    let (total_bytes, download_type, has_media, is_torrent_folder) = match probe {
        Some(crate::services::engine::ProbeResult::Http(m)) => {
            let dt = crate::models::download::DownloadType::Http(
                crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: m.supports_resume,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                },
            );
            (
                m.content_length,
                dt,
                crate::models::download::FileType::from_filename(&filename)
                    == crate::models::download::FileType::Media,
                false,
            )
        }
        Some(crate::services::engine::ProbeResult::Torrent(info)) => {
            let selected: Vec<usize> = app.add_dialog.selected_files.iter().copied().collect();
            let mut total = 0;
            let mut has_media = false;
            for f in &info.files {
                if selected.is_empty() || selected.contains(&f.id) {
                    total += f.size;
                    if crate::models::download::FileType::from_filename(&f.path)
                        == crate::models::download::FileType::Media
                    {
                        has_media = true;
                    }
                }
            }
            let is_folder = info.files.len() > 1;
            let dt = crate::models::download::DownloadType::Torrent(
                crate::models::download::TorrentMetadata {
                    magnet_uri: url.clone(),
                    info_hash: Some(String::new()),
                    peers_connected: 0,
                    seeds_connected: 0,
                    upload_speed_bps: 0,
                    is_folder,
                    selected_files: if selected.is_empty() {
                        None
                    } else {
                        Some(selected)
                    },
                },
            );
            (Some(total), dt, has_media, is_folder)
        }
        None => {
            let is_torrent = crate::core::utils::paths::is_torrent_target(&url);
            let dt = if is_torrent {
                crate::models::download::DownloadType::Torrent(
                    crate::models::download::TorrentMetadata {
                        magnet_uri: url.clone(),
                        info_hash: Some(String::new()),
                        peers_connected: 0,
                        seeds_connected: 0,
                        upload_speed_bps: 0,
                        is_folder: false,
                        selected_files: None,
                    },
                )
            } else {
                crate::models::download::DownloadType::Http(crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: false,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                })
            };
            (
                None,
                dt,
                crate::models::download::FileType::from_filename(&filename)
                    == crate::models::download::FileType::Media,
                false,
            )
        }
    };

    // --- Duplicate & Conflict resolution ---
    let is_duplicate = find_existing_download(app, &url, &save_path, &filename).is_some();
    let has_conflict = if is_torrent_folder {
        crate::core::utils::paths::folder_exists(&save_path, &filename)
    } else {
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename)
    };

    if is_duplicate || has_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = if is_torrent_folder {
                    crate::core::utils::paths::generate_unique_folder_name(&save_path, &filename)
                } else {
                    crate::core::utils::paths::generate_unique_filename(&save_path, &filename)
                };
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename as is
            }
            None => {
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
                        is_torrent_folder,
                        download_type: Some(download_type),
                        total_bytes,
                        is_duplicate_listing: is_duplicate,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

    let max_connections = app
        .add_dialog
        .max_connections
        .trim()
        .parse::<u32>()
        .unwrap_or(8);

    let new_item = build_download_item(NewDownloadParams {
        _url: url,
        filename,
        _mirror_urls: parsed_mirrors,
        save_path,
        total_bytes,
        state: DownloadState::Downloading {
            downloaded_bytes: 0,
            total_bytes,
            speed_bps: 0,
            eta_secs: None,
        },
        is_scheduled: false,
        max_connections,
        speed_limit_bps,
        download_type,
        has_media,
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

    let parsed_mirrors = app.add_dialog.parsed_mirrors();
    let save_path = app.add_dialog.save_to.clone();
    let speed_limit_bps = app.settings.global_speed_limit_bps();

    let (mut filename, total_bytes, dt, has_media, state, is_torrent_folder) = match &app
        .add_dialog
        .probe_result
    {
        Some(crate::services::engine::ProbeResult::Http(m)) => {
            let fname = if !app.add_dialog.filename.trim().is_empty() {
                crate::core::utils::paths::sanitize_filename(app.add_dialog.filename.trim())
            } else {
                add_dialogue::extract_filename(&url, m.content_disposition.as_deref())
            };
            let has_media = crate::models::download::FileType::from_filename(&fname)
                == crate::models::download::FileType::Media;
            let dt = crate::models::download::DownloadType::Http(
                crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: m.supports_resume,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                },
            );
            (
                fname,
                m.content_length,
                dt,
                has_media,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: m.content_length,
                    speed_bps: 0,
                    eta_secs: None,
                },
                false,
            )
        }
        Some(crate::services::engine::ProbeResult::Torrent(info)) => {
            let fname = if !app.add_dialog.filename.trim().is_empty() {
                crate::core::utils::paths::sanitize_filename(app.add_dialog.filename.trim())
            } else {
                crate::core::utils::paths::sanitize_filename(&info.name)
            };
            let mut total = 0;
            let mut has_media = false;
            for f in &info.files {
                total += f.size;
                if crate::models::download::FileType::from_filename(&f.path)
                    == crate::models::download::FileType::Media
                {
                    has_media = true;
                }
            }
            let is_folder = info.files.len() > 1;
            let dt = crate::models::download::DownloadType::Torrent(
                crate::models::download::TorrentMetadata {
                    magnet_uri: url.clone(),
                    info_hash: Some(String::new()),
                    peers_connected: 0,
                    seeds_connected: 0,
                    upload_speed_bps: 0,
                    is_folder,
                    selected_files: None,
                },
            );
            (
                fname,
                Some(total),
                dt,
                has_media,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: Some(total),
                    speed_bps: 0,
                    eta_secs: None,
                },
                is_folder,
            )
        }
        None => {
            let fname = if !app.add_dialog.filename.trim().is_empty() {
                crate::core::utils::paths::sanitize_filename(app.add_dialog.filename.trim())
            } else {
                add_dialogue::extract_filename(&url, None)
            };
            let has_media = crate::models::download::FileType::from_filename(&fname)
                == crate::models::download::FileType::Media;
            let dt = if crate::core::utils::paths::is_torrent_target(&url) {
                crate::models::download::DownloadType::Torrent(
                    crate::models::download::TorrentMetadata {
                        magnet_uri: url.clone(),
                        info_hash: Some(String::new()),
                        peers_connected: 0,
                        seeds_connected: 0,
                        upload_speed_bps: 0,
                        is_folder: false,
                        selected_files: None,
                    },
                )
            } else {
                crate::models::download::DownloadType::Http(crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: false,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                })
            };
            (
                fname,
                None,
                dt,
                has_media,
                DownloadState::FetchingMetadata,
                false,
            )
        }
    };

    // --- Duplicate & Conflict resolution ---
    let is_duplicate = find_existing_download(app, &url, &save_path, &filename).is_some();
    let has_conflict = if is_torrent_folder {
        crate::core::utils::paths::folder_exists(&save_path, &filename)
    } else {
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename)
    };
    if is_duplicate || has_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = if is_torrent_folder {
                    crate::core::utils::paths::generate_unique_folder_name(&save_path, &filename)
                } else {
                    crate::core::utils::paths::generate_unique_filename(&save_path, &filename)
                };
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename
            }
            None => {
                app.conflict_dialog
                    .open(conflict_dialogue::ConflictPendingDownload {
                        url,
                        filename,
                        save_to: save_path,
                        max_connections: app.settings.max_connections,
                        speed_limit: speed_limit_bps,
                        mirror_urls: parsed_mirrors,
                        is_torrent_folder,
                        download_type: Some(dt),
                        total_bytes,
                        is_duplicate_listing: is_duplicate,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

    let new_item = build_download_item(NewDownloadParams {
        _url: url,
        filename,
        _mirror_urls: parsed_mirrors,
        save_path,
        total_bytes,
        state,
        is_scheduled: false,
        max_connections: app.settings.max_connections as u32,
        speed_limit_bps,
        download_type: dt,
        has_media,
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
        crate::core::utils::paths::sanitize_filename(app.add_dialog.filename.trim())
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
    let parsed_mirrors = app.add_dialog.parsed_mirrors();
    let probe = app.add_dialog.probe_result.clone();

    let (total_bytes, download_type, has_media, is_torrent_folder) = match probe {
        Some(crate::services::engine::ProbeResult::Http(m)) => {
            let dt = crate::models::download::DownloadType::Http(
                crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: m.supports_resume,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                },
            );
            (
                m.content_length,
                dt,
                crate::models::download::FileType::from_filename(&filename)
                    == crate::models::download::FileType::Media,
                false,
            )
        }
        Some(crate::services::engine::ProbeResult::Torrent(info)) => {
            let selected: Vec<usize> = app.add_dialog.selected_files.iter().copied().collect();
            let mut total = 0;
            let mut has_media = false;
            for f in &info.files {
                if selected.is_empty() || selected.contains(&f.id) {
                    total += f.size;
                    if crate::models::download::FileType::from_filename(&f.path)
                        == crate::models::download::FileType::Media
                    {
                        has_media = true;
                    }
                }
            }
            let is_folder = info.files.len() > 1;
            let dt = crate::models::download::DownloadType::Torrent(
                crate::models::download::TorrentMetadata {
                    magnet_uri: url.clone(),
                    info_hash: Some(String::new()),
                    peers_connected: 0,
                    seeds_connected: 0,
                    upload_speed_bps: 0,
                    is_folder,
                    selected_files: if selected.is_empty() {
                        None
                    } else {
                        Some(selected)
                    },
                },
            );
            (Some(total), dt, has_media, is_folder)
        }
        None => {
            let is_torrent = crate::core::utils::paths::is_torrent_target(&url);
            let dt = if is_torrent {
                crate::models::download::DownloadType::Torrent(
                    crate::models::download::TorrentMetadata {
                        magnet_uri: url.clone(),
                        info_hash: Some(String::new()),
                        peers_connected: 0,
                        seeds_connected: 0,
                        upload_speed_bps: 0,
                        is_folder: false,
                        selected_files: None,
                    },
                )
            } else {
                crate::models::download::DownloadType::Http(crate::models::download::HttpMetadata {
                    primary_url: crate::models::download::DownloadUrl::new(&url),
                    mirror_urls: parsed_mirrors
                        .iter()
                        .map(crate::models::download::DownloadUrl::new)
                        .collect(),
                    resumable: false,
                    etag: None,
                    last_modified: None,
                    chunks: Vec::new(),
                })
            };
            (
                None,
                dt,
                crate::models::download::FileType::from_filename(&filename)
                    == crate::models::download::FileType::Media,
                false,
            )
        }
    };

    // --- Duplicate & Conflict resolution ---
    let is_duplicate = find_existing_download(app, &url, &save_path, &filename).is_some();
    let has_conflict = if is_torrent_folder {
        crate::core::utils::paths::folder_exists(&save_path, &filename)
    } else {
        crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename)
    };
    if is_duplicate || has_conflict {
        match app.settings.file_conflict_action {
            Some(settings::FileConflictAction::AutoRename) => {
                filename = if is_torrent_folder {
                    crate::core::utils::paths::generate_unique_folder_name(&save_path, &filename)
                } else {
                    crate::core::utils::paths::generate_unique_filename(&save_path, &filename)
                };
            }
            Some(settings::FileConflictAction::Overwrite) => {
                // Overwrite: keep filename as is
            }
            None => {
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
                        is_torrent_folder,
                        download_type: Some(download_type),
                        total_bytes,
                        is_duplicate_listing: is_duplicate,
                    });
                app.add_dialog.is_open = false;
                app.add_dialog.reset();
                return Task::none();
            }
        }
    }

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
        _url: url,
        filename,
        _mirror_urls: app.add_dialog.parsed_mirrors(),
        save_path,
        total_bytes,
        state: initial_state,
        is_scheduled: true,
        max_connections,
        speed_limit_bps,
        download_type,
        has_media,
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
                if let Some(http) = item.http_meta_mut() {
                    http.primary_url = app.mirror_dialog.primary_url.clone();
                    http.mirror_urls = app.mirror_dialog.mirror_urls.clone();
                }
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
                if let Some(http) = item.http_meta_mut() {
                    http.primary_url = app.mirror_dialog.primary_url.clone();
                    http.mirror_urls = app.mirror_dialog.mirror_urls.clone();
                }
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
                app.settings.file_conflict_action = Some(settings::FileConflictAction::AutoRename);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.conflict_dialog.pending.take() {
                let new_filename = if pending.is_torrent_folder {
                    let mut candidate = crate::core::utils::paths::generate_unique_folder_name(
                        &pending.save_to,
                        &pending.filename,
                    );
                    let mut counter = 1u32;
                    while app.downloads.iter().any(|d| {
                        d.save_path == pending.save_to
                            && d.filename.eq_ignore_ascii_case(&candidate)
                    }) {
                        candidate = format!("{} ({})", pending.filename, counter);
                        counter += 1;
                    }
                    candidate
                } else {
                    let mut candidate = crate::core::utils::paths::generate_unique_filename(
                        &pending.save_to,
                        &pending.filename,
                    );
                    let path = std::path::Path::new(&pending.filename);
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("download");
                    let ext = path.extension().and_then(|s| s.to_str());
                    let mut counter = 1u32;
                    while app.downloads.iter().any(|d| {
                        d.save_path == pending.save_to
                            && d.filename.eq_ignore_ascii_case(&candidate)
                    }) {
                        candidate = match ext {
                            Some(extension) => format!("{} ({}).{}", stem, counter, extension),
                            None => format!("{} ({})", stem, counter),
                        };
                        counter += 1;
                    }
                    candidate
                };

                let dt = if let Some(existing_dt) = pending.download_type {
                    existing_dt
                } else if crate::core::utils::paths::is_torrent_target(&pending.url) {
                    crate::models::download::DownloadType::Torrent(
                        crate::models::download::TorrentMetadata {
                            magnet_uri: pending.url.clone(),
                            info_hash: Some(String::new()),
                            peers_connected: 0,
                            seeds_connected: 0,
                            upload_speed_bps: 0,
                            is_folder: pending.is_torrent_folder,
                            selected_files: None,
                        },
                    )
                } else {
                    crate::models::download::DownloadType::Http(
                        crate::models::download::HttpMetadata {
                            primary_url: crate::models::download::DownloadUrl::new(&pending.url),
                            mirror_urls: pending
                                .mirror_urls
                                .iter()
                                .map(crate::models::download::DownloadUrl::new)
                                .collect(),
                            resumable: false,
                            etag: None,
                            last_modified: None,
                            chunks: Vec::new(),
                        },
                    )
                };

                let state = if pending.total_bytes.is_some() {
                    DownloadState::Downloading {
                        downloaded_bytes: 0,
                        total_bytes: pending.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    }
                } else {
                    DownloadState::FetchingMetadata
                };

                let has_media = crate::models::download::FileType::from_filename(&new_filename)
                    == crate::models::download::FileType::Media;
                let new_item = build_download_item(NewDownloadParams {
                    _url: pending.url.clone(),
                    filename: new_filename,
                    _mirror_urls: pending.mirror_urls.clone(),
                    save_path: pending.save_to.clone(),
                    total_bytes: pending.total_bytes,
                    state,
                    is_scheduled: false,
                    max_connections: pending.max_connections as u32,
                    speed_limit_bps: pending.speed_limit,
                    download_type: dt,
                    has_media,
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
                app.settings.file_conflict_action = Some(settings::FileConflictAction::Overwrite);
                let _ = storage::json_store::save_settings(&app.settings);
            }
            if let Some(pending) = app.conflict_dialog.pending.take() {
                let dt = if let Some(existing_dt) = pending.download_type {
                    existing_dt
                } else if crate::core::utils::paths::is_torrent_target(&pending.url) {
                    crate::models::download::DownloadType::Torrent(
                        crate::models::download::TorrentMetadata {
                            magnet_uri: pending.url.clone(),
                            info_hash: Some(String::new()),
                            peers_connected: 0,
                            seeds_connected: 0,
                            upload_speed_bps: 0,
                            is_folder: pending.is_torrent_folder,
                            selected_files: None,
                        },
                    )
                } else {
                    crate::models::download::DownloadType::Http(
                        crate::models::download::HttpMetadata {
                            primary_url: crate::models::download::DownloadUrl::new(&pending.url),
                            mirror_urls: pending
                                .mirror_urls
                                .iter()
                                .map(crate::models::download::DownloadUrl::new)
                                .collect(),
                            resumable: false,
                            etag: None,
                            last_modified: None,
                            chunks: Vec::new(),
                        },
                    )
                };

                let state = if pending.total_bytes.is_some() {
                    DownloadState::Downloading {
                        downloaded_bytes: 0,
                        total_bytes: pending.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    }
                } else {
                    DownloadState::FetchingMetadata
                };

                let has_media = crate::models::download::FileType::from_filename(&pending.filename)
                    == crate::models::download::FileType::Media;
                let new_item = build_download_item(NewDownloadParams {
                    _url: pending.url.clone(),
                    filename: pending.filename.clone(),
                    _mirror_urls: pending.mirror_urls.clone(),
                    save_path: pending.save_to.clone(),
                    total_bytes: pending.total_bytes,
                    state,
                    is_scheduled: false,
                    max_connections: pending.max_connections as u32,
                    speed_limit_bps: pending.speed_limit,
                    download_type: dt,
                    has_media,
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
                        let target =
                            std::path::Path::new(&pending.save_path).join(&pending.filename);
                        let temp_target = std::path::Path::new(&pending.save_path)
                            .join(format!("{}.qdmdownload", pending.filename));
                        let staging_dir = std::path::Path::new(&pending.save_path)
                            .join(format!(".qdmdownload_{}", pending.id));

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

/// Handles messages from the download details dialog.
pub(crate) fn handle_detail_dialog_message(
    app: &mut QdmApp,
    msg: detail_dialogue::DetailDialogueMessage,
) -> Task<Message> {
    match msg {
        detail_dialogue::DetailDialogueMessage::CloseDetailDialog => {
            app.detail_dialog.close();
            Task::none()
        }
        detail_dialogue::DetailDialogueMessage::SelectTab(tab) => {
            app.detail_dialog.current_tab = tab;
            Task::none()
        }
        detail_dialogue::DetailDialogueMessage::CopyText(content, feedback) => {
            app.detail_dialog.copy_feedback = Some(feedback);
            iced::clipboard::write(content)
        }
        detail_dialogue::DetailDialogueMessage::OpenFolder(id) => {
            crate::app::handlers::downloads::handle_open_folder(app, id)
        }
    }
}

/// Handles messages from the update conflict dialogue (non-resumable active downloads).
pub(crate) fn handle_update_conflict_message(
    app: &mut QdmApp,
    msg: crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogMessage,
) -> Task<Message> {
    match msg {
        crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogMessage::Close => {
            app.update_conflict_dialog.close();
            Task::none()
        }
        crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogMessage::ProceedWithInstall => {
            app.update_conflict_dialog.close();
            // Cancel non-resumable active downloads
            let to_cancel: Vec<usize> = app.downloads
                .iter()
                .filter(|d| !d.download_type.is_update())
                .filter(|d| matches!(d.state, crate::models::download::DownloadState::Downloading { .. }))
                .filter(|d| d.http_meta().map(|h| !h.resumable).unwrap_or(false))
                .map(|d| d.id)
                .collect();

            for id in to_cancel {
                let engine = app.engine.clone();
                tokio::spawn(async move {
                    engine.cancel(id).await;
                });
            }
            crate::app::handlers::navigation::execute_install_update(app)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_find_existing_download_magnet() {
        let mut app = QdmApp::default();
        let item = build_download_item(NewDownloadParams {
            _url: "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335380dc1d7d1ee&dn=archlinux.iso".to_string(),
            filename: "archlinux.iso".to_string(),
            _mirror_urls: Vec::new(),
            save_path: "C:\\Downloads".to_string(),
            total_bytes: None,
            state: DownloadState::Completed,
            is_scheduled: false,
            max_connections: 8,
            speed_limit_bps: None,
            download_type: crate::models::download::DownloadType::Torrent(crate::models::download::TorrentMetadata {
                magnet_uri: "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335380dc1d7d1ee&dn=archlinux.iso".to_string(),
                info_hash: Some("c12fe1c06bba254a9dc9f519b335380dc1d7d1ee".to_string()),
                peers_connected: 0,
                seeds_connected: 0,
                upload_speed_bps: 0,
                is_folder: false,
                selected_files: None,
            }),
            has_media: false,
        });
        app.downloads.push(item);

        // Matching info hash even with different query params or tracker order
        let duplicate_magnet = "magnet:?tr=udp://tracker.opentrackr.org:1337&xt=urn:btih:c12fe1c06bba254a9dc9f519b335380dc1d7d1ee";
        assert!(
            find_existing_download(&app, duplicate_magnet, "C:\\Downloads", "archlinux.iso")
                .is_some()
        );

        // Matching save_path and filename
        assert!(find_existing_download(
            &app,
            "http://example.com/archlinux.iso",
            "C:\\Downloads",
            "archlinux.iso"
        )
        .is_some());

        // Non-duplicate
        assert!(find_existing_download(
            &app,
            "http://example.com/ubuntu.iso",
            "C:\\Downloads",
            "ubuntu.iso"
        )
        .is_none());
    }
}
