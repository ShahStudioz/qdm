//! Handles UI navigation, settings delegation, and app initialization results.
//!
//! This module processes navigation changes, search input, toolbar actions,
//! settings messages, and the results of loading/persisting downloads.

use crate::app::{Message, QdmApp};
use crate::models::download::{DownloadItem, DownloadState};
use crate::views::components::sidebar;
use crate::views::settings::settings;
use iced::Task;

/// Processes the 16ms animation tick.
pub(crate) fn handle_tick(app: &mut QdmApp) -> Task<Message> {
    app.settings.tick_animation();
    Task::none()
}

/// Switches the active sidebar navigation filter.
pub(crate) fn handle_nav_selected(app: &mut QdmApp, filter: sidebar::NavFilter) -> Task<Message> {
    app.current_filter = filter;
    app.is_topbar_menu_open = false;
    Task::none()
}

/// Updates the search query for filtering downloads.
pub(crate) fn handle_search_changed(app: &mut QdmApp, query: String) -> Task<Message> {
    app.search_query = query;
    Task::none()
}

/// Opens the "Add URL" dialog, pre-populated with current settings.
/// Automatically checks the clipboard to paste any valid URL or magnet link.
pub(crate) fn handle_add_url_pressed(app: &mut QdmApp) -> Task<Message> {
    app.is_topbar_menu_open = false;
    app.add_dialog.reset();
    app.add_dialog.save_to = app.settings.download_folder.clone();
    app.add_dialog.max_connections = app.settings.max_connections.to_string();
    if !app.settings.speed_limit_value.is_empty() {
        app.add_dialog.speed_limit = app.settings.speed_limit_value.clone();
        app.add_dialog.speed_unit = app.settings.speed_limit_unit;
    }
    app.add_dialog.engine = Some(app.engine.clone());
    app.add_dialog.is_open = true;

    iced::clipboard::read().map(|opt| {
        Message::AddDialogueModalMessages(
            crate::views::dialogues::add_dialogue::AddDialogueModalMessage::ClipboardContentRead(
                opt,
            ),
        )
    })
}

/// Navigates to the Settings view.
pub(crate) fn handle_settings_pressed(app: &mut QdmApp) -> Task<Message> {
    app.current_filter = sidebar::NavFilter::Settings;
    app.is_topbar_menu_open = false;
    Task::none()
}

/// Toggles the topbar dropdown menu visibility.
pub(crate) fn handle_toggle_topbar_menu(app: &mut QdmApp) -> Task<Message> {
    app.is_topbar_menu_open = !app.is_topbar_menu_open;
    Task::none()
}

/// Navigates to the Queue view.
pub(crate) fn handle_open_queue_view(app: &mut QdmApp) -> Task<Message> {
    app.current_filter = sidebar::NavFilter::Queue;
    app.is_topbar_menu_open = false;
    Task::none()
}

/// Handles the result of loading downloads from disk on startup.
pub(crate) fn handle_downloads_loaded(
    app: &mut QdmApp,
    result: Result<Vec<DownloadItem>, String>,
) -> Task<Message> {
    match result {
        Ok(loaded_items) => {
            app.downloads = loaded_items;

            // Automatically resume HTTP downloads that were in active Downloading state.
            // Torrent downloads are NOT started here — they are handled by
            // TorrentEngineInitialized to avoid a race where the engine isn't ready yet.
            let mut auto_resume_tasks = Vec::new();
            for item in &app.downloads {
                if matches!(item.state, DownloadState::Downloading { .. }) {
                    if let crate::models::download::DownloadType::Torrent(_) = &item.download_type {
                        continue; // Skip torrents — handled by TorrentEngineInitialized
                    }
                    let item_clone = item.clone();
                    let engine = app.engine.clone();
                    let play_media = app.settings.torrent_play_media_while_downloading;
                    auto_resume_tasks.push(Task::perform(
                        async move {
                            engine.start_or_resume(item_clone, play_media).await;
                            Ok(())
                        },
                        |_: Result<(), String>| Message::Tick,
                    ));
                }
            }

            if !auto_resume_tasks.is_empty() {
                return Task::batch(auto_resume_tasks);
            }
        }
        Err(err) => {
            println!("[QDM Storage Error] Failed to load downloads: {}", err);
        }
    }
    Task::none()
}

