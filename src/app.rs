use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::services::storage;
use crate::theme::colors;
use crate::views::components::{sidebar, toolbar};
use crate::views::dialogues::add_dialogue;
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

    NavSelected(sidebar::NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    NotificationPressed,
    SettingsPressed,

    TogglePause(usize),
    CancelDownload(usize),
    OpenFolder(usize),

    SettingsMessage(settings::SettingsMessage),
    AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage),
}

pub struct QdmApp {
    current_filter: sidebar::NavFilter,
    search_query: String,
    downloads: Vec<DownloadItem>,
    settings: settings::SettingsModel,
    add_dialog: add_dialogue::AddDialogModel,
}

impl Default for QdmApp {
    fn default() -> Self {
        Self {
            current_filter: sidebar::NavFilter::All,
            search_query: String::new(),
            downloads: Vec::new(),
            settings: settings::SettingsModel::default(),
            add_dialog: add_dialogue::AddDialogModel::default(),
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
            Message::TogglePause(id) => {
                if let Some(item) = self.downloads.iter_mut().find(|d| d.id == id) {
                    match item.state {
                        DownloadState::Downloading { progress, .. } => {
                            item.state = DownloadState::Paused { progress };
                        }
                        DownloadState::Paused { progress } => {
                            item.state = DownloadState::Downloading {
                                progress,
                                speed: "2.4 MB/s".to_string(),
                                eta: "10m 00s".to_string(),
                            };
                        }
                        DownloadState::Failed { .. } => {
                            item.state = DownloadState::Downloading {
                                progress: 89.0,
                                speed: "1.5 MB/s".to_string(),
                                eta: "2m 15s".to_string(),
                            };
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
            Message::CancelDownload(id) => {
                self.downloads.retain(|d| d.id != id);
                let downloads_clone = self.downloads.clone();
                return Task::perform(
                    async move {
                        storage::json_store::save_downloads(&downloads_clone)
                    },
                    Message::DownloadsPersisted,
                );
            }
            Message::OpenFolder(id) => {
                println!("[QDM] Open folder for item {}", id);
            }

            Message::SettingsMessage(msg) => {
                settings::update(&mut self.settings, msg);
            }

            Message::AddDialogueModalMessages(add_dialogue::AddDialogueModalMessage::SubmitNewDownload) => {
                let filename = if self.add_dialog.filename.trim().is_empty() {
                    "download.file".to_string()
                } else {
                    self.add_dialog.filename.trim().to_string()
                };

                let size_total = self
                    .add_dialog
                    .download_file_metadata
                    .as_ref()
                    .and_then(|m| m.content_length)
                    .map(add_dialogue::format_bytes)
                    .unwrap_or_else(|| "Unknown".to_string());

                let file_type = FileType::from_filename(&filename);
                let save_path = self.add_dialog.save_to.clone();

                let new_item = DownloadItem {
                    id: 0,
                    filename,
                    url: self.add_dialog.url.clone(),
                    save_path,
                    size_downloaded: "0 B".to_string(),
                    size_total,
                    state: DownloadState::Downloading {
                        progress: 0.0,
                        speed: "0 B/s".to_string(),
                        eta: "Connecting...".to_string(),
                    },
                    file_type,
                    created_at: 0,
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
                self.downloads.insert(0, inserted_item);
            }
            Message::DownloadSaved(Err(err)) => {
                println!("[QDM Storage Error] Failed to save download: {}", err);
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
        if self.settings.is_animating() {
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let downloading_count = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Downloading { .. }))
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
            "0.0 MB/s",
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
                    sidebar::NavFilter::Downloading => matches!(d.state, DownloadState::Downloading { .. }),
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
                        || d.url
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
        } else {
            base_view.into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
