use crate::icons::{self, icon};
use crate::models::download::{DownloadItem, DownloadState, DownloadUrl, FileType};
use crate::services::downloads::{DownloadEngine, EngineUiEvent, FileMetadata};
use crate::services::storage;
use crate::theme::{colors, styles};
use crate::views::components::{sidebar, toolbar};
use crate::views::dialogues::{add_dialogue, conflict_dialogue, delete_dialogue, mirror_dialogue};
use crate::views::downloads::download_list::download_list_view;
use crate::views::settings::settings;
use iced::widget::{column, container, row, stack, text};
use iced::{Alignment, Element, Length, Subscription, Task, Theme};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    SecondTick,
    CheckNetworkConnectivity,
    NetworkConnectivityResult(bool),
    SyncWithDisk,
    DownloadsLoaded(Result<Vec<DownloadItem>, String>),
    DownloadSaved(Result<DownloadItem, String>),
    DownloadsPersisted(Result<(), String>),
    BackgroundMetadataFetched(usize, Result<FileMetadata, String>),
    EngineEvent(EngineUiEvent),

    NavSelected(sidebar::NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    SettingsPressed,
    ToggleTopbarMenu,
    OpenQueueView,

    TogglePause(usize),
    CancelDownload(usize),
    OpenFolder(usize),
    OpenMirrorsModal(usize),
    MoveQueueItemUp(usize),
    MoveQueueItemDown(usize),
    MoveScheduledItemUp(usize),
    MoveScheduledItemDown(usize),
    RemoveFromSchedule(usize),

    SettingsMessage(settings::SettingsMessage),
    AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage),
    MirrorDialogueMessages(mirror_dialogue::MirrorDialogueMessage),
    ConflictDialogueMessages(conflict_dialogue::ConflictDialogMessage),
    DeleteDialogueMessages(delete_dialogue::DeleteDialogMessage),
}

pub struct QdmApp {
    current_filter: sidebar::NavFilter,
    search_query: String,
    downloads: Vec<DownloadItem>,
    retry_counts: HashMap<usize, u32>,
    schedule_power_action_triggered: bool,
    is_topbar_menu_open: bool,
    settings: settings::SettingsModel,
    add_dialog: add_dialogue::AddDialogModel,
    mirror_dialog: mirror_dialogue::MirrorDialogModel,
    conflict_dialog: conflict_dialogue::ConflictDialogModel,
    delete_dialog: delete_dialogue::DeleteDialogModel,
    engine: DownloadEngine,
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

