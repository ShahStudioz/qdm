use crate::models::download::{DownloadItem, DownloadState, DownloadUrl, FileType};
use crate::services::downloads::{DownloadEngine, EngineUiEvent, FileMetadata};
use crate::services::storage;
use crate::theme::colors;
use crate::views::components::{sidebar, toolbar};
use crate::views::dialogues::{add_dialogue, mirror_dialogue};
use crate::views::downloads::download_list::download_list_view;
use crate::views::settings::settings;
use iced::widget::{column, container, row, stack};
use iced::{Element, Length, Subscription, Task, Theme};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
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
}

pub struct QdmApp {
    current_filter: sidebar::NavFilter,
    search_query: String,
    downloads: Vec<DownloadItem>,
    settings: settings::SettingsModel,
    add_dialog: add_dialogue::AddDialogModel,
    mirror_dialog: mirror_dialogue::MirrorDialogModel,
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
                self.downloads.retain(|d| d.id != id);
                let engine = self.engine.clone();
                let downloads_clone = self.downloads.clone();
                return Task::perform(
                    async move {
                        engine.cancel(id).await;
                        storage::json_store::save_downloads(&downloads_clone)
                    },
                    Message::DownloadsPersisted,
                );
            }
            Message::OpenFolder(id) => {
                if let Some(item) = self.downloads.iter().find(|d| d.id == id) {
                    let _ = std::process::Command::new("explorer")
                        .arg(&item.save_path)
                        .spawn();
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

            Message::SettingsMessage(msg) => {
                settings::update(&mut self.settings, msg);
            }

            Message::AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage::SubmitNewDownload) => {
                let url = self.add_dialog.url.trim().to_string();
                if url.is_empty() {
                    return Task::none();
                }

                let filename = if self.add_dialog.filename.trim().is_empty() {
                    add_dialogue::extract_filename(&url, None)
                } else {
                    self.add_dialog.filename.trim().to_string()
                };

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
                let save_path = self.add_dialog.save_to.clone();

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

                let filename = add_dialogue::extract_filename(&url, None);
                let file_type = FileType::from_filename(&filename);
                let save_path = self.add_dialog.save_to.clone();
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

        Subscription::batch([engine_sub, anim_sub])
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

        if self.add_dialog.is_open {
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
