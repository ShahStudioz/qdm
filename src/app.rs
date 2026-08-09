use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::theme::colors;
use crate::views;
use crate::views::add_dialog::AddDialogModel;
use crate::views::download_list::download_list_view;
use crate::views::settings::{settings_view, SettingsModel, SettingsTab};
use crate::views::sidebar::{sidebar_view, NavFilter};
use crate::views::toolbar::toolbar_view;
use iced::widget::{column, container, row, stack};
use iced::{Element, Length, Subscription, Task, Theme};

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    NavSelected(NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    NotificationPressed,
    SettingsPressed,
    TogglePause(usize),
    CancelDownload(usize),
    OpenFolder(usize),

    // Settings Messages
    SettingsTabSelected(SettingsTab),
    ToggleStartup(bool),
    ToggleTray(bool),
    FolderChanged(String),
    BrowseFolderPressed,
    StepperDecrement,
    StepperIncrement,
    ToggleNotifications(bool),
    SoundChanged(String),
    ToggleUpdates(bool),
    CheckUpdatesPressed,
    ResetDefaultsPressed,
    SaveChangesPressed,

    // Add Dialog Messages
    AddDialogueModalMessages(views::add_dialog::AddDialogueModalMessage),
}

pub struct QdmApp {
    current_filter: NavFilter,
    search_query: String,
    downloads: Vec<DownloadItem>,
    settings: SettingsModel,
    add_dialog: AddDialogModel,
}

impl Default for QdmApp {
    fn default() -> Self {
        Self {
            current_filter: NavFilter::All,
            search_query: String::new(),
            downloads: vec![
                DownloadItem {
                    id: 1,
                    filename: "ubuntu-24.04-desktop-amd64.iso".to_string(),
                    url: "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso"
                        .to_string(),
                    size_downloaded: "3.2 GB".to_string(),
                    size_total: "4.7 GB".to_string(),
                    state: DownloadState::Downloading {
                        progress: 67.0,
                        speed: "2.4 MB/s".to_string(),
                        eta: "12m 30s".to_string(),
                    },
                    file_type: FileType::Media,
                },
                DownloadItem {
                    id: 2,
                    filename: "rust-analyzer-v0.3.zip".to_string(),
                    url: "github.com/rust-lang/rust-analyzer/releases/...".to_string(),
                    size_downloaded: "245 MB".to_string(),
                    size_total: "245 MB".to_string(),
                    state: DownloadState::Completed,
                    file_type: FileType::Archive,
                },
                DownloadItem {
                    id: 3,
                    filename: "project-assets-final.tar.gz".to_string(),
                    url: "cdn.example.com/assets/v2/project-assets-final.tar.gz".to_string(),
                    size_downloaded: "840 MB".to_string(),
                    size_total: "2.0 GB".to_string(),
                    state: DownloadState::Paused { progress: 42.0 },
                    file_type: FileType::Archive,
                },
                DownloadItem {
                    id: 4,
                    filename: "nodejs-v22.0.0-win-x64.msi".to_string(),
                    url: "nodejs.org/dist/v22.0.0/nodejs-v22.0.0-win-x64.msi".to_string(),
                    size_downloaded: "7.1 MB".to_string(),
                    size_total: "31.0 MB".to_string(),
                    state: DownloadState::Downloading {
                        progress: 23.0,
                        speed: "1.8 MB/s".to_string(),
                        eta: "14s".to_string(),
                    },
                    file_type: FileType::Code,
                },
                DownloadItem {
                    id: 5,
                    filename: "database-backup-2024.sql.gz".to_string(),
                    url: "internal.server.local/backups/database-backup-2024.sql.gz".to_string(),
                    size_downloaded: "1.2 GB".to_string(),
                    size_total: "1.4 GB".to_string(),
                    state: DownloadState::Failed {
                        progress: 89.0,
                        error: "30000ms".to_string(),
                    },
                    file_type: FileType::Code,
                },
            ],
            settings: SettingsModel::default(),
            add_dialog: AddDialogModel::default(),
        }
    }
}

