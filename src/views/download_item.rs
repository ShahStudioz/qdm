use iced::widget::{button, column, container, progress_bar, row, text, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::theme::{colors, styles};

pub fn download_item_view<'a, Message>(
    item: &'a DownloadItem,
    on_toggle_pause: impl Fn(usize) -> Message + 'a,
    on_cancel: impl Fn(usize) -> Message + 'a,
    on_open_folder: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // 1. File Icon & Box Styling based on type and state
    let (file_icon_char, icon_color, box_bg) = match item.file_type {
        FileType::Media => (icons::ICON_DISC, colors::PRIMARY, colors::SURFACE_HIGH),
        FileType::Archive => (icons::ICON_ZIP, colors::SUCCESS, colors::SURFACE_HIGH),
        FileType::Code => match item.state {
            DownloadState::Failed { .. } => (icons::ICON_DATABASE, colors::ERROR, colors::FAILED_BG),
            _ => (icons::ICON_GRID, colors::PRIMARY, colors::SURFACE_HIGH),
        },
        FileType::Document | FileType::Other => (icons::ICON_BOX, colors::TEXT_MUTED, colors::SURFACE_HIGH),
    };

    let icon_box = container(
        icon(file_icon_char).size(22).color(icon_color)
    )
    .width(52)
    .height(52)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(box_bg)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    });

    // 2. Top Header Row: Filename + Badge Pill + Action Buttons (top right)
    let filename_text = text(&item.filename)
        .size(15)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let mut header_left = row![filename_text].spacing(10).align_y(Alignment::Center);

    // Optional State Badge Pill next to filename
    match &item.state {
        DownloadState::Completed => {
            let badge = container(
                text("COMPLETED").size(10).font(styles::BOLD_FONT).color(colors::BACKGROUND)
            )
            .padding([2, 8])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SUCCESS)),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            });
            header_left = header_left.push(badge);
        }
        DownloadState::Paused { .. } => {
            let badge = container(
                text("PAUSED").size(10).font(styles::BOLD_FONT).color(colors::BACKGROUND)
            )
            .padding([2, 8])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::WARNING)),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            });
            header_left = header_left.push(badge);
        }
        DownloadState::Failed { .. } => {
            let badge = container(
                text("FAILED").size(10).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY)
            )
            .padding([2, 8])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::ERROR)),
                border: iced::Border { radius: 10.0.into(), ..Default::default() },
                ..Default::default()
            });
            header_left = header_left.push(badge);
        }
        _ => {}
    }

    // Top Right Action Buttons
    let item_id = item.id;
    let actions_row: Element<Message> = match &item.state {
        DownloadState::Downloading { .. } => {
            let pause_btn = button(icon(icons::ICON_PAUSE).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let cancel_btn = button(icon(icons::ICON_CANCEL).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            row![pause_btn, cancel_btn].spacing(12).into()
        }
        DownloadState::Completed => {
            let folder_btn = button(icon(icons::ICON_FOLDER).size(14))
                .style(styles::icon_button_style)
                .on_press(on_open_folder(item_id));
            row![folder_btn].into()
        }
        DownloadState::Paused { .. } => {
            let play_btn = button(icon(icons::ICON_PLAY).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let cancel_btn = button(icon(icons::ICON_CANCEL).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            row![play_btn, cancel_btn].spacing(12).into()
        }
        DownloadState::Failed { .. } => {
            let retry_btn = button(icon(icons::ICON_RETRY).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let trash_btn = button(icon(icons::ICON_TRASH).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            row![retry_btn, trash_btn].spacing(12).into()
        }
    };

    let top_row = row![
        header_left,
        Space::with_width(Length::Fill),
        actions_row,
    ]
    .align_y(Alignment::Center);

    // 3. Second Row: URL & optional Error message
    let url_text = text(&item.url)
        .size(12)
        .font(styles::MONO_FONT)
        .color(colors::TEXT_MUTED);

    let second_row_col: Element<Message> = if let DownloadState::Failed { error, .. } = &item.state {
        let err_row = row![
            icon(icons::ICON_WARN).size(13).color(colors::ERROR),
            text(format!("Connection timed out after {}", error))
                .size(12)
                .color(colors::ERROR),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        column![url_text, err_row].spacing(6).into()
    } else {
        url_text.into()
    };

    // 4. Third Row: Progress Bar section
    let (progress_val, bar_color, size_detail, speed_eta_col) = match &item.state {
        DownloadState::Downloading { progress, speed, eta } => (
            *progress,
            colors::PRIMARY,
            format!("{} / {}", item.size_downloaded, item.size_total),
            Some(column![
                text(speed).size(13).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY),
                text(format!("{} left", eta)).size(11).color(colors::TEXT_MUTED),
            ].align_x(Alignment::End)),
        ),
        DownloadState::Completed => (
            100.0,
            colors::SUCCESS,
            "245 MB  ·  Today, 14:32".to_string(),
            None,
        ),
        DownloadState::Paused { progress } => (
            *progress,
            colors::WARNING,
            format!("{} / {}", item.size_downloaded, item.size_total),
            Some(column![
                text("0.0 KB/s").size(13).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
                text("--").size(11).color(colors::TEXT_MUTED),
            ].align_x(Alignment::End)),
        ),
        DownloadState::Failed { progress, .. } => (
            *progress,
            colors::ERROR,
            format!("Failed at {:.0}% \n{} / {}", progress, item.size_downloaded, item.size_total),
            None,
        ),
    };

    let percent_text = text(format!("{:.0}%", progress_val))
        .size(12)
        .font(styles::BOLD_FONT)
        .color(bar_color);

    let pbar = progress_bar(0.0..=100.0, progress_val)
        .height(6)
        .style(styles::progress_bar_style_with_color(bar_color));

    let size_text = text(size_detail)
        .size(11)
        .font(styles::MONO_FONT)
        .color(colors::TEXT_MUTED);

    let progress_header = row![
        percent_text,
        Space::with_width(Length::Fill),
        size_text,
    ]
    .align_y(Alignment::Center);

    let progress_bar_col = column![
        progress_header,
        pbar,
    ]
    .spacing(4)
    .width(Length::Fill);

    let bottom_row: Element<Message> = if let Some(speed_col) = speed_eta_col {
        row![
            progress_bar_col,
            Space::with_width(24),
            speed_col,
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        progress_bar_col.into()
    };

    // Right Column assembly (Main Card Content)
    let card_content = column![
        top_row,
        Space::with_height(4),
        second_row_col,
        Space::with_height(8),
        bottom_row,
    ]
    .spacing(2)
    .width(Length::Fill);

    let card_layout = row![
        icon_box,
        card_content,
    ]
    .spacing(16)
    .align_y(Alignment::Start);

    // Apply Card Container Style based on State
    let card_container = container(card_layout)
        .width(Length::Fill)
        .padding([16, 20])
        .style(move |theme| match &item.state {
            DownloadState::Completed => styles::completed_card_style(theme),
            DownloadState::Failed { .. } => styles::failed_card_style(theme),
            _ => styles::card_style(theme),
        });

    card_container.into()
}
