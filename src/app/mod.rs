//! QDM Application core — state, messages, and update dispatcher.
//!
//! This module defines the central [`QdmApp`] struct and [`Message`] enum,
//! provides initialization and queue synchronization logic, and dispatches
//! incoming messages to focused handler modules.
//!
//! # Module Layout
//!
//! - [`handlers`] — Message handler functions grouped by domain
//! - [`subscriptions`] — Iced subscription streams (engine, timers, network)
//! - [`view`] — UI rendering (`view()` and `theme()`)

mod handlers;
mod subscriptions;
mod view;

use crate::models::download::DownloadItem;
use crate::services::http::EngineUiEvent;
use crate::services::shared::storage;
use crate::views::components::sidebar;
use crate::views::dialogues::{
    add_dialogue, conflict_dialogue, delete_dialogue, detail_dialogue, mirror_dialogue,
};
use crate::views::settings::settings;
use iced::Task;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

/// All messages that can be sent to the QDM application.
#[derive(Debug, Clone)]
pub enum Message {
    // --- Ticks & Timers ---
    Tick,
    SecondTick,
    CheckNetworkConnectivity,
    NetworkConnectivityResult(bool),
    SyncWithDisk,

    // --- Storage Results ---
    DownloadsLoaded(Result<Vec<DownloadItem>, String>),
    DownloadSaved(Result<DownloadItem, String>),
    DownloadsPersisted(Result<(), String>),
    BackgroundMetadataFetched(usize, Result<crate::services::engine::ProbeResult, String>),
    TorrentEngineInitialized,

    // --- Engine Events ---
    EngineEvent(EngineUiEvent),

    // --- Navigation & UI ---
    NavSelected(sidebar::NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    SettingsPressed,
    ToggleTopbarMenu,
    OpenQueueView,

    // --- Download Actions ---
    TogglePause(usize),
    CancelDownload(usize),
    CopyLink(usize),
    OpenFolder(usize),
    OpenMirrorsModal(usize),
    OpenDownloadDetails(usize),
    ItemClicked(usize),

    // --- Queue Management ---
    MoveQueueItemUp(usize),
    MoveQueueItemDown(usize),
    MoveScheduledItemUp(usize),
    MoveScheduledItemDown(usize),
    RemoveFromSchedule(usize),

    // --- Dialog Messages ---
    SettingsMessage(settings::SettingsMessage),
    AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage),
    MirrorDialogueMessages(mirror_dialogue::MirrorDialogueMessage),
    DetailDialogueMessages(detail_dialogue::DetailDialogueMessage),
    ConflictDialogueMessages(conflict_dialogue::ConflictDialogMessage),
    DeleteDialogueMessages(delete_dialogue::DeleteDialogMessage),
    UpdateConflictMessages(
        crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogMessage,
    ),

    // --- Update Management ---
    #[allow(dead_code)]
    CheckForUpdates,
    UpdateCheckResult(Result<Option<crate::services::updater::UpdateInfo>, String>),
    #[allow(dead_code)]
    StartUpdateDownload,
    #[allow(dead_code)]
    CancelUpdateDownload,
    InstallUpdateClicked,
    #[allow(dead_code)]
    ConfirmInstallUpdateAnyway,
    OpenUpdateTab,

    // --- Window Management ---
    WindowIdRetrieved(Option<iced::window::Id>),
    WindowEvent(iced::window::Id, iced::window::Event),
    WindowDragPressed,
    WindowMinimizePressed,
    WindowToggleMaximizePressed,
    WindowClosePressed,
    WindowMaximizedResult(bool),
    #[allow(dead_code)]
    WindowConfigured,
    TrayTick,

    // --- Single-Instance IPC ---
    SingleInstanceCommand(crate::core::single_instance::SingleInstanceCommand),
}

// ---------------------------------------------------------------------------
// QdmApp
// ---------------------------------------------------------------------------

