use crate::icons::{self, icon};
use crate::models::download::{format_bytes, truncate_filename, DownloadItem, DownloadState};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone, Default)]
pub struct UpdateConflictDialogModel {
    pub is_open: bool,
    pub active_non_resumable: Vec<DownloadItem>,
}

impl UpdateConflictDialogModel {
    pub fn open(&mut self, active_non_resumable: Vec<DownloadItem>) {
        self.is_open = true;
        self.active_non_resumable = active_non_resumable;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.active_non_resumable.clear();
    }
}

#[derive(Debug, Clone)]
pub enum UpdateConflictDialogMessage {
    Close,
    ProceedWithInstall,
}

pub fn view(state: &UpdateConflictDialogModel) -> Element<'_, UpdateConflictDialogMessage> {
    let title_text = text("Non-Resumable Downloads Active")
        .size(16)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let close_btn = button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(UpdateConflictDialogMessage::Close);

    let header_row = row![
        icon(icons::ICON_WARN).size(18).color(colors::WARNING),
        title_text,
        Space::with_width(Length::Fill),
        close_btn,
    ]
    .spacing(10)
    .padding([16, 20])
    .align_y(Alignment::Center);

    let header_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let warning_banner = container(
        row![
            icon(icons::ICON_WARN).size(15).color(colors::WARNING),
            text("Installing this update requires restarting QDM. The following downloads do NOT support resuming and will be lost if stopped:")
                .size(12)
                .color(colors::WARNING)
                .width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(0.95, 0.70, 0.20, 0.12))),
        border: iced::Border {
            color: iced::Color::from_rgba(0.95, 0.70, 0.20, 0.35),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    let mut items_col = column![].spacing(8).width(Length::Fill);
    for item in &state.active_non_resumable {
        let name = truncate_filename(&item.filename, 42);
        let progress_info = match &item.state {
            DownloadState::Downloading {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                let down_str = format_bytes(*downloaded_bytes);
                if let Some(total) = total_bytes {
                    let pct = if *total > 0 {
                        (*downloaded_bytes as f64 / *total as f64) * 100.0
                    } else {
                        0.0
                    };
                    format!("{} of {} ({:.1}%)", down_str, format_bytes(*total), pct)
                } else {
                    format!("{} downloaded (unknown total)", down_str)
                }
            }
            _ => format!("{} downloaded", format_bytes(item.downloaded_bytes)),
        };

        let row_item = container(
            row![
                icon(icons::ICON_FILE).size(14).color(colors::TEXT_MUTED),
                column![
                    text(name)
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_PRIMARY),
                    text(progress_info).size(11).color(colors::TEXT_MUTED),
                ]
                .spacing(2),
                Space::with_width(Length::Fill),
                container(
                    text("Non-Resumable")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::ERROR)
                )
                .padding([2, 6])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgba(
                        0.93, 0.26, 0.26, 0.15
                    ))),
                    border: iced::Border {
                        color: colors::ERROR,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

        items_col = items_col.push(row_item);
    }

    let items_scroll = scrollable(items_col)
        .height(Length::Shrink)
        .style(styles::scrollable_style);

    let cancel_btn = button(
        text("Wait & Finish Downloads First")
            .size(13)
            .color(colors::TEXT_PRIMARY),
    )
    .padding([8, 16])
    .style(styles::ghost_button_style)
    .on_press(UpdateConflictDialogMessage::Close);

    let proceed_btn = button(
        row![
            icon(icons::ICON_CANCEL).size(13).color(iced::Color::WHITE),
            text("Cancel Downloads & Update Now")
                .size(13)
                .font(styles::BOLD_FONT)
                .color(iced::Color::WHITE),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(|_theme, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => {
                iced::Color::from_rgb(0.85, 0.20, 0.20)
            }
            _ => colors::ERROR,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: iced::Color::WHITE,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            shadow: Default::default(),
        }
    })
    .on_press(UpdateConflictDialogMessage::ProceedWithInstall);

    let buttons_row = row![cancel_btn, Space::with_width(Length::Fill), proceed_btn,]
        .spacing(12)
        .align_y(Alignment::Center);

    let content_col = column![
        warning_banner,
        items_scroll,
        Space::with_height(4),
        buttons_row,
    ]
    .spacing(14)
    .padding([20, 20]);

    let modal_card = container(column![header_row, header_divider, content_col])
        .width(540)
        .style(styles::card_style);

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(styles::modal_backdrop_style)
        .into()
}