impl QdmApp {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.settings.tick_animation();
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
                self.current_filter = NavFilter::Settings;
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
                }
            }
            Message::CancelDownload(id) => {
                self.downloads.retain(|d| d.id != id);
            }
            Message::OpenFolder(id) => {
                println!("[QDM] Open folder for item {}", id);
            }

            // Settings Handlers
            Message::SettingsTabSelected(tab) => {
                self.settings.active_tab = tab;
            }
            Message::ToggleStartup(val) => {
                self.settings.launch_at_startup = val;
            }
            Message::ToggleTray(val) => {
                self.settings.minimize_to_tray = val;
            }
            Message::FolderChanged(folder) => {
                self.settings.download_folder = folder;
            }
            Message::BrowseFolderPressed => {
                println!("[QDM] Browse download folder pressed");
            }
            Message::StepperDecrement => {
                if self.settings.simultaneous_downloads > 1 {
                    self.settings.simultaneous_downloads -= 1;
                }
            }
            Message::StepperIncrement => {
                if self.settings.simultaneous_downloads < 16 {
                    self.settings.simultaneous_downloads += 1;
                }
            }
            Message::ToggleNotifications(val) => {
                self.settings.show_notifications = val;
            }
            Message::SoundChanged(sound) => {
                self.settings.notification_sound = sound;
            }
            Message::ToggleUpdates(val) => {
                self.settings.auto_check_updates = val;
            }
            Message::CheckUpdatesPressed => {
                println!("[QDM] Checking for updates...");
            }
            Message::ResetDefaultsPressed => {
                self.settings = SettingsModel::default();
            }
            Message::SaveChangesPressed => {
                println!("[QDM] Settings saved: {:?}", self.settings.download_folder);
            }

            // Add Dialog Handlers
            Message::AddDialogueModalMessages(views::add_dialog::AddDialogueModalMessage::SubmitNewDownload) => {
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
                    .map(views::add_dialog::format_bytes)
                    .unwrap_or_else(|| "Unknown".to_string());

                let file_type = FileType::from_filename(&filename);
                let new_id = self.downloads.iter().map(|d| d.id).max().unwrap_or(0) + 1;

                let new_item = DownloadItem {
                    id: new_id,
                    filename,
                    url: self.add_dialog.url.clone(),
                    size_downloaded: "0 B".to_string(),
                    size_total,
                    state: DownloadState::Downloading {
                        progress: 0.0,
                        speed: "0 B/s".to_string(),
                        eta: "Connecting...".to_string(),
                    },
                    file_type,
                };

                self.downloads.insert(0, new_item);
                self.add_dialog.is_open = false;
                self.add_dialog.reset();
            }
            Message::AddDialogueModalMessages(message) => {
                return views::add_dialog::update(&mut self.add_dialog, message)
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
        // Counts for badges
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

        // 1. Sidebar View
        let sidebar = sidebar_view(
            self.current_filter,
            downloading_count,
            completed_count,
            failed_count,
            scheduled_count,
            Message::NavSelected,
        );

        // Title string for toolbar
        let title = match self.current_filter {
            NavFilter::All => "All Downloads",
            NavFilter::Downloading => "Active Downloads",
            NavFilter::Completed => "Completed Downloads",
            NavFilter::Failed => "Failed Downloads",
            NavFilter::Scheduled => "Scheduled Downloads",
            NavFilter::Settings => "Settings",
        };

        // 2. Toolbar View
        let toolbar = toolbar_view(
            title,
            &self.search_query,
            downloading_count,
            "12.4 MB/s",
            Message::SearchChanged,
            Message::AddUrlPressed,
            Message::NotificationPressed,
            Message::SettingsPressed,
        );

        // 3. Main Content View: Settings view if NavFilter::Settings, otherwise Download List
        let main_content: Element<Message> = if self.current_filter == NavFilter::Settings {
            settings_view(
                &self.settings,
                Message::SettingsTabSelected,
                Message::ToggleStartup,
                Message::ToggleTray,
                Message::FolderChanged,
                Message::BrowseFolderPressed,
                Message::StepperDecrement,
                Message::StepperIncrement,
                Message::ToggleNotifications,
                Message::SoundChanged,
                Message::ToggleUpdates,
                Message::CheckUpdatesPressed,
                Message::ResetDefaultsPressed,
                Message::SaveChangesPressed,
            )
        } else {
            let filtered_items = self.downloads.iter().filter(|d| {
                let matches_filter = match self.current_filter {
                    NavFilter::All => true,
                    NavFilter::Downloading => matches!(d.state, DownloadState::Downloading { .. }),
                    NavFilter::Completed => matches!(d.state, DownloadState::Completed),
                    NavFilter::Failed => matches!(d.state, DownloadState::Failed { .. }),
                    NavFilter::Scheduled => false,
                    NavFilter::Settings => false,
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

        // Right side layout: Toolbar + Main Content
        let right_content = column![toolbar, main_content]
            .width(Length::Fill)
            .height(Length::Fill);

        // Overall root layout: Sidebar + Right Content
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

        // 4. Modal Overlay Stack if Add Dialog is open
        if self.add_dialog.is_open {
            // Map the dialog's local Message enum to the parent's Message enum
            let dialog_modal =
                views::add_dialog::view(&self.add_dialog).map(Message::AddDialogueModalMessages);

            stack![base_view, dialog_modal].into()
        } else {
            base_view.into()
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
