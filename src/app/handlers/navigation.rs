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
    Task::none()
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

            // Automatically resume any downloads that were in active Downloading state
            let mut auto_resume_tasks = Vec::new();
            for item in &app.downloads {
                if matches!(item.state, DownloadState::Downloading { .. }) {
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
                if let Some(folder) = dialog.pick_folder().await {
                    Some(folder.path().to_string_lossy().to_string())
                } else {
                    None
                }
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
            settings::update(
                &mut app.settings,
                msg,
            );
            app.synchronize_and_persist_queue()
        }

        // Torrent play media toggle requires restarting active torrents
        // so their files can be moved in/out of staging dynamically
        settings::SettingsMessage::ToggleTorrentPlayMedia(val) => {
            settings::update(&mut app.settings, settings::SettingsMessage::ToggleTorrentPlayMedia(val));
            
            // Queue active torrents for restart
            let mut tasks = Vec::new();
            for item in app.downloads.iter() {
                if matches!(item.state, crate::models::download::DownloadState::Downloading { .. }) {
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

        // All other settings messages are handled generically
        other => {
            settings::update(&mut app.settings, other);
            Task::none()
        }
    }
}
