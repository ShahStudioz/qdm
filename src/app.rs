use crate::models::download::{DownloadItem, DownloadState, DownloadUrl, FileType};
use crate::services::downloads::{DownloadEngine, EngineUiEvent, FileMetadata};
use crate::services::storage;
use crate::theme::colors;
use crate::views::components::{sidebar, toolbar};
use crate::views::dialogues::{add_dialogue, conflict_dialogue, delete_dialogue, mirror_dialogue};
use crate::views::downloads::download_list::download_list_view;
use crate::views::settings::settings;
use iced::widget::{column, container, row, stack};
use iced::{Element, Length, Subscription, Task, Theme};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    SecondTick,
    SyncWithDisk,
    DownloadsLoaded(Result<Vec<DownloadItem>, String>),
    DownloadSaved(Result<DownloadItem, String>),
    DownloadsPersisted(Result<(), String>),
    BackgroundMetadataFetched(usize, Result<FileMetadata, String>),
    EngineEvent(EngineUiEvent),

    NavSelected(sidebar::NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    NotificationPressed,
    SettingsPressed,

    TogglePause(usize),
    CancelDownload(usize),
    OpenFolder(usize),
    OpenMirrorsModal(usize),

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
            async {
                storage::json_store::load_downloads()
            },
            Message::DownloadsLoaded,
        );

        (app, task)
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
            }
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::AddUrlPressed => {
                self.add_dialog.reset();
                self.add_dialog.is_open = true;
            }
            Message::NotificationPressed => {
                println!("[QDM] Notifications clicked");
            }
            Message::SettingsPressed => {
                self.current_filter = sidebar::NavFilter::Settings;
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

                    // Guard: Never allow trailing ProgressUpdated events to revert a Completed or Paused/Failed download
                    if matches!(item.state, DownloadState::Completed | DownloadState::Failed { .. } | DownloadState::Paused { .. }) {
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
                    item.state = state;
                }
            }
            Message::EngineEvent(EngineUiEvent::DownloadCompleted { id, sha256 }) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    println!("[QDM UI] Item {} completed successfully! SHA-256: {:?}", id, sha256);
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

                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move {
                            storage::json_store::save_downloads(&downloads_clone)
                        },
                        Message::DownloadsPersisted,
                    );
                }
            }
            Message::EngineEvent(EngineUiEvent::DownloadFailed { id, error }) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    println!("[QDM UI] Item {} failed: {}", id, error);
                    item.state = DownloadState::Failed {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        error,
                    };

                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move {
                            storage::json_store::save_downloads(&downloads_clone)
                        },
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

                    match &item.state {
                        DownloadState::Paused { .. } | DownloadState::Failed { .. } | DownloadState::Completed => {
                            target.state = item.state;
                        }
                        _ => {}
                    }

                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move {
                            storage::json_store::save_downloads(&downloads_clone)
                        },
                        Message::DownloadsPersisted,
                    );
                }
            }

            Message::TogglePause(id) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    match &item.state {
                        DownloadState::Downloading { downloaded_bytes, total_bytes, .. } => {
                            let bytes = *downloaded_bytes;
                            let total = *total_bytes;
                            item.state = DownloadState::Paused {
                                downloaded_bytes: bytes,
                                total_bytes: total,
                            };
                            let engine = self.engine.clone();
                            let downloads_clone = self.downloads.clone();
                            return Task::perform(
                                async move {
                                    engine.pause(id).await;
                                    storage::json_store::save_downloads(&downloads_clone)
                                },
                                Message::DownloadsPersisted,
                            );
                        }
                        DownloadState::Paused { .. }
                        | DownloadState::Failed { .. }
                        | DownloadState::Queued => {
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
                        _ => {}
                    }
                }
            }
            Message::CancelDownload(id) => {
                if let Some(action) = self.settings.delete_action {
                    // Apply remembered preference immediately
                    let item_opt = self.downloads.iter().find(|d| d.id == id).cloned();
                    self.downloads.retain(|d| d.id != id);
                    let engine = self.engine.clone();
                    let downloads_clone = self.downloads.clone();
                    return Task::perform(
                        async move {
                            engine.cancel(id).await;
                            if action == settings::DeleteAction::DeleteFromDisk {
                                if let Some(item) = item_opt {
                                    let target = std::path::Path::new(&item.save_path).join(&item.filename);
                                    let temp_target = std::path::Path::new(&item.save_path).join(format!("{}.qdmdownload", item.filename));
                                    if target.exists() {
                                        let _ = std::fs::remove_file(target);
                                    }
                                    if temp_target.exists() {
                                        let _ = std::fs::remove_file(temp_target);
                                    }
                                }
                            }
                            storage::json_store::save_downloads(&downloads_clone)
                        },
                        Message::DownloadsPersisted,
                    );
                } else {
                    // No remembered preference: prompt user with Delete Confirmation Modal
                    if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                        self.delete_dialog.open(delete_dialogue::DeletePendingItem {
                            id,
                            filename: item.filename.clone(),
                            save_path: item.save_path.clone(),
                        });
                    }
                }
            }
            Message::OpenFolder(id) => {
                if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                    let target = std::path::Path::new(&item.save_path).join(&item.filename);
                    let temp_target = std::path::Path::new(&item.save_path).join(format!("{}.qdmdownload", item.filename));
                    if target.exists() {
                        let _ = std::process::Command::new("explorer")
                            .arg(format!("/select,{}", target.display()))
                            .spawn();
                    } else if temp_target.exists() {
                        let _ = std::process::Command::new("explorer")
                            .arg(format!("/select,{}", temp_target.display()))
                            .spawn();
                    } else {
                        let _ = std::process::Command::new("explorer")
                            .arg(&item.save_path)
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
                    let mut dialog = rfd::AsyncFileDialog::new().set_title("Select Default Download Directory");
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

            Message::SettingsMessage(msg) => {
                settings::update(&mut self.settings, msg);
            }

            Message::AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage::SubmitNewDownload) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let mut filename = if self.add_dialog.filename.trim().is_empty() {
                    add_dialogue::extract_filename(&url, None)
                } else {
                    self.add_dialog.filename.trim().to_string()
                };

                let save_path = self.add_dialog.save_to.clone();
                let file_conflict = crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);

                if file_conflict {
                    match self.settings.file_conflict_action {
                        Some(settings::FileConflictAction::AutoRename) => {
                            filename = crate::core::utils::paths::generate_unique_filename(&save_path, &filename);
                        }
                        Some(settings::FileConflictAction::Overwrite) => {
                            // Overwrite: keep filename as is
                        }
                        None => {
                            let parsed_mirrors = self.add_dialog.parsed_mirrors();
                            let max_connections = self.add_dialog.max_connections.trim().parse::<usize>().unwrap_or(8);
                            let speed_limit = self.add_dialog.speed_limit.trim().parse::<usize>().unwrap_or(0);
                            self.conflict_dialog.open(conflict_dialogue::ConflictPendingDownload {
                                url,
                                filename,
                                save_to: save_path,
                                max_connections,
                                speed_limit,
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
                    max_connections,
                    speed_limit_bps: None,
                    etag: None,
                    last_modified: None,
                    sha256_hash: None,
                    chunks: Vec::new(),
                    created_at: 0,
                    updated_at: 0,
                    completed_at: None,
                };

                self.add_dialog.is_open = false;
                self.add_dialog.reset();

                return Task::perform(
                    async move {
                        storage::json_store::insert_download(new_item)
                    },
                    Message::DownloadSaved,
                );
            }

            Message::AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage::QuickAddDownload) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let mut filename = add_dialogue::extract_filename(&url, None);
                let save_path = self.add_dialog.save_to.clone();
                let file_conflict = crate::core::utils::paths::file_exists_or_downloading(&save_path, &filename);

                if file_conflict {
                    match self.settings.file_conflict_action {
                        Some(settings::FileConflictAction::AutoRename) => {
                            filename = crate::core::utils::paths::generate_unique_filename(&save_path, &filename);
                        }
                        Some(settings::FileConflictAction::Overwrite) => {
                            // Overwrite: keep filename
                        }
                        None => {
                            let parsed_mirrors = self.add_dialog.parsed_mirrors();
                            self.conflict_dialog.open(conflict_dialogue::ConflictPendingDownload {
                                url,
                                filename,
                                save_to: save_path,
                                max_connections: 8,
                                speed_limit: 0,
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
                    max_connections: 8,
                    speed_limit_bps: None,
                    etag: None,
                    last_modified: None,
                    sha256_hash: None,
                    chunks: Vec::new(),
                    created_at: 0,
                    updated_at: 0,
                    completed_at: None,
                };

                self.add_dialog.is_open = false;
                self.add_dialog.reset();

                return Task::perform(
                    async move {
                        storage::json_store::insert_download(new_item)
                    },
                    Message::DownloadSaved,
                );
            }

            Message::DownloadSaved(Ok(inserted_item)) => {
                let item_id = inserted_item.id;
                let url = inserted_item.primary_url.url.clone();
                let needs_bg_meta = matches!(inserted_item.state, DownloadState::FetchingMetadata)
                    || inserted_item.total_bytes.is_none();

                self.downloads.insert(0, inserted_item.clone());

                if needs_bg_meta {
                    let engine = self.engine.clone();
                    return Task::perform(
                        async move {
                            (item_id, engine.probe_metadata(&url).await)
                        },
                        |(id, res)| Message::BackgroundMetadataFetched(id, res),
                    );
                } else {
                    let engine = self.engine.clone();
                    return Task::perform(
                        async move {
                            engine.start_or_resume(inserted_item).await;
                            Ok(())
                        },
                        |_: Result<(), String>| Message::Tick,
                    );
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
                                    let better_name = add_dialogue::extract_filename(&item.primary_url.url, Some(cd));
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

            Message::MirrorDialogueMessages(mirror_dialogue::MirrorDialogueMessage::CloseMirrorDialog)
            | Message::MirrorDialogueMessages(mirror_dialogue::MirrorDialogueMessage::SaveAndClose) => {
                let target_id = self.mirror_dialog.download_id;
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == target_id) {
                    item.primary_url = self.mirror_dialog.primary_url.clone();
                    item.mirror_urls = self.mirror_dialog.mirror_urls.clone();
                }
                self.mirror_dialog.close();

                let downloads_clone = self.downloads.clone();
                return Task::perform(
                    async move {
                        storage::json_store::save_downloads(&downloads_clone)
                    },
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
                            println!("[QDM Disk Sync] Removed externally deleted completed file: {}", d.filename);
                            changed = true;
                            return false;
                        }
                    }
                    true
                });

                for d in &mut self.downloads {
                    if !matches!(d.state, DownloadState::Completed) && d.downloaded_bytes > 0 {
                        let target = std::path::Path::new(&d.save_path).join(&d.filename);
                        let temp_target = std::path::Path::new(&d.save_path).join(format!("{}.qdmdownload", d.filename));
                        if !target.exists() && !temp_target.exists() && !matches!(d.state, DownloadState::Failed { .. }) {
                            println!("[QDM Disk Sync] Partial download missing on disk: {}", d.filename);
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
                        self.settings.file_conflict_action = Some(settings::FileConflictAction::AutoRename);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.conflict_dialog.pending.take() {
                        let new_filename = crate::core::utils::paths::generate_unique_filename(&pending.save_to, &pending.filename);
                        let file_type = FileType::from_filename(&new_filename);
                        let mirror_urls: Vec<DownloadUrl> = pending.mirror_urls.into_iter().map(DownloadUrl::new).collect();
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
                            max_connections: pending.max_connections as u32,
                            speed_limit_bps: if pending.speed_limit > 0 { Some(pending.speed_limit as u64 * 1024) } else { None },
                            etag: None,
                            last_modified: None,
                            sha256_hash: None,
                            chunks: Vec::new(),
                            created_at: 0,
                            updated_at: 0,
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
                        self.settings.file_conflict_action = Some(settings::FileConflictAction::Overwrite);
                        let _ = storage::json_store::save_settings(&self.settings);
                    }
                    if let Some(pending) = self.conflict_dialog.pending.take() {
                        let file_type = FileType::from_filename(&pending.filename);
                        let mirror_urls: Vec<DownloadUrl> = pending.mirror_urls.into_iter().map(DownloadUrl::new).collect();
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
                            max_connections: pending.max_connections as u32,
                            speed_limit_bps: if pending.speed_limit > 0 { Some(pending.speed_limit as u64 * 1024) } else { None },
                            etag: None,
                            last_modified: None,
                            sha256_hash: None,
                            chunks: Vec::new(),
                            created_at: 0,
                            updated_at: 0,
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
            }

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
                        let downloads_clone = self.downloads.clone();
                        self.delete_dialog.close();
                        return Task::perform(
                            async move {
                                engine.cancel(pending.id).await;
                                storage::json_store::save_downloads(&downloads_clone)
                            },
                            Message::DownloadsPersisted,
                        );
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
                        let downloads_clone = self.downloads.clone();
                        self.delete_dialog.close();
                        return Task::perform(
                            async move {
                                engine.cancel(pending.id).await;
                                let target = std::path::Path::new(&pending.save_path).join(&pending.filename);
                                let temp_target = std::path::Path::new(&pending.save_path).join(format!("{}.qdmdownload", pending.filename));
                                if target.exists() {
                                    let _ = std::fs::remove_file(target);
                                }
                                if temp_target.exists() {
                                    let _ = std::fs::remove_file(temp_target);
                                }
                                storage::json_store::save_downloads(&downloads_clone)
                            },
                            Message::DownloadsPersisted,
                        );
                    }
                    self.delete_dialog.close();
                }
            }

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

        let second_tick_sub = iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::SecondTick);
        let disk_sync_sub = iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::SyncWithDisk);

        Subscription::batch([engine_sub, anim_sub, second_tick_sub, disk_sync_sub])
    }

    pub fn view(&self) -> Element<'_, Message> {
        let downloading_count = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Downloading { .. } | DownloadState::FetchingMetadata | DownloadState::Queued))
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

        let scheduled_count = 0;

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
            sidebar::NavFilter::Settings => "Settings",
        };

        let toolbar = toolbar::toolbar_view(
            title,
            &self.search_query,
            downloading_count,
            &total_speed_str,
            Message::SearchChanged,
            Message::AddUrlPressed,
            Message::NotificationPressed,
            Message::SettingsPressed,
        );

        let main_content: Element<Message> = if self.current_filter == sidebar::NavFilter::Settings {
            settings::settings_view(&self.settings).map(Message::SettingsMessage)
        } else {
            let filtered_items = self.downloads.iter().filter(|d| {
                let matches_filter = match self.current_filter {
                    sidebar::NavFilter::All => true,
                    sidebar::NavFilter::Downloading => matches!(d.state, DownloadState::Downloading { .. } | DownloadState::FetchingMetadata | DownloadState::Queued),
                    sidebar::NavFilter::Completed => matches!(d.state, DownloadState::Completed),
                    sidebar::NavFilter::Failed => matches!(d.state, DownloadState::Failed { .. }),
                    sidebar::NavFilter::Scheduled => false,
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

            download_list_view(
                filtered_items,
                Message::TogglePause,
                Message::CancelDownload,
                Message::OpenFolder,
                Message::OpenMirrorsModal,
            )
        };

        let right_content = column![toolbar, main_content]
            .width(Length::Fill)
            .height(Length::Fill);

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
            let conflict_modal =
                conflict_dialogue::view(&self.conflict_dialog).map(Message::ConflictDialogueMessages);

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
