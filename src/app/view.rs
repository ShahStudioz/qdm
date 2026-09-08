//! View rendering for the QDM application.
//!
//! Contains the main `view()` method that composes the sidebar, toolbar,
//! content area, and modal overlays into the final application layout,
//! as well as the `theme()` method.

use super::{Message, QdmApp};
use crate::icons::{self, icon};
use crate::models::download::DownloadState;
use crate::theme::{colors, styles};
use crate::views::components::{sidebar, title_bar, toolbar};
use crate::views::dialogues::{add_dialogue, conflict_dialogue, delete_dialogue, mirror_dialogue};
use crate::views::downloads::download_list::download_list_view;
use crate::views::settings::settings;
use iced::widget::{column, container, row, stack, text};
use iced::{Alignment, Element, Length, Theme};

impl QdmApp {
    /// Renders the complete application UI.
    ///
    /// Composes the sidebar navigation, toolbar, main content area (download list,
    /// queue view, or settings), schedule banner, and any active modal dialogs.
    pub fn view(&self) -> Element<'_, Message> {
        // --- Sidebar badge counts ---
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

        // --- Aggregate speeds for toolbar ---
        let total_download_speed_bps: u64 = self
            .downloads
            .iter()
            .filter_map(|d| match &d.state {
                DownloadState::Downloading { speed_bps, .. } => Some(*speed_bps),
                _ => None,
            })
            .sum();

        let total_upload_speed_bps: u64 = self
            .downloads
            .iter()
            .filter_map(|d| match &d.state {
                DownloadState::Downloading { .. } => {
                    d.torrent_meta().map(|t| t.upload_speed_bps)
                }
                _ => None,
            })
            .sum();

        // --- Sidebar ---
        let sidebar = sidebar::sidebar_view(
            self.current_filter,
            downloading_count,
            completed_count,
            failed_count,
            scheduled_count,
            Message::NavSelected,
        );

        // --- Toolbar ---
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
            total_download_speed_bps,
            total_upload_speed_bps,
            self.is_topbar_menu_open,
            Message::SearchChanged,
            Message::AddUrlPressed,
            Message::ToggleTopbarMenu,
        );

        // --- Schedule banner (shown on the Scheduled tab) ---
        let schedule_banner: Option<Element<Message>> = self.build_schedule_banner();

        // --- Main content area ---
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
                            || d.get_url()
                                .to_lowercase()
                                .contains(&self.search_query.to_lowercase())
                    };

                    matches_filter && matches_search
                });

                let list_view = download_list_view(
                    filtered_items,
                    &self.copied_link_ids,
                    Message::TogglePause,
                    Message::CancelDownload,
                    Message::OpenFolder,
                    Message::OpenMirrorsModal,
                    Message::CopyLink,
                    Message::ItemClicked,
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

        // --- Layout composition ---
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

        let title_bar = title_bar::title_bar_view(
            self.is_maximized,
            Message::WindowDragPressed,
            Message::WindowMinimizePressed,
            Message::WindowToggleMaximizePressed,
            Message::WindowClosePressed,
        );

        // --- Modal overlays (highest priority on top of main content area) ---
        let main_area: Element<Message> = if self.conflict_dialog.is_open {
            let conflict_modal = conflict_dialogue::view(&self.conflict_dialog)
                .map(Message::ConflictDialogueMessages);
            stack![root_layout, conflict_modal]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.delete_dialog.is_open {
            let delete_modal =
                delete_dialogue::view(&self.delete_dialog).map(Message::DeleteDialogueMessages);
            stack![root_layout, delete_modal]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.add_dialog.is_open {
            let dialog_modal =
                add_dialogue::view(&self.add_dialog).map(Message::AddDialogueModalMessages);
            stack![root_layout, dialog_modal]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else if self.mirror_dialog.is_open {
            let mirror_modal =
                mirror_dialogue::view(&self.mirror_dialog).map(Message::MirrorDialogueMessages);
            stack![root_layout, mirror_modal]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            root_layout.into()
        };

        let window_content = column![title_bar, main_area]
            .width(Length::Fill)
            .height(Length::Fill);

        container(window_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::BACKGROUND)),
                text_color: Some(colors::TEXT_PRIMARY),
                border: iced::Border {
                    color: colors::BORDER,
                    width: if self.is_maximized { 0.0 } else { 1.0 },
                    radius: if self.is_maximized { 0.0.into() } else { 8.0.into() },
                },
                ..Default::default()
            })
            .into()
    }

    /// Returns the application theme.
    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    /// Builds the schedule status banner shown on the Scheduled tab.
    ///
    /// Shows whether the scheduler is active, the current window status,
    /// and countdown to the next window if applicable.
    fn build_schedule_banner(&self) -> Option<Element<'_, Message>> {
        if self.current_filter != sidebar::NavFilter::Scheduled {
            return None;
        }

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
                radius: 8.0.into(),
                color: border_col,
                ..Default::default()
            },
            ..Default::default()
        });

        Some(
            container(banner)
                .padding(iced::padding::top(16).bottom(0).left(24).right(24))
                .into(),
        )
    }
}
