use iced::widget::{column, container, row};
use iced::{Element, Length, Theme};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::theme::colors;
use crate::views::download_list::download_list_view;
use crate::views::sidebar::{sidebar_view, NavFilter};
use crate::views::toolbar::toolbar_view;

#[derive(Debug, Clone)]
pub enum Message {
    NavSelected(NavFilter),
    SearchChanged(String),
    AddUrlPressed,
    NotificationPressed,
    SettingsPressed,
    TogglePause(usize),
    CancelDownload(usize),
    OpenFolder(usize),
}

pub struct QdmApp {
    current_filter: NavFilter,
    search_query: String,
    downloads: Vec<DownloadItem>,
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
                    url: "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso".to_string(),
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
        }
    }
}

impl QdmApp {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::NavSelected(filter) => {
                self.current_filter = filter;
            }
            Message::SearchChanged(query) => {
                self.search_query = query;
            }
            Message::AddUrlPressed => {
                println!("[QDM] Add URL clicked");
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

        // 3. Filter downloads directly as iterator
        let filtered_items = self.downloads.iter().filter(|d| {
            let matches_filter = match self.current_filter {
                NavFilter::All => true,
                NavFilter::Downloading => matches!(d.state, DownloadState::Downloading { .. }),
                NavFilter::Completed => matches!(d.state, DownloadState::Completed),
                NavFilter::Failed => matches!(d.state, DownloadState::Failed { .. }),
                NavFilter::Scheduled => false,
                NavFilter::Settings => true,
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

        // 4. Main List View
        let main_list = download_list_view(
            filtered_items,
            Message::TogglePause,
            Message::CancelDownload,
            Message::OpenFolder,
        );

        // Right side layout: Toolbar + Main List
        let right_content = column![toolbar, main_list].width(Length::Fill).height(Length::Fill);

        // Overall root layout: Sidebar + Right Content
        let root_layout = row![sidebar, right_content].width(Length::Fill).height(Length::Fill);

        container(root_layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::BACKGROUND)),
                text_color: Some(colors::TEXT_PRIMARY),
                ..Default::default()
            })
            .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }
}