/// The root application state for Quick Download Manager.
pub struct QdmApp {
    // --- Window State ---
    pub(crate) window_id: Option<iced::window::Id>,
    pub(crate) is_maximized: bool,
    pub(crate) last_title_bar_click: Option<std::time::Instant>,
    pub(crate) tray: Option<crate::services::tray::TrayManager>,

    // --- UI State ---
    pub(crate) current_filter: sidebar::NavFilter,
    pub(crate) search_query: String,
    pub(crate) is_topbar_menu_open: bool,

    // --- Downloads ---
    pub(crate) downloads: Vec<DownloadItem>,
    pub(crate) retry_counts: HashMap<usize, u32>,

    // --- Scheduler ---
    pub(crate) schedule_power_action_triggered: bool,

    // --- Settings & Dialogs ---
    pub(crate) settings: settings::SettingsModel,
    pub(crate) add_dialog: add_dialogue::AddDialogModel,
    pub(crate) mirror_dialog: mirror_dialogue::MirrorDialogModel,
    pub(crate) detail_dialog: detail_dialogue::DetailDialogModel,
    pub(crate) conflict_dialog: conflict_dialogue::ConflictDialogModel,
    pub(crate) delete_dialog: delete_dialogue::DeleteDialogModel,
    pub(crate) update_conflict_dialog:
        crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogModel,

    // --- Auto-Update Subsystem ---
    pub(crate) update_status: crate::services::updater::UpdateStatus,

    // --- Engine ---
    pub(crate) engine: crate::services::engine::AppEngine,

    // --- Transient UI State ---
    /// IDs of download items whose link was recently copied (for icon flash).
    pub(crate) copied_link_ids: std::collections::HashSet<usize>,
    pub(crate) last_item_click: Option<(usize, std::time::Instant)>,
}

impl Default for QdmApp {
    fn default() -> Self {
        Self {
            window_id: None,
            is_maximized: false,
            last_title_bar_click: None,
            current_filter: sidebar::NavFilter::All,
            search_query: String::new(),
            downloads: Vec::new(),
            retry_counts: HashMap::new(),
            schedule_power_action_triggered: false,
            is_topbar_menu_open: false,
            settings: settings::SettingsModel::default(),
            add_dialog: add_dialogue::AddDialogModel::default(),
            mirror_dialog: mirror_dialogue::MirrorDialogModel::default(),
            detail_dialog: detail_dialogue::DetailDialogModel::default(),
            conflict_dialog: conflict_dialogue::ConflictDialogModel::default(),
            delete_dialog: delete_dialogue::DeleteDialogModel::default(),
            update_conflict_dialog: crate::views::dialogues::update_conflict_dialogue::UpdateConflictDialogModel::default(),
            update_status: crate::services::updater::UpdateStatus::default(),
            engine: crate::services::engine::AppEngine::new(crate::services::http::DownloadEngine::new()),
            copied_link_ids: std::collections::HashSet::new(),
            last_item_click: None,
            tray: None,
        }
    }
}