    pub fn synchronize_and_persist_queue(&mut self) -> Task<Message> {
        self.synchronize_and_persist_queue_with_protected(None)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.settings.tick_animation();
            }
            Message::SecondTick => {
                if self.conflict_dialog.is_open {
                    if self.conflict_dialog.tick_second() {
                        return self.update(Message::ConflictDialogueMessages(
                            conflict_dialogue::ConflictDialogMessage::AutoRenameChosen,
                        ));
                    }
                }

                // 2. Automated Download Scheduler evaluation
                let action = crate::services::schedule::SchedulerService::evaluate_tick(
                    &self.settings.schedule,
                    &self.downloads,
                    self.schedule_power_action_triggered,
                );

                match action {
                    crate::services::schedule::SchedulerTickAction::ResumeDownloads(ids) => {
                        for id in ids {
                            if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                                println!("[QDM Scheduler] Pushing scheduled download {} into queue for active window", id);
                                if matches!(item.state, DownloadState::Scheduled) {
                                    item.state = DownloadState::Queued;
                                }
                            }
                        }
                        return self.synchronize_and_persist_queue();
                    }
                    crate::services::schedule::SchedulerTickAction::PauseDownloads(ids) => {
                        let mut tasks = Vec::new();
                        for id in ids {
                            if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                                println!("[QDM Scheduler] Auto-pausing download {} (schedule window ended)", id);
                                item.state = DownloadState::Scheduled;
                                let engine = self.engine.clone();
                                tasks.push(Task::perform(
                                    async move {
                                        engine.pause(id).await;
                                        Ok(())
                                    },
                                    |_: Result<(), String>| Message::Tick,
                                ));
                            }
                        }
                        if !tasks.is_empty() {
                            let downloads_clone = self.downloads.clone();
                            return Task::batch([
                                Task::perform(
                                    async move { storage::json_store::save_downloads(&downloads_clone) },
                                    Message::DownloadsPersisted,
                                ),
                                Task::batch(tasks),
                            ]);
                        }
                    }
                    crate::services::schedule::SchedulerTickAction::TriggerPowerAction(
                        power_action,
                    ) => {
                        self.schedule_power_action_triggered = true;
                        crate::services::schedule::SchedulerService::execute_power_action(
                            power_action,
                        );
                    }
                    crate::services::schedule::SchedulerTickAction::None => {}
                }
            }
            Message::CheckNetworkConnectivity => {
                return Task::perform(
                    crate::services::network::connectivity::ConnectivityMonitor::is_online(),
                    Message::NetworkConnectivityResult,
                );
            }
            Message::NetworkConnectivityResult(is_online) => {
                if is_online {
                    let mut resume_tasks = Vec::new();
                    for item in &mut self.downloads {
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
                            let engine = self.engine.clone();
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
                        let downloads_clone = self.downloads.clone();
                        return Task::batch([
                            Task::perform(
                                async move { storage::json_store::save_downloads(&downloads_clone) },
                                Message::DownloadsPersisted,
                            ),
                            Task::batch(resume_tasks),
                        ]);
                    }
                }
            }
            Message::DownloadsLoaded(Ok(loaded_items)) => {
                self.downloads = loaded_items;

                // Automatically resume any downloads that were in active Downloading state
                let mut auto_resume_tasks = Vec::new();
                for item in &self.downloads {
                    if matches!(item.state, DownloadState::Downloading { .. }) {
                        let item_clone = item.clone();
                        let engine = self.engine.clone();
                        auto_resume_tasks.push(Task::perform(
                            async move {
                                engine.start_or_resume(item_clone).await;
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
            Message::DownloadsLoaded(Err(err)) => {
                println!("[QDM Storage Error] Failed to load downloads: {}", err);
            }
            Message::NavSelected(filter) => {
                self.current_filter = filter;
                self.is_topbar_menu_open = false;
            }
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::AddUrlPressed => {
                self.is_topbar_menu_open = false;
                self.add_dialog.reset();
                self.add_dialog.save_to = self.settings.download_folder.clone();
                self.add_dialog.max_connections = self.settings.max_connections.to_string();
                if !self.settings.speed_limit_value.is_empty() {
                    self.add_dialog.speed_limit = self.settings.speed_limit_value.clone();
                    self.add_dialog.speed_unit = self.settings.speed_limit_unit;
                }
                self.add_dialog.is_open = true;
            }
            Message::SettingsPressed => {
                self.current_filter = sidebar::NavFilter::Settings;
                self.is_topbar_menu_open = false;
            }
            Message::ToggleTopbarMenu => {
                self.is_topbar_menu_open = !self.is_topbar_menu_open;
            }
            Message::OpenQueueView => {
                self.current_filter = sidebar::NavFilter::Queue;
                self.is_topbar_menu_open = false;
            }
            Message::MoveQueueItemUp(id) => {
                if crate::services::downloads::QueueService::move_item_up(&mut self.downloads, id) {
                    return self.synchronize_and_persist_queue();
                }
            }
            Message::MoveQueueItemDown(id) => {
                if crate::services::downloads::QueueService::move_item_down(&mut self.downloads, id)
                {
                    return self.synchronize_and_persist_queue();
                }
            }
            Message::MoveScheduledItemUp(id) => {
                if crate::services::downloads::QueueService::move_scheduled_up(
                    &mut self.downloads,
                    id,
                ) {
                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }
            Message::MoveScheduledItemDown(id) => {
                if crate::services::downloads::QueueService::move_scheduled_down(
                    &mut self.downloads,
                    id,
                ) {
                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }
            Message::RemoveFromSchedule(id) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    item.is_scheduled = false;
                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }

            // --- Download Engine Event Handling ---
            Message::EngineEvent(EngineUiEvent::ProgressUpdated {
                id,
                downloaded_bytes,
                total_bytes,
                speed_bps,
                eta_secs,
                chunks,
            }) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    item.downloaded_bytes = downloaded_bytes;
                    if total_bytes.is_some() {
                        item.total_bytes = total_bytes;
                    }
                    item.chunks = chunks;

                    // If stream is actively transferring bytes, reset per-failure auto-retry counter
                    if speed_bps > 0 && self.retry_counts.contains_key(&id) {
                        self.retry_counts.remove(&id);
                    }

                    // Guard: Never allow trailing ProgressUpdated events to revert a Completed or Paused/Failed/Queued/Scheduled download
                    if matches!(
                        item.state,
                        DownloadState::Completed
                            | DownloadState::Failed { .. }
                            | DownloadState::Paused { .. }
                            | DownloadState::WaitingForNetwork { .. }
                            | DownloadState::Queued
                            | DownloadState::Scheduled
                    ) {
                        return Task::none();
                    }

                    // If all bytes have been downloaded, transition to Completed
                    if let Some(total) = item.total_bytes {
                        if total > 0 && downloaded_bytes >= total {
                            item.state = DownloadState::Completed;
                            return Task::none();
                        }
                    }

                    item.state = DownloadState::Downloading {
                        downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps,
                        eta_secs,
                    };
                }
            }
            Message::EngineEvent(EngineUiEvent::StateChanged { id, state }) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    println!("[QDM UI] Item {} state changed -> {:?}", id, state);
                    // Do not allow engine pause/resume to overwrite Queued or Scheduled state managed by UI queue
                    if matches!(item.state, DownloadState::Queued | DownloadState::Scheduled) {
                        if matches!(state, DownloadState::Completed | DownloadState::Failed { .. }) {
                            item.state = state;
                        }
                    } else {
                        item.state = state;
                    }
                }
            }
            Message::EngineEvent(EngineUiEvent::DownloadCompleted { id, sha256 }) => {
                self.retry_counts.remove(&id);
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    println!(
                        "[QDM UI] Item {} completed successfully! SHA-256: {:?}",
                        id, sha256
                    );
                    item.state = DownloadState::Completed;
                    if let Some(total) = item.total_bytes {
                        item.downloaded_bytes = total;
                    }
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    item.completed_at = Some(now);
                    if let Some(hash) = sha256 {
                        item.sha256_hash = Some(hash);
                    }

                    // Automatically promote next queued download
                    return self.synchronize_and_persist_queue();
                }
            }
            Message::EngineEvent(EngineUiEvent::DownloadFailed { id, error }) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    println!("[QDM UI] Item {} failed: {}", id, error);

                    // Check auto-retry budget
                    if self.settings.auto_retry_downloads {
                        let count = self.retry_counts.entry(id).or_insert(0);
                        if *count < self.settings.max_auto_retries {
                            *count += 1;
                            let current_retry = *count;
                            println!(
                                "[QDM Auto-Retry] Download {} encountered error, triggering retry {}/{} in 2s...",
                                id, current_retry, self.settings.max_auto_retries
                            );

                            item.state = DownloadState::Downloading {
                                downloaded_bytes: item.downloaded_bytes,
                                total_bytes: item.total_bytes,
                                speed_bps: 0,
                                eta_secs: None,
                            };

                            let item_clone = item.clone();
                            let engine = self.engine.clone();
                            return Task::perform(
                                async move {
                                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                                    engine.start_or_resume(item_clone).await;
                                    Ok(())
                                },
                                |_: Result<(), String>| Message::Tick,
                            );
                        }
                    }

                    item.state = DownloadState::Failed {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        error,
                    };

                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }
            Message::EngineEvent(EngineUiEvent::PersistRequested { item }) => {
                if let Some(target) = self.downloads.iter_mut().find(|d| d.id == item.id) {
                    // Update persistent metadata and chunk positions
                    target.downloaded_bytes = item.downloaded_bytes;
                    if item.total_bytes.is_some() {
                        target.total_bytes = item.total_bytes;
                    }
                    target.chunks = item.chunks;
                    target.etag = item.etag;
                    target.last_modified = item.last_modified;
                    target.sha256_hash = item.sha256_hash;

                    // Only update state if not currently Queued or Scheduled by the queue manager
                    if !matches!(target.state, DownloadState::Queued | DownloadState::Scheduled) {
                        match &item.state {
                            DownloadState::Paused { .. }
                            | DownloadState::Failed { .. }
                            | DownloadState::Completed => {
                                target.state = item.state;
                            }
                            _ => {}
                        }
                    }

                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }

            Message::TogglePause(id) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
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
                            let engine = self.engine.clone();
                            let pause_task = Task::perform(
                                async move {
                                    engine.pause(id).await;
                                    Ok(())
                                },
                                |_: Result<(), String>| Message::Tick,
                            );

                            // Promote next queued item into the free slot!
                            let sync_task = self.synchronize_and_persist_queue();
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
                            let engine = self.engine.clone();
                            let start_task = Task::perform(
                                async move {
                                    engine.start_or_resume(item_clone).await;
                                    Ok(())
                                },
                                |_: Result<(), String>| Message::Tick,
                            );

                            // Synchronize queue: if limit exceeded, preempts lowest-priority running item
                            let sync_task = self.synchronize_and_persist_queue();
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
                            let engine = self.engine.clone();
                            let start_task = Task::perform(
                                async move {
                                    engine.start_or_resume(item_clone).await;
                                    Ok(())
                                },
                                |_: Result<(), String>| Message::Tick,
                            );

                            // Protect this item so it won't preempt itself; preempt lowest of the OTHER running items
                            let sync_task =
                                self.synchronize_and_persist_queue_with_protected(Some(id));
                            return Task::batch([start_task, sync_task]);
                        }
                        DownloadState::Completed => {}
                    }
                }
            }
            Message::CancelDownload(id) => {
                if let Some(action) = self.settings.delete_action {
                    // Apply remembered preference immediately
                    let item_opt = self.downloads.iter().find(|d| d.id == id).cloned();
                    self.downloads.retain(|d| d.id != id);
                    let engine = self.engine.clone();
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
                    let sync_task = self.synchronize_and_persist_queue();
                    return Task::batch([cancel_task, sync_task]);
                }

                // If not remembered, open the confirmation modal
                if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                    self.delete_dialog.open(delete_dialogue::DeletePendingItem {
                        id: item.id,
                        filename: item.filename.clone(),
                        save_path: item.save_path.clone(),
                    });
                }
            }
            Message::OpenFolder(id) => {
                if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                    let folder_path = item.save_path.clone();
                    #[cfg(target_os = "windows")]
                    {
                        let _ = std::process::Command::new("explorer")
                            .arg(&folder_path)
                            .spawn();
                    }
                    #[cfg(target_os = "macos")]
                    {
                        let _ = std::process::Command::new("open").arg(&folder_path).spawn();
                    }
                    #[cfg(target_os = "linux")]
                    {
                        let _ = std::process::Command::new("xdg-open")
                            .arg(&folder_path)
                            .spawn();
                    }
                }
            }
            Message::OpenMirrorsModal(id) => {
                if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                    self.mirror_dialog.open(
                        item.id,
                        item.filename.clone(),
                        item.primary_url.clone(),
                        item.mirror_urls.clone(),
                    );
                }
            }

            Message::SettingsMessage(settings::SettingsMessage::BrowseFolderPressed) => {
                let current_folder = self.settings.download_folder.clone();
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
                return Task::perform(task, |res| {
                    Message::SettingsMessage(settings::SettingsMessage::BrowseFolderResult(res))
                });
            }

            Message::SettingsMessage(settings::SettingsMessage::MoveScheduledItemUp(id)) => {
                return self.update(Message::MoveScheduledItemUp(id));
            }
            Message::SettingsMessage(settings::SettingsMessage::MoveScheduledItemDown(id)) => {
                return self.update(Message::MoveScheduledItemDown(id));
            }
            Message::SettingsMessage(settings::SettingsMessage::RemoveFromSchedule(id)) => {
                return self.update(Message::RemoveFromSchedule(id));
            }
            Message::SettingsMessage(settings::SettingsMessage::SimultaneousDownloadsInc)
            | Message::SettingsMessage(settings::SettingsMessage::SimultaneousDownloadsDec) => {
                settings::update(
                    &mut self.settings,
                    settings::SettingsMessage::SimultaneousDownloadsInc,
                );
                return self.synchronize_and_persist_queue();
            }

            Message::SettingsMessage(msg) => {
                settings::update(&mut self.settings, msg);
            }

            Message::AddDialogueModalMessages(
                add_dialogue::AddDialogueModalMessage::SubmitNewDownload,
            ) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let mut filename = if self.add_dialog.filename.trim().is_empty() {
                    add_dialogue::extract_filename(&url, None)
                } else {
                    self.add_dialog.filename.trim().to_string()
                };

                let speed_limit_bps = if !self.add_dialog.speed_limit.trim().is_empty() {
                    self.add_dialog
                        .speed_limit
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .map(|v| self.add_dialog.speed_unit.to_bps(v))
                } else {
                    self.settings.global_speed_limit_bps()
                };

                let save_path = self.add_dialog.save_to.clone();
                let file_conflict =
                    crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);

                if file_conflict {
                    match self.settings.file_conflict_action {
                        Some(settings::FileConflictAction::AutoRename) => {
                            filename = crate::core::utils::paths::generate_unique_filename(
                                &save_path, &filename,
                            );
                        }
                        Some(settings::FileConflictAction::Overwrite) => {
                            // Overwrite: keep filename as is
                        }
                        None => {
                            let parsed_mirrors = self.add_dialog.parsed_mirrors();
                            let max_connections = self
                                .add_dialog
                                .max_connections
                                .trim()
                                .parse::<usize>()
                                .unwrap_or(8);
                            self.conflict_dialog
                                .open(conflict_dialogue::ConflictPendingDownload {
                                    url,
                                    filename,
                                    save_to: save_path,
                                    max_connections,
                                    speed_limit: speed_limit_bps,
                                    mirror_urls: parsed_mirrors,
                                });
                            self.add_dialog.is_open = false;
                            self.add_dialog.reset();
                            return Task::none();
                        }
                    }
                }

                let total_bytes = self
                    .add_dialog
                    .download_file_metadata
                    .as_ref()
                    .and_then(|m| m.content_length);

                let resumable = self
                    .add_dialog
                    .download_file_metadata
                    .as_ref()
                    .map(|m| m.supports_resume)
                    .unwrap_or(false);

                let file_type = FileType::from_filename(&filename);

                let mirror_urls: Vec<DownloadUrl> = self
                    .add_dialog
                    .parsed_mirrors()
                    .into_iter()
                    .map(DownloadUrl::new)
                    .collect();

                let max_connections = self
                    .add_dialog
                    .max_connections
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(8);

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let new_item = DownloadItem {
                    id: 0,
                    filename,
                    primary_url: DownloadUrl::new(&url),
                    mirror_urls,
                    save_path,
                    downloaded_bytes: 0,
                    total_bytes,
                    state: DownloadState::Downloading {
                        downloaded_bytes: 0,
                        total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    },
                    file_type,
                    resumable,
                    is_scheduled: false,
                    max_connections,
                    speed_limit_bps,
                    etag: None,
                    last_modified: None,
                    sha256_hash: None,
                    chunks: Vec::new(),
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };

                self.add_dialog.is_open = false;
                self.add_dialog.reset();

                return Task::perform(
                    async move { storage::json_store::insert_download(new_item) },
                    Message::DownloadSaved,
                );
            }

            Message::AddDialogueModalMessages(
                add_dialogue::AddDialogueModalMessage::QuickAddDownload,
            ) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let mut filename = add_dialogue::extract_filename(&url, None);
                let save_path = self.add_dialog.save_to.clone();
                let speed_limit_bps = self.settings.global_speed_limit_bps();
                let file_conflict =
                    crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);

                if file_conflict {
                    match self.settings.file_conflict_action {
                        Some(settings::FileConflictAction::AutoRename) => {
                            filename = crate::core::utils::paths::generate_unique_filename(
                                &save_path, &filename,
                            );
                        }
                        Some(settings::FileConflictAction::Overwrite) => {
                            // Overwrite: keep filename
                        }
                        None => {
                            let parsed_mirrors = self.add_dialog.parsed_mirrors();
                            self.conflict_dialog
                                .open(conflict_dialogue::ConflictPendingDownload {
                                    url,
                                    filename,
                                    save_to: save_path,
                                    max_connections: self.settings.max_connections,
                                    speed_limit: speed_limit_bps,
                                    mirror_urls: parsed_mirrors,
                                });
                            self.add_dialog.is_open = false;
                            self.add_dialog.reset();
                            return Task::none();
                        }
                    }
                }

                let file_type = FileType::from_filename(&filename);
                let mirror_urls: Vec<DownloadUrl> = self
                    .add_dialog
                    .parsed_mirrors()
                    .into_iter()
                    .map(DownloadUrl::new)
                    .collect();

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let new_item = DownloadItem {
                    id: 0,
                    filename,
                    primary_url: DownloadUrl::new(&url),
                    mirror_urls,
                    save_path,
                    downloaded_bytes: 0,
                    total_bytes: None,
                    state: DownloadState::FetchingMetadata,
                    file_type,
                    resumable: false,
                    is_scheduled: false,
                    max_connections: self.settings.max_connections as u32,
                    speed_limit_bps,
                    etag: None,
                    last_modified: None,
                    sha256_hash: None,
                    chunks: Vec::new(),
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };

                self.add_dialog.is_open = false;
                self.add_dialog.reset();

                return Task::perform(
                    async move { storage::json_store::insert_download(new_item) },
                    Message::DownloadSaved,
                );
            }

            Message::AddDialogueModalMessages(
                add_dialogue::AddDialogueModalMessage::ScheduleNewDownload,
            ) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let mut filename = if self.add_dialog.filename.trim().is_empty() {
                    add_dialogue::extract_filename(&url, None)
                } else {
                    self.add_dialog.filename.trim().to_string()
                };

                let speed_limit_bps = if !self.add_dialog.speed_limit.trim().is_empty() {
                    self.add_dialog
                        .speed_limit
                        .trim()
                        .parse::<u64>()
                        .ok()
                        .map(|v| self.add_dialog.speed_unit.to_bps(v))
                } else {
                    self.settings.global_speed_limit_bps()
                };

                let save_path = self.add_dialog.save_to.clone();
                let file_conflict =
                    crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);

                if file_conflict {
                    match self.settings.file_conflict_action {
                        Some(settings::FileConflictAction::AutoRename) => {
                            filename = crate::core::utils::paths::generate_unique_filename(
                                &save_path, &filename,
                            );
                        }
                        Some(settings::FileConflictAction::Overwrite) => {
                            // Overwrite: keep filename as is
                        }
                        None => {
                            let parsed_mirrors = self.add_dialog.parsed_mirrors();
                            let max_connections = self
                                .add_dialog
                                .max_connections
                                .trim()
                                .parse::<usize>()
                                .unwrap_or(8);
                            self.conflict_dialog
                                .open(conflict_dialogue::ConflictPendingDownload {
                                    url,
                                    filename,
                                    save_to: save_path,
                                    max_connections,
                                    speed_limit: speed_limit_bps,
                                    mirror_urls: parsed_mirrors,
                                });
                            self.add_dialog.is_open = false;
                            self.add_dialog.reset();
                            return Task::none();
                        }
                    }
                }

                let total_bytes = self
                    .add_dialog
                    .download_file_metadata
                    .as_ref()
                    .and_then(|m| m.content_length);

                let resumable = self
                    .add_dialog
                    .download_file_metadata
                    .as_ref()
                    .map(|m| m.supports_resume)
                    .unwrap_or(false);

                let file_type = FileType::from_filename(&filename);

                let mirror_urls: Vec<DownloadUrl> = self
                    .add_dialog
                    .parsed_mirrors()
                    .into_iter()
                    .map(DownloadUrl::new)
                    .collect();

                let max_connections = self
                    .add_dialog
                    .max_connections
                    .trim()
                    .parse::<u32>()
                    .unwrap_or(8);

                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                let now_dt = chrono::Local::now().naive_local();
                let initial_state = if self.settings.schedule.is_in_active_window(now_dt) {
                    DownloadState::Queued
                } else {
                    DownloadState::Scheduled
                };

                let new_item = DownloadItem {
                    id: 0,
                    filename,
                    primary_url: DownloadUrl::new(&url),
                    mirror_urls,
                    save_path,
                    downloaded_bytes: 0,
                    total_bytes,
                    state: initial_state,
                    file_type,
                    resumable,
                    is_scheduled: true,
                    max_connections,
                    speed_limit_bps,
                    etag: None,
                    last_modified: None,
                    sha256_hash: None,
                    chunks: Vec::new(),
                    created_at: now,
                    updated_at: now,
                    completed_at: None,
                };

                self.add_dialog.is_open = false;
                self.add_dialog.reset();

                return Task::perform(
                    async move { storage::json_store::insert_download(new_item) },
                    Message::DownloadSaved,
                );
            }

            Message::DownloadSaved(Ok(inserted_item)) => {
                let item_id = inserted_item.id;
                let url = inserted_item.primary_url.url.clone();
                let is_scheduled = inserted_item.is_scheduled;
                let needs_bg_meta = matches!(inserted_item.state, DownloadState::FetchingMetadata)
                    || (inserted_item.total_bytes.is_none() && !is_scheduled);

                self.downloads.push(inserted_item.clone());

                if is_scheduled {
                    let now = chrono::Local::now().naive_local();
                    if self.settings.schedule.is_in_active_window(now) {
                        return self.synchronize_and_persist_queue();
                    } else {
                        return Task::none();
                    }
                } else if needs_bg_meta {
                    let engine = self.engine.clone();
                    return Task::perform(
                        async move { (item_id, engine.probe_metadata(&url).await) },
                        |(id, res)| Message::BackgroundMetadataFetched(id, res),
                    );
                } else {
                    return self.synchronize_and_persist_queue();
                }
            }
            Message::DownloadSaved(Err(err)) => {
                println!("[QDM Storage Error] Failed to save download: {}", err);
            }

            Message::BackgroundMetadataFetched(id, result) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    match result {
                        Ok(meta) => {
                            if let Some(len) = meta.content_length {
                                item.total_bytes = Some(len);
                            }
                            item.resumable = meta.supports_resume;
                            item.etag = meta.etag;
                            item.last_modified = meta.last_modified;

                            if item.filename == "download.file" {
                                if let Some(ref cd) = meta.content_disposition {
                                    let better_name = add_dialogue::extract_filename(
                                        &item.primary_url.url,
                                        Some(cd),
                                    );
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
                    let engine = self.engine.clone();
                    let downloads_clone = self.downloads.clone();

                    return Task::perform(
                        async move {
                            let _ = storage::json_store::save_downloads(&downloads_clone);
                            engine.start_or_resume(item_clone).await;
                            Ok(())
                        },
                        Message::DownloadsPersisted,
                    );
                }
            }

            Message::MirrorDialogueMessages(
                mirror_dialogue::MirrorDialogueMessage::CloseMirrorDialog,
            )
            | Message::MirrorDialogueMessages(
                mirror_dialogue::MirrorDialogueMessage::SaveAndClose,
            ) => {
                let target_id = self.mirror_dialog.download_id;
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == target_id) {
                    item.primary_url = self.mirror_dialog.primary_url.clone();
                    item.mirror_urls = self.mirror_dialog.mirror_urls.clone();
                }
                self.mirror_dialog.close();

                let downloads_clone = self.downloads.clone();
                return Task::perform(
                    async move { storage::json_store::save_downloads(&downloads_clone) },
                    Message::DownloadsPersisted,
                );
            }

            Message::MirrorDialogueMessages(message) => {
                let task = mirror_dialogue::update(&mut self.mirror_dialog, message)
                    .map(Message::MirrorDialogueMessages);

                let target_id = self.mirror_dialog.download_id;
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == target_id) {
                    item.primary_url = self.mirror_dialog.primary_url.clone();
                    item.mirror_urls = self.mirror_dialog.mirror_urls.clone();
                }

                return task;
            }

            Message::SyncWithDisk => {
                let mut changed = false;
                self.downloads.retain(|d| {
                    let target = std::path::Path::new(&d.save_path).join(&d.filename);
                    if matches!(d.state, DownloadState::Completed) {
                        if !target.exists() {
                            println!(
                                "[QDM Disk Sync] Removed externally deleted completed file: {}",
                                d.filename
                            );
                            changed = true;
                            return false;
                        }
                    }
                    true
                });

                for d in &mut self.downloads {
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
                                error: "File removed or missing from destination folder"
                                    .to_string(),
                            };
                            d.downloaded_bytes = 0;
                            changed = true;
                        }
                    }
                }

                if changed {
                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    );
                }
            }

            Message::ConflictDialogueMessages(msg) => match msg {
                conflict_dialogue::ConflictDialogMessage::Close => {
                    self.conflict_dialog.close();
                }
                conflict_dialogue::ConflictDialogMessage::ToggleRemember(val) => {
                    self.conflict_dialog.remember_choice = val;
                }
                conflict_dialogue::ConflictDialogMessage::AutoRenameChosen => {
                    if self.conflict_dialog.remember_choice {
                        self.settings.file_conflict_action =
                            Some(settings::FileConflictAction::AutoRename);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.conflict_dialog.pending.take() {
                        let new_filename = crate::core::utils::paths::generate_unique_filename(
                            &pending.save_to,
                            &pending.filename,
                        );
                        let file_type = FileType::from_filename(&new_filename);
                        let mirror_urls: Vec<DownloadUrl> = pending
                            .mirror_urls
                            .into_iter()
                            .map(DownloadUrl::new)
                            .collect();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let new_item = DownloadItem {
                            id: 0,
                            filename: new_filename,
                            primary_url: DownloadUrl::new(&pending.url),
                            mirror_urls,
                            save_path: pending.save_to,
                            downloaded_bytes: 0,
                            total_bytes: None,
                            state: DownloadState::FetchingMetadata,
                            file_type,
                            resumable: false,
                            is_scheduled: false,
                            max_connections: pending.max_connections as u32,
                            speed_limit_bps: pending.speed_limit,
                            etag: None,
                            last_modified: None,
                            sha256_hash: None,
                            chunks: Vec::new(),
                            created_at: now,
                            updated_at: now,
                            completed_at: None,
                        };
                        self.conflict_dialog.close();
                        return Task::perform(
                            async move { storage::json_store::insert_download(new_item) },
                            Message::DownloadSaved,
                        );
                    }
                    self.conflict_dialog.close();
                }
                conflict_dialogue::ConflictDialogMessage::OverwriteChosen => {
                    if self.conflict_dialog.remember_choice {
                        self.settings.file_conflict_action =
                            Some(settings::FileConflictAction::Overwrite);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.conflict_dialog.pending.take() {
                        let file_type = FileType::from_filename(&pending.filename);
                        let mirror_urls: Vec<DownloadUrl> = pending
                            .mirror_urls
                            .into_iter()
                            .map(DownloadUrl::new)
                            .collect();
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        let new_item = DownloadItem {
                            id: 0,
                            filename: pending.filename,
                            primary_url: DownloadUrl::new(&pending.url),
                            mirror_urls,
                            save_path: pending.save_to,
                            downloaded_bytes: 0,
                            total_bytes: None,
                            state: DownloadState::FetchingMetadata,
                            file_type,
                            resumable: false,
                            is_scheduled: false,
                            max_connections: pending.max_connections as u32,
                            speed_limit_bps: pending.speed_limit,
                            etag: None,
                            last_modified: None,
                            sha256_hash: None,
                            chunks: Vec::new(),
                            created_at: now,
                            updated_at: now,
                            completed_at: None,
                        };
                        self.conflict_dialog.close();
                        return Task::perform(
                            async move { storage::json_store::insert_download(new_item) },
                            Message::DownloadSaved,
                        );
                    }
                    self.conflict_dialog.close();
                }
            },

            Message::DeleteDialogueMessages(msg) => match msg {
                delete_dialogue::DeleteDialogMessage::Close => {
                    self.delete_dialog.close();
                }
                delete_dialogue::DeleteDialogMessage::ToggleRemember(val) => {
                    self.delete_dialog.remember_choice = val;
                }
                delete_dialogue::DeleteDialogMessage::RemoveFromListChosen => {
                    if self.delete_dialog.remember_choice {
                        self.settings.delete_action = Some(settings::DeleteAction::RemoveFromList);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.delete_dialog.pending.take() {
                        self.downloads.retain(|d| d.id != pending.id);
                        let engine = self.engine.clone();
                        self.delete_dialog.close();
                        let cancel_task = Task::perform(
                            async move {
                                engine.cancel(pending.id).await;
                                Ok(())
                            },
                            |_: Result<(), String>| Message::Tick,
                        );
                        let sync_task = self.synchronize_and_persist_queue();
                        return Task::batch([cancel_task, sync_task]);
                    }
                    self.delete_dialog.close();
                }
                delete_dialogue::DeleteDialogMessage::DeleteFromDiskChosen => {
                    if self.delete_dialog.remember_choice {
                        self.settings.delete_action = Some(settings::DeleteAction::DeleteFromDisk);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.delete_dialog.pending.take() {
                        self.downloads.retain(|d| d.id != pending.id);
                        let engine = self.engine.clone();
                        self.delete_dialog.close();
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
                        let sync_task = self.synchronize_and_persist_queue();
                        return Task::batch([cancel_task, sync_task]);
                    }
                    self.delete_dialog.close();
                }
            },

            Message::DownloadsPersisted(Err(err)) => {
                println!("[QDM Storage Error] Failed to persist downloads: {}", err);
            }
            Message::DownloadsPersisted(Ok(())) => {}

            Message::AddDialogueModalMessages(message) => {
                return add_dialogue::update(&mut self.add_dialog, message)
                    .map(Message::AddDialogueModalMessages);
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let engine = self.engine.clone();
        let engine_sub = Subscription::run_with_id(
            "qdm_download_engine_stream",
            iced::stream::channel(256, move |mut output| async move {
                use iced::futures::SinkExt;
                let mut rx = engine.subscribe();
                while let Ok(event) = rx.recv().await {
                    let _ = output.send(Message::EngineEvent(event)).await;
                }
            }),
        );

        let anim_sub = if self.settings.is_animating() {
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };

        let second_tick_sub =
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::SecondTick);
        let disk_sync_sub =
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::SyncWithDisk);
        let network_check_sub = iced::time::every(std::time::Duration::from_secs(3))
            .map(|_| Message::CheckNetworkConnectivity);

        Subscription::batch([
            engine_sub,
            anim_sub,
            second_tick_sub,
            disk_sync_sub,
            network_check_sub,
        ])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let downloading_count = self
            .downloads
            .iter()
            .filter(|d| {
                matches!(
                    d.state,
                    DownloadState::Downloading { .. }
                        | DownloadState::FetchingMetadata
                        | DownloadState::Queued
                        | DownloadState::WaitingForNetwork { .. }
                )
            })
            .count();

        let completed_count = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Completed))
            .count();

        let failed_count = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Failed { .. }))
            .count();

        let scheduled_count = self.downloads.iter().filter(|d| d.is_scheduled).count();

        let total_speed_bps: u64 = self
            .downloads
            .iter()
            .filter_map(|d| match &d.state {
                DownloadState::Downloading { speed_bps, .. } => Some(*speed_bps),
                _ => None,
            })
            .sum();
        let total_speed_str = crate::models::download::format_speed(total_speed_bps);

        let sidebar = sidebar::sidebar_view(
            self.current_filter,
            downloading_count,
            completed_count,
            failed_count,
            scheduled_count,
            Message::NavSelected,
        );

        let title = match self.current_filter {
            sidebar::NavFilter::All => "All Downloads",
            sidebar::NavFilter::Downloading => "Active Downloads",
            sidebar::NavFilter::Completed => "Completed Downloads",
            sidebar::NavFilter::Failed => "Failed Downloads",
            sidebar::NavFilter::Scheduled => "Scheduled Downloads",
            sidebar::NavFilter::Queue => "Downloads Queue",
            sidebar::NavFilter::Settings => "Settings",
        };

        let toolbar = toolbar::toolbar_view(
            title,
            &self.search_query,
            downloading_count,
            &total_speed_str,
            self.is_topbar_menu_open,
            Message::SearchChanged,
            Message::AddUrlPressed,
            Message::ToggleTopbarMenu,
        );

        let schedule_banner: Option<Element<Message>> = if self.current_filter
            == sidebar::NavFilter::Scheduled
        {
            let now = chrono::Local::now().naive_local();
            let is_enabled = self.settings.schedule.enabled;
            let in_window = self.settings.schedule.is_in_active_window(now);

            let (banner_icon, banner_text, banner_bg, border_col) = if is_enabled {
                if in_window {
                    (
                        icons::ICON_SCHEDULED,
                        format!(
                            "Schedule Active · Currently downloading inside active window (Starts {} - Stops {})",
                            self.settings.schedule.formatted_start_12h(),
                            if self.settings.schedule.stop_enabled {
                                self.settings.schedule.formatted_stop_12h()
                            } else {
                                "Unlimited".to_string()
                            }
                        ),
                        colors::SURFACE_HIGH,
                        colors::PRIMARY,
                    )
                } else if let Some(until) = self.settings.schedule.time_until_next_start(now) {
                    (
                        icons::ICON_SCHEDULED,
                        format!(
                            "Schedule Active · Next window starts in {} ({})",
                            crate::models::schedule::format_countdown(until),
                            self.settings.schedule.formatted_start_12h()
                        ),
                        colors::SURFACE_HIGH,
                        colors::PRIMARY,
                    )
                } else {
                    (
                        icons::ICON_SCHEDULED,
                        format!(
                            "Schedule Active · Starts at {}",
                            self.settings.schedule.formatted_start_12h()
                        ),
                        colors::SURFACE_HIGH,
                        colors::PRIMARY,
                    )
                }
            } else {
                (
                    icons::ICON_SCHEDULED,
                    "Download Scheduler is currently disabled. Go to Settings > Scheduler to configure time windows.".to_string(),
                    colors::SURFACE,
                    colors::BORDER,
                )
            };

            let banner = container(
                row![
                    icon(banner_icon).size(14).color(if is_enabled {
                        colors::PRIMARY
                    } else {
                        colors::TEXT_MUTED
                    }),
                    text(banner_text)
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(if is_enabled {
                            colors::TEXT_PRIMARY
                        } else {
                            colors::TEXT_MUTED
                        }),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(banner_bg)),
                border: iced::Border {
                    color: border_col,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            Some(
                container(banner)
                    .padding(iced::padding::top(16).bottom(0).left(24).right(24))
                    .into(),
            )
        } else {
            None
        };

        let main_content: Element<Message> = match self.current_filter {
            sidebar::NavFilter::Settings => {
                settings::settings_view(&self.settings, &self.downloads)
                    .map(Message::SettingsMessage)
            }
            sidebar::NavFilter::Queue => crate::views::preferences::queue_view(
                &self.downloads,
                Message::MoveQueueItemUp,
                Message::MoveQueueItemDown,
                Message::TogglePause,
            ),
            _ => {
                let filtered_items = self.downloads.iter().rev().filter(|d| {
                    let matches_filter = match self.current_filter {
                        sidebar::NavFilter::All => true,
                        sidebar::NavFilter::Downloading => matches!(
                            d.state,
                            DownloadState::Downloading { .. }
                                | DownloadState::FetchingMetadata
                                | DownloadState::Queued
                                | DownloadState::WaitingForNetwork { .. }
                        ),
                        sidebar::NavFilter::Completed => {
                            matches!(d.state, DownloadState::Completed)
                        }
                        sidebar::NavFilter::Failed => {
                            matches!(d.state, DownloadState::Failed { .. })
                        }
                        sidebar::NavFilter::Scheduled => d.is_scheduled,
                        sidebar::NavFilter::Queue => false,
                        sidebar::NavFilter::Settings => false,
                    };

                    let matches_search = if self.search_query.is_empty() {
                        true
                    } else {
                        d.filename
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                            || d.primary_url
                                .url
                                .to_lowercase()
                                .contains(&self.search_query.to_lowercase())
                    };

                    matches_filter && matches_search
                });

                let list_view = download_list_view(
                    filtered_items,
                    Message::TogglePause,
                    Message::CancelDownload,
                    Message::OpenFolder,
                    Message::OpenMirrorsModal,
                );

                if let Some(banner) = schedule_banner {
                    column![banner, list_view]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                } else {
                    list_view
                }
            }
        };

        let right_content: Element<Message> = if self.is_topbar_menu_open {
            let overlay =
                toolbar::dropdown_overlay(Message::OpenQueueView, Message::SettingsPressed);
            stack![
                column![toolbar, main_content]
                    .width(Length::Fill)
                    .height(Length::Fill),
                overlay
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            column![toolbar, main_content]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        };

        let root_layout = row![sidebar, right_content]
            .width(Length::Fill)
            .height(Length::Fill);

        let base_view = container(root_layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::BACKGROUND)),
                text_color: Some(colors::TEXT_PRIMARY),
                ..Default::default()
            });

        if self.conflict_dialog.is_open {
            let conflict_modal = conflict_dialogue::view(&self.conflict_dialog)
                .map(Message::ConflictDialogueMessages);

            stack![base_view, conflict_modal].into()
        } else if self.delete_dialog.is_open {
            let delete_modal =
                delete_dialogue::view(&self.delete_dialog).map(Message::DeleteDialogueMessages);

            stack![base_view, delete_modal].into()
        } else if self.add_dialog.is_open {
            let dialog_modal =
                add_dialogue::view(&self.add_dialog).map(Message::AddDialogueModalMessages);

            stack![base_view, dialog_modal].into()
        } else if self.mirror_dialog.is_open {
            let mirror_modal =
                mirror_dialogue::view(&self.mirror_dialog).map(Message::MirrorDialogueMessages);

            stack![base_view, mirror_modal].into()
        } else {
            base_view.into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
