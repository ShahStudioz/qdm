use iced::widget::{button, checkbox, column, container, row, text, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

#[derive(Debug, Clone)]
pub struct ConflictPendingDownload {
    pub url: String,
    pub filename: String,
    pub save_to: String,
    pub max_connections: usize,
    pub speed_limit: Option<u64>,
    pub mirror_urls: Vec<String>,
    pub is_torrent_folder: bool,
    pub download_type: Option<crate::models::download::DownloadType>,
    pub total_bytes: Option<u64>,
    pub is_duplicate_listing: bool,
}

#[derive(Debug, Clone)]
pub struct ConflictDialogModel {
    pub is_open: bool,
    pub pending: Option<ConflictPendingDownload>,
    pub remember_choice: bool,
    pub remaining_secs: u32,
}

impl Default for ConflictDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            pending: None,
            remember_choice: false,
            remaining_secs: 30,
        }
    }
}

impl ConflictDialogModel {
    pub fn open(&mut self, pending: ConflictPendingDownload) {
        self.is_open = true;
        self.pending = Some(pending);
        self.remember_choice = false;
        self.remaining_secs = 30;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.pending = None;
        self.remember_choice = false;
        self.remaining_secs = 30;
    }

    pub fn tick_second(&mut self) -> bool {
        if self.is_open && self.remaining_secs > 0 {
            self.remaining_secs -= 1;
            if self.remaining_secs == 0 {
                return true; // Countdown expired, should trigger auto-rename
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub enum ConflictDialogMessage {
    Close,
    ToggleRemember(bool),
    AutoRenameChosen,
    OverwriteChosen,
}

pub fn view(state: &ConflictDialogModel) -> Element<'_, ConflictDialogMessage> {
    let raw_filename = state
        .pending
        .as_ref()
        .map(|p| p.filename.as_str())
        .unwrap_or("file");
    let filename = crate::models::download::truncate_filename(raw_filename, 48);

    let save_to = state
        .pending
        .as_ref()
        .map(|p| p.save_to.as_str())
        .unwrap_or("");

    let is_folder = state
        .pending
        .as_ref()
        .map(|p| p.is_torrent_folder)
        .unwrap_or(false);

    let is_duplicate = state
        .pending
        .as_ref()
        .map(|p| p.is_duplicate_listing)
        .unwrap_or(false);

    let title_label = if is_duplicate {
        "Duplicate Download Detected"
    } else if is_folder {
        "Folder Conflict Detected"
    } else {
        "File Conflict Detected"
    };
    let title_text = text(title_label)
        .size(16)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let close_btn = button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(ConflictDialogMessage::Close);

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

    let auto_timer_banner = container(
        row![
            icon(icons::ICON_CLOCK).size(14).color(colors::WARNING),
            text(format!(
                "Auto-renaming and downloading in {}s if no action taken",
                state.remaining_secs
            ))
            .size(12)
            .font(styles::BOLD_FONT)
            .color(colors::WARNING)
            .width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(iced::Background::Color(iced::Color::from_rgba(0.95, 0.70, 0.20, 0.10))),
        border: iced::Border {
            color: iced::Color::from_rgba(0.95, 0.70, 0.20, 0.35),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    let detail_description = if is_duplicate {
        "This download is already in your download list:"
    } else if is_folder {
        "A folder with this name already exists in destination:"
    } else {
        "A file or partial download with this name already exists in destination:"
    };
    let file_detail_box = container(
        column![
            text(detail_description)
                .size(12)
                .color(colors::TEXT_MUTED),
            text(filename)
                .size(13)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY),
            text(save_to)
                .size(11)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(4),
    )
    .padding([10, 14])
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

    let question_text = text("How would you like to handle this download?")
        .size(13)
        .color(colors::TEXT_PRIMARY);

    let remember_chk = checkbox("Remember my choice for future downloads", state.remember_choice)
        .on_toggle(ConflictDialogMessage::ToggleRemember)
        .size(16)
        .text_size(13)
        .style(|_theme, _status| checkbox::Style {
            background: iced::Background::Color(colors::SURFACE_HIGH),
            icon_color: colors::PRIMARY,
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 4.0.into(),
            },
            text_color: Some(colors::TEXT_PRIMARY),
        });

    let cancel_btn = button(text("Cancel").size(13).color(colors::TEXT_MUTED))
        .padding([8, 14])
        .style(styles::ghost_button_style)
        .on_press(ConflictDialogMessage::Close);

    let overwrite_btn = button(
        row![
            icon(icons::ICON_WARN).size(14).color(colors::WARNING),
            text("Overwrite").size(13).font(styles::BOLD_FONT).color(colors::WARNING),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .style(styles::ghost_button_style)
    .on_press(ConflictDialogMessage::OverwriteChosen);

    let rename_btn_label = if is_duplicate {
        if state.remaining_secs > 0 {
            format!("Rename & Download ({}s)", state.remaining_secs)
        } else {
            "Rename & Download".to_string()
        }
    } else if state.remaining_secs > 0 {
        format!("Auto-Rename & Download ({}s)", state.remaining_secs)
    } else {
        "Auto-Rename & Download".to_string()
    };

    let rename_btn = button(
        row![
            icon(icons::ICON_PLUS).size(14).color(colors::BACKGROUND),
            text(rename_btn_label).size(13).font(styles::BOLD_FONT).color(colors::BACKGROUND),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(styles::primary_button_style)
    .on_press(ConflictDialogMessage::AutoRenameChosen);

    let buttons_row = if is_duplicate {
        row![
            cancel_btn,
            Space::with_width(Length::Fill),
            rename_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    } else {
        row![
            cancel_btn,
            Space::with_width(Length::Fill),
            overwrite_btn,
            rename_btn,
        ]
        .spacing(10)
        .align_y(Alignment::Center)
    };

    let mut content_col = column![
        auto_timer_banner,
        file_detail_box,
        question_text,
    ];
    if !is_duplicate {
        content_col = content_col.push(remember_chk);
    }
    let content_col = content_col
        .push(Space::with_height(6))
        .push(buttons_row)
        .spacing(12)
        .padding([20, 20]);

    let modal_card = container(column![header_row, header_divider, content_col])
        .width(520)
        .style(styles::card_style);

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.65,
            ))),
            ..Default::default()
        })
        .into()
}