/// Handles the result of persisting downloads to disk.
pub(crate) fn handle_downloads_persisted(result: Result<(), String>) -> Task<Message> {
    if let Err(err) = result {
        println!("[QDM Storage Error] Failed to persist downloads: {}", err);
    }
    Task::none()
}

/// Dispatches settings messages, with special handling for browse-folder,
/// queue-related re-dispatches, and simultaneous download changes.
pub(crate) fn handle_settings_message(
    app: &mut QdmApp,
    msg: settings::SettingsMessage,
) -> Task<Message> {
    match msg {
        settings::SettingsMessage::BrowseFolderPressed => {
            let current_folder = app.settings.download_folder.clone();
            let task = async move {
                let mut dialog =
                    rfd::AsyncFileDialog::new().set_title("Select Default Download Directory");
                if std::path::Path::new(&current_folder).exists() {
                    dialog = dialog.set_directory(&current_folder);
                }
                dialog
                    .pick_folder()
                    .await
                    .map(|folder| folder.path().to_string_lossy().to_string())
            };
            Task::perform(task, |res| {
                Message::SettingsMessage(settings::SettingsMessage::BrowseFolderResult(res))
            })
        }

        // Re-dispatch schedule management messages to the top-level handler
        settings::SettingsMessage::MoveScheduledItemUp(id) => {
            app.update(Message::MoveScheduledItemUp(id))
        }
        settings::SettingsMessage::MoveScheduledItemDown(id) => {
            app.update(Message::MoveScheduledItemDown(id))
        }
        settings::SettingsMessage::RemoveFromSchedule(id) => {
            app.update(Message::RemoveFromSchedule(id))
        }

        // Simultaneous download count changes require queue re-synchronization
        settings::SettingsMessage::SimultaneousDownloadsInc
        | settings::SettingsMessage::SimultaneousDownloadsDec => {
            settings::update(&mut app.settings, msg);
            app.synchronize_and_persist_queue()
        }

        // Torrent play media toggle requires restarting active torrents
        // so their files can be moved in/out of staging dynamically
        settings::SettingsMessage::ToggleTorrentPlayMedia(val) => {
            settings::update(
                &mut app.settings,
                settings::SettingsMessage::ToggleTorrentPlayMedia(val),
            );

            // Queue active torrents for restart
            let mut tasks = Vec::new();
            for item in app.downloads.iter() {
                if matches!(
                    item.state,
                    crate::models::download::DownloadState::Downloading { .. }
                ) {
                    if let crate::models::download::DownloadType::Torrent(_) = item.download_type {
                        let id = item.id;
                        let engine = app.engine.clone();
                        let item_clone = item.clone();
                        let play_media = app.settings.torrent_play_media_while_downloading;
                        tasks.push(Task::perform(
                            async move {
                                engine.pause(id).await;
                                engine.start_or_resume(item_clone, play_media).await;
                                Ok(())
                            },
                            |_: Result<(), String>| Message::Tick,
                        ));
                    }
                }
            }
            Task::batch(tasks)
        }

        settings::SettingsMessage::ToggleStartup(val) => {
            settings::update(
                &mut app.settings,
                settings::SettingsMessage::ToggleStartup(val),
            );
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::core::utils::platform::set_launch_at_startup(val)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                move |res| {
                    Message::SettingsMessage(
                        settings::SettingsMessage::StartupRegistrationFinished(val, res),
                    )
                },
            )
        }

        settings::SettingsMessage::ResetDefaultsPressed => {
            settings::update(
                &mut app.settings,
                settings::SettingsMessage::ResetDefaultsPressed,
            );
            let enabled = app.settings.launch_at_startup;
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::core::utils::platform::set_launch_at_startup(enabled)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                move |res| {
                    Message::SettingsMessage(
                        settings::SettingsMessage::StartupRegistrationFinished(enabled, res),
                    )
                },
            )
        }

        settings::SettingsMessage::CheckUpdatesPressed => handle_check_for_updates(app),
        settings::SettingsMessage::StartUpdateDownload => handle_start_update_download(app),
        settings::SettingsMessage::CancelUpdateDownload => handle_cancel_update_download(app),
        settings::SettingsMessage::InstallUpdatePressed => handle_install_update_clicked(app),

        // All other settings messages are handled generically
        other => {
            settings::update(&mut app.settings, other);
            Task::none()
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-Update Handlers
// ---------------------------------------------------------------------------

/// Initiates checking for updates against the configured API endpoint.
pub(crate) fn handle_check_for_updates(app: &mut QdmApp) -> Task<Message> {
    app.update_status = crate::services::updater::UpdateStatus::Checking;
    let api_url = app.settings.update_api_url.clone();
    Task::perform(
        async move {
            crate::services::updater::check_for_updates(&api_url, crate::core::version::APP_VERSION)
                .await
        },
        Message::UpdateCheckResult,
    )
}

/// Processes the result of a version check from the update server.
pub(crate) fn handle_update_check_result(
    app: &mut QdmApp,
    res: Result<Option<crate::services::updater::UpdateInfo>, String>,
) -> Task<Message> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match res {
        Ok(Some(info)) => {
            // Check if the update file is already downloaded in ~/.qdm/updates/
            let updates_dir = crate::core::utils::paths::get_updates_dir();
            let target_file = updates_dir.join(&info.file_name);
            let mut already_ready = false;

            if target_file.is_file() {
                if let Ok(meta) = std::fs::metadata(&target_file) {
                    if meta.len() > 0 {
                        let hash_valid = if let Some(ref sha) = info.checksum_sha256 {
                            crate::services::updater::verify_file_sha256(&target_file, sha)
                        } else {
                            true
                        };
                        if hash_valid {
                            let _ =
                                crate::services::updater::save_cached_update(&info, &target_file);
                            app.update_status =
                                crate::services::updater::UpdateStatus::ReadyToInstall {
                                    info: info.clone(),
                                    file_path: target_file,
                                    file_size: meta.len(),
                                };
                            already_ready = true;
                        }
                    }
                }
            } else if target_file.is_dir() {
                // If a leftover directory exists from previous buggy runs, clean it up
                let _ = std::fs::remove_dir_all(&target_file);
            }

            if !already_ready {
                app.update_status = crate::services::updater::UpdateStatus::UpdateAvailable {
                    info,
                    checked_at: now,
                };
            }
        }
        Ok(None) => {
            app.update_status = crate::services::updater::UpdateStatus::UpToDate {
                checked_at: now,
                latest_version: crate::core::version::APP_VERSION.to_string(),
            };
        }
        Err(err) => {
            app.update_status = crate::services::updater::UpdateStatus::Error { message: err };
        }
    }
    Task::none()
}