impl QdmApp {
    /// Creates a new QDM application instance, loading settings from disk
    /// and kicking off the initial downloads load.
    pub fn new() -> (Self, Task<Message>) {
        let mut initial_settings = storage::json_store::load_settings().unwrap_or_default();
        let mut startup_reg_task = Task::none();

        if initial_settings.is_fresh_install {
            // Fresh installation setup:
            // 1. Enable launch at startup by default and persist the updated flag
            initial_settings.launch_at_startup = true;
            initial_settings.is_fresh_install = false;
            initial_settings.sync_animations();
            let _ = storage::json_store::save_settings(&initial_settings);

            startup_reg_task = Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        crate::core::utils::platform::set_launch_at_startup(true)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                |res| {
                    Message::SettingsMessage(
                        settings::SettingsMessage::StartupRegistrationFinished(true, res),
                    )
                },
            );
        } else if initial_settings.launch_at_startup {
            // Ensure OS startup command stays synced (e.g. includes `--minimized` flag)
            startup_reg_task = Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        crate::core::utils::platform::set_launch_at_startup(true)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                |res| {
                    Message::SettingsMessage(
                        settings::SettingsMessage::StartupRegistrationFinished(true, res),
                    )
                },
            );
        }

        let tray = if let Some((rgba, width, height)) = crate::icons::load_logo_square_rgba(32) {
            crate::services::tray::TrayManager::new(rgba, width, height)
        } else {
            None
        };

        // Check if an already downloaded and verified update exists in ~/.qdm/updates/
        let mut initial_update_status = crate::services::updater::UpdateStatus::default();
        if let Some((info, file_path)) = crate::services::updater::load_cached_update() {
            let size = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
            initial_update_status = crate::services::updater::UpdateStatus::ReadyToInstall {
                info,
                file_path,
                file_size: size,
            };
        }

        let app = Self {
            settings: initial_settings.clone(),
            tray,
            update_status: initial_update_status.clone(),
            ..Default::default()
        };

        let downloads_task = Task::perform(
            async { storage::json_store::load_downloads() },
            Message::DownloadsLoaded,
        );

        let engine_clone = app.engine.clone();
        let default_dir = std::path::PathBuf::from(initial_settings.download_folder.clone());
        let torrent_task = Task::perform(
            async move {
                let _ = engine_clone.init_torrent(default_dir).await;
            },
            |_| Message::TorrentEngineInitialized,
        );

        let window_task = iced::window::get_latest().map(Message::WindowIdRetrieved);

        let update_check_task = if initial_settings.auto_check_updates
            && !initial_update_status.is_ready_to_install()
        {
            let api_url = initial_settings.update_api_url.clone();
            Task::perform(
                async move {
                    crate::services::updater::check_for_updates(
                        &api_url,
                        crate::core::version::APP_VERSION,
                    )
                    .await
                },
                Message::UpdateCheckResult,
            )
        } else {
            Task::none()
        };

        let initial_arg_task = if let Some(arg) = crate::core::single_instance::take_initial_arg() {
            Task::perform(async move { arg }, |url| {
                Message::SingleInstanceCommand(
                    crate::core::single_instance::SingleInstanceCommand::Open(url),
                )
            })
        } else {
            Task::none()
        };

        (
            app,
            Task::batch([
                startup_reg_task,
                downloads_task,
                torrent_task,
                window_task,
                update_check_task,
                initial_arg_task,
            ]),
        )
    }

    // -----------------------------------------------------------------------
    // Queue Synchronization
    // -----------------------------------------------------------------------

    /// Synchronizes the download queue with the engine, optionally protecting
    /// a specific download from being preempted.
    ///
    /// This is called after any state change that could affect which downloads
    /// should be actively running (completions, pauses, cancels, limit changes).
    pub fn synchronize_and_persist_queue_with_protected(
        &mut self,
        protected_id: Option<usize>,
    ) -> Task<Message> {
        let action = crate::services::http::QueueService::synchronize_queue_with_protected(
            &mut self.downloads,
            self.settings.simultaneous_downloads,
            protected_id,
        );

        let mut tasks = Vec::new();
        for item in action.to_start {
            let engine = self.engine.clone();
            let play_media = self.settings.torrent_play_media_while_downloading;
            tasks.push(Task::perform(
                async move {
                    engine.start_or_resume(item, play_media).await;
                    Ok(())
                },
                |_: Result<(), String>| Message::Tick,
            ));
        }

        for id in action.to_pause {
            let engine = self.engine.clone();
            tasks.push(Task::perform(
                async move {
                    engine.pause(id).await;
                    Ok(())
                },
                |_: Result<(), String>| Message::Tick,
            ));
        }

        let downloads_clone = self.downloads.clone();
        tasks.push(Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        ));

        Task::batch(tasks)
    }

    /// Synchronizes the download queue with the engine (no protected downloads).
    pub fn synchronize_and_persist_queue(&mut self) -> Task<Message> {
        self.synchronize_and_persist_queue_with_protected(None)
    }

    // -----------------------------------------------------------------------
    // Update Dispatcher
    // -----------------------------------------------------------------------

    /// Central message dispatcher.
    ///
    /// Routes each [`Message`] variant to the appropriate handler module,
    /// keeping this function as a thin routing layer.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // --- Ticks & Animation ---
            Message::Tick => handlers::navigation::handle_tick(self),
            Message::SecondTick => handlers::queue::handle_second_tick(self),

            // --- Network Monitoring ---
            Message::CheckNetworkConnectivity => handlers::network::handle_check_connectivity(self),
            Message::NetworkConnectivityResult(is_online) => {
                handlers::network::handle_connectivity_result(self, is_online)
            }
            Message::SyncWithDisk => handlers::network::handle_sync_with_disk(self),

            // --- Storage Results ---
            Message::DownloadsLoaded(result) => {
                handlers::navigation::handle_downloads_loaded(self, result)
            }
            Message::TorrentEngineInitialized => {
                // The torrent engine just finished initializing. Any torrent downloads
                // that were in Downloading state when handle_downloads_loaded ran may
                // have failed to start because the engine wasn't ready yet. Kick them off now.
                let mut tasks = Vec::new();
                for item in &self.downloads {
                    if matches!(
                        item.state,
                        crate::models::download::DownloadState::Downloading { .. }
                    ) {
                        if let crate::models::download::DownloadType::Torrent(_) =
                            &item.download_type
                        {
                            let item_clone = item.clone();
                            let engine = self.engine.clone();
                            let play_media = self.settings.torrent_play_media_while_downloading;
                            tasks.push(Task::perform(
                                async move {
                                    engine.start_or_resume(item_clone, play_media).await;
                                    Ok(())
                                },
                                |_: Result<(), String>| Message::Tick,
                            ));
                        }
                    }
                }
                if tasks.is_empty() {
                    Task::none()
                } else {
                    Task::batch(tasks)
                }
            }
            Message::DownloadSaved(result) => {
                handlers::downloads::handle_download_saved(self, result)
            }
            Message::DownloadsPersisted(result) => {
                handlers::navigation::handle_downloads_persisted(result)
            }
            Message::BackgroundMetadataFetched(id, result) => {
                handlers::downloads::handle_background_metadata_fetched(self, id, result)
            }

            // --- Engine Events ---
            Message::EngineEvent(event) => handlers::engine::handle_engine_event(self, event),

            // --- Navigation & UI ---
            Message::NavSelected(filter) => handlers::navigation::handle_nav_selected(self, filter),
            Message::SearchChanged(query) => {
                handlers::navigation::handle_search_changed(self, query)
            }
            Message::AddUrlPressed => handlers::navigation::handle_add_url_pressed(self),
            Message::SettingsPressed => handlers::navigation::handle_settings_pressed(self),
            Message::ToggleTopbarMenu => handlers::navigation::handle_toggle_topbar_menu(self),
            Message::OpenQueueView => handlers::navigation::handle_open_queue_view(self),

            // --- Download Actions ---
            Message::TogglePause(id) => handlers::downloads::handle_toggle_pause(self, id),
            Message::CancelDownload(id) => handlers::downloads::handle_cancel_download(self, id),
            Message::CopyLink(id) => handlers::downloads::handle_copy_link(self, id),
            Message::OpenFolder(id) => handlers::downloads::handle_open_folder(self, id),
            Message::OpenMirrorsModal(id) => {
                handlers::downloads::handle_open_mirrors_modal(self, id)
            }
            Message::OpenDownloadDetails(id) => {
                handlers::downloads::handle_open_download_details(self, id)
            }
            Message::ItemClicked(id) => handlers::downloads::handle_item_clicked(self, id),

            // --- Queue Management ---
            Message::MoveQueueItemUp(id) => handlers::queue::handle_move_queue_item_up(self, id),
            Message::MoveQueueItemDown(id) => {
                handlers::queue::handle_move_queue_item_down(self, id)
            }
            Message::MoveScheduledItemUp(id) => {
                handlers::queue::handle_move_scheduled_item_up(self, id)
            }
            Message::MoveScheduledItemDown(id) => {
                handlers::queue::handle_move_scheduled_item_down(self, id)
            }
            Message::RemoveFromSchedule(id) => {
                handlers::queue::handle_remove_from_schedule(self, id)
            }

            // --- Settings ---
            Message::SettingsMessage(msg) => {
                handlers::navigation::handle_settings_message(self, msg)
            }

            // --- Dialog Messages ---
            Message::AddDialogueModalMessages(msg) => {
                handlers::dialogs::handle_add_dialog_message(self, msg)
            }
            Message::MirrorDialogueMessages(msg) => {
                handlers::dialogs::handle_mirror_dialog_message(self, msg)
            }
            Message::DetailDialogueMessages(msg) => {
                handlers::dialogs::handle_detail_dialog_message(self, msg)
            }
            Message::ConflictDialogueMessages(msg) => {
                handlers::dialogs::handle_conflict_dialog_message(self, msg)
            }
            Message::DeleteDialogueMessages(msg) => {
                handlers::dialogs::handle_delete_dialog_message(self, msg)
            }
            Message::UpdateConflictMessages(msg) => {
                handlers::dialogs::handle_update_conflict_message(self, msg)
            }

            // --- Update Actions ---
            Message::CheckForUpdates => handlers::navigation::handle_check_for_updates(self),
            Message::UpdateCheckResult(res) => {
                handlers::navigation::handle_update_check_result(self, res)
            }
            Message::StartUpdateDownload => {
                handlers::navigation::handle_start_update_download(self)
            }
            Message::CancelUpdateDownload => {
                handlers::navigation::handle_cancel_update_download(self)
            }
            Message::InstallUpdateClicked => {
                handlers::navigation::handle_install_update_clicked(self)
            }
            Message::ConfirmInstallUpdateAnyway => {
                handlers::navigation::execute_install_update(self)
            }
            Message::OpenUpdateTab => handlers::navigation::handle_open_update_tab(self),

            // --- Window Management ---
            Message::WindowIdRetrieved(id_opt) => {
                handlers::window::handle_window_id_retrieved(self, id_opt)
            }
            Message::WindowEvent(id, event) => {
                handlers::window::handle_window_event(self, id, event)
            }
            Message::WindowMaximizedResult(is_max) => {
                handlers::window::handle_window_maximized_result(self, is_max)
            }
            Message::WindowDragPressed => handlers::window::handle_window_drag(self),
            Message::WindowMinimizePressed => handlers::window::handle_window_minimize(self),
            Message::WindowToggleMaximizePressed => {
                handlers::window::handle_window_toggle_maximize(self)
            }
            Message::WindowClosePressed => handlers::window::handle_window_close(self),
            Message::WindowConfigured => Task::none(),
            Message::TrayTick => {
                if let Some(tray) = &self.tray {
                    if let Some(action) = tray.poll_events() {
                        match action {
                            crate::services::tray::TrayAction::Restore => {
                                return handlers::window::handle_restore_from_tray(self);
                            }
                            crate::services::tray::TrayAction::Quit => {
                                return handlers::window::handle_quit_from_tray(self);
                            }
                        }
                    }
                }
                Task::none()
            }
            Message::SingleInstanceCommand(cmd) => {
                let restore_task = handlers::window::handle_restore_from_tray(self);
                match cmd {
                    crate::core::single_instance::SingleInstanceCommand::Focus => restore_task,
                    crate::core::single_instance::SingleInstanceCommand::Open(url) => {
                        let add_task = handlers::navigation::handle_add_url_pressed(self);
                        self.add_dialog.url = url;
                        let probe_task = add_dialogue::update(
                            &mut self.add_dialog,
                            add_dialogue::AddDialogueModalMessage::FetchFileInfoPressed,
                        )
                        .map(Message::AddDialogueModalMessages);
                        Task::batch([restore_task, add_task, probe_task])
                    }
                }
            }
        }
    }
}
