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
use crate::services::downloads::{DownloadEngine, EngineUiEvent, FileMetadata};
use crate::services::storage;
use crate::views::components::sidebar;
use crate::views::dialogues::{add_dialogue, conflict_dialogue, delete_dialogue, mirror_dialogue};
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
    BackgroundMetadataFetched(usize, Result<FileMetadata, String>),

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
    OpenFolder(usize),
    OpenMirrorsModal(usize),

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
    ConflictDialogueMessages(conflict_dialogue::ConflictDialogMessage),
    DeleteDialogueMessages(delete_dialogue::DeleteDialogMessage),
}

// ---------------------------------------------------------------------------
// QdmApp
// ---------------------------------------------------------------------------

/// The root application state for Quick Download Manager.
pub struct QdmApp {
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
    pub(crate) conflict_dialog: conflict_dialogue::ConflictDialogModel,
    pub(crate) delete_dialog: delete_dialogue::DeleteDialogModel,

    // --- Engine ---
    pub(crate) engine: DownloadEngine,
}

impl Default for QdmApp {
    fn default() -> Self {
        Self {
            current_filter: sidebar::NavFilter::All,
            search_query: String::new(),
            downloads: Vec::new(),
            retry_counts: HashMap::new(),
            schedule_power_action_triggered: false,
            is_topbar_menu_open: false,
            settings: settings::SettingsModel::default(),
            add_dialog: add_dialogue::AddDialogModel::default(),
            mirror_dialog: mirror_dialogue::MirrorDialogModel::default(),
            conflict_dialog: conflict_dialogue::ConflictDialogModel::default(),
            delete_dialog: delete_dialogue::DeleteDialogModel::default(),
            engine: DownloadEngine::new(),
        }
    }
}

impl QdmApp {
    /// Creates a new QDM application instance, loading settings from disk
    /// and kicking off the initial downloads load.
    pub fn new() -> (Self, Task<Message>) {
        let initial_settings = storage::json_store::load_settings().unwrap_or_default();
        let app = Self {
            settings: initial_settings,
            ..Default::default()
        };

        let task = Task::perform(
            async { storage::json_store::load_downloads() },
            Message::DownloadsLoaded,
        );

        (app, task)
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
        let action = crate::services::downloads::QueueService::synchronize_queue_with_protected(
            &mut self.downloads,
            self.settings.simultaneous_downloads,
            protected_id,
        );

        let mut tasks = Vec::new();
        for item in action.to_start {
            let engine = self.engine.clone();
            tasks.push(Task::perform(
                async move {
                    engine.start_or_resume(item).await;
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
            Message::CheckNetworkConnectivity => {
                handlers::network::handle_check_connectivity(self)
            }
            Message::NetworkConnectivityResult(is_online) => {
                handlers::network::handle_connectivity_result(self, is_online)
            }
            Message::SyncWithDisk => handlers::network::handle_sync_with_disk(self),

            // --- Storage Results ---
            Message::DownloadsLoaded(result) => {
                handlers::navigation::handle_downloads_loaded(self, result)
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
            Message::NavSelected(filter) => {
                handlers::navigation::handle_nav_selected(self, filter)
            }
            Message::SearchChanged(query) => {
                handlers::navigation::handle_search_changed(self, query)
            }
            Message::AddUrlPressed => handlers::navigation::handle_add_url_pressed(self),
            Message::SettingsPressed => handlers::navigation::handle_settings_pressed(self),
            Message::ToggleTopbarMenu => handlers::navigation::handle_toggle_topbar_menu(self),
            Message::OpenQueueView => handlers::navigation::handle_open_queue_view(self),

            // --- Download Actions ---
            Message::TogglePause(id) => handlers::downloads::handle_toggle_pause(self, id),
            Message::CancelDownload(id) => {
                handlers::downloads::handle_cancel_download(self, id)
            }
            Message::OpenFolder(id) => handlers::downloads::handle_open_folder(self, id),
            Message::OpenMirrorsModal(id) => {
                handlers::downloads::handle_open_mirrors_modal(self, id)
            }

            // --- Queue Management ---
            Message::MoveQueueItemUp(id) => {
                handlers::queue::handle_move_queue_item_up(self, id)
            }
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
            Message::ConflictDialogueMessages(msg) => {
                handlers::dialogs::handle_conflict_dialog_message(self, msg)
            }
            Message::DeleteDialogueMessages(msg) => {
                handlers::dialogs::handle_delete_dialog_message(self, msg)
            }
        }
    }
}