/// Starts downloading the available update file using QDM's HTTP engine.
pub(crate) fn handle_start_update_download(app: &mut QdmApp) -> Task<Message> {
    let info = match &app.update_status {
        crate::services::updater::UpdateStatus::UpdateAvailable { info, .. } => info.clone(),
        _ => return Task::none(),
    };

    let updates_dir = crate::core::utils::paths::get_updates_dir();
    let target_file = updates_dir.join(&info.file_name);
    // If a leftover directory exists from previous buggy runs, clean it up
    if target_file.is_dir() {
        let _ = std::fs::remove_dir_all(&target_file);
    }
    let save_path = updates_dir.to_string_lossy().to_string();

    let download_id = app.downloads.iter().map(|d| d.id).max().unwrap_or(0) + 1000;

    let primary_url = crate::models::download::DownloadUrl::new(info.download_url.clone());
    let http_meta = crate::models::download::HttpMetadata {
        primary_url,
        mirror_urls: Vec::new(),
        resumable: true,
        etag: None,
        last_modified: None,
        chunks: Vec::new(),
    };

    let item = crate::models::download::DownloadItem {
        id: download_id,
        filename: info.file_name.clone(),
        download_type: crate::models::download::DownloadType::Update(http_meta),
        save_path,
        downloaded_bytes: 0,
        total_bytes: info.file_size_bytes,
        state: crate::models::download::DownloadState::Downloading {
            downloaded_bytes: 0,
            total_bytes: info.file_size_bytes,
            speed_bps: 0,
            eta_secs: None,
        },
        file_type: crate::models::download::FileType::from_filename(&info.file_name),
        is_scheduled: false,
        max_connections: 4,
        speed_limit_bps: None,
        sha256_hash: info.checksum_sha256.clone(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        updated_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        completed_at: None,
    };

    app.update_status = crate::services::updater::UpdateStatus::Downloading {
        info,
        download_id,
        downloaded_bytes: 0,
        total_bytes: item.total_bytes,
        speed_bps: 0,
        eta_secs: None,
    };

    // Keep item in app.downloads so engine events can route to it
    app.downloads.push(item.clone());

    let engine = app.engine.clone();
    Task::perform(
        async move {
            engine.start_or_resume(item, false).await;
            Ok(())
        },
        |_: Result<(), String>| Message::Tick,
    )
}

/// Cancels an in-progress update download.
pub(crate) fn handle_cancel_update_download(app: &mut QdmApp) -> Task<Message> {
    if let crate::services::updater::UpdateStatus::Downloading {
        download_id, info, ..
    } = &app.update_status
    {
        let id = *download_id;
        let engine = app.engine.clone();
        app.downloads.retain(|d| d.id != id);
        app.update_status = crate::services::updater::UpdateStatus::UpdateAvailable {
            info: info.clone(),
            checked_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        return Task::perform(
            async move {
                engine.cancel(id).await;
                Ok(())
            },
            |_: Result<(), String>| Message::Tick,
        );
    }
    Task::none()
}

/// Handles click on "Install Update" button with non-resumable active download conflict detection.
pub(crate) fn handle_install_update_clicked(app: &mut QdmApp) -> Task<Message> {
    let active_non_resumable: Vec<_> = app
        .downloads
        .iter()
        .filter(|d| !d.download_type.is_update())
        .filter(|d| {
            matches!(
                d.state,
                crate::models::download::DownloadState::Downloading { .. }
            )
        })
        .filter(|d| d.http_meta().map(|h| !h.resumable).unwrap_or(false))
        .cloned()
        .collect();

    if !active_non_resumable.is_empty() {
        app.update_conflict_dialog.open(active_non_resumable);
        Task::none()
    } else {
        execute_install_update(app)
    }
}

/// Executes the installer and shuts down the application gracefully.
pub(crate) fn execute_install_update(app: &mut QdmApp) -> Task<Message> {
    let file_path = match &app.update_status {
        crate::services::updater::UpdateStatus::ReadyToInstall { file_path, .. } => {
            file_path.clone()
        }
        _ => return Task::none(),
    };

    // Save state before exiting
    let _ = crate::services::shared::storage::json_store::save_downloads(&app.downloads);

    match crate::core::utils::platform::launch_installer(&file_path) {
        Ok(()) => {
            std::process::exit(0);
        }
        Err(e) => {
            app.update_status = crate::services::updater::UpdateStatus::Error {
                message: format!("Failed to launch installer: {}", e),
            };
            Task::none()
        }
    }
}

/// Navigates the UI directly to the Updates settings tab.
pub(crate) fn handle_open_update_tab(app: &mut QdmApp) -> Task<Message> {
    app.current_filter = crate::views::components::sidebar::NavFilter::Settings;
    app.settings.active_tab = crate::views::settings::settings::SettingsTab::Updates;
    Task::none()
}
