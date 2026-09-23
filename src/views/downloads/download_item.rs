use crate::icons::{self, icon};
use crate::models::download::{
    format_eta, format_speed, truncate_filename, DownloadItem, DownloadState, FileType,
};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, mouse_area, progress_bar, row, text, Space};
use iced::{Alignment, Element, Length};

fn make_badge<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    bg_color: iced::Color,
    border_color: Option<iced::Color>,
) -> Element<'a, Message> {
    container(content)
        .height(Length::Fixed(20.0))
        .align_y(Alignment::Center)
        .padding([2, 8])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(bg_color)),
            border: iced::Border {
                color: border_color.unwrap_or(iced::Color::TRANSPARENT),
                width: if border_color.is_some() { 1.0 } else { 0.0 },
                radius: 10.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn make_limit_badge<'a, Message: 'a>(limit_bps: u64) -> Element<'a, Message> {
    container(
        row![
            icon(icons::ICON_GAUGE).size(10).color(colors::PRIMARY),
            text(format!("LIMIT {}", format_speed(limit_bps)))
                .size(9)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .height(Length::Fixed(20.0))
    .align_y(Alignment::Center)
    .padding([2, 6])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .into()
}

pub fn download_item_view<'a, Message>(
    item: &'a DownloadItem,
    is_copied: bool,
    on_toggle_pause: impl Fn(usize) -> Message + 'a,
    on_cancel: impl Fn(usize) -> Message + 'a,
    on_open_folder: impl Fn(usize) -> Message + 'a,
    on_open_mirrors: impl Fn(usize) -> Message + 'a,
    on_open_details: impl Fn(usize) -> Message + 'a,
    on_copy_link: impl Fn(usize) -> Message + 'a,
    on_item_click: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let (file_icon_char, icon_color, box_bg) = if item.is_folder() {
        (icons::ICON_FOLDER, colors::WARNING, colors::SURFACE_HIGH)
    } else {
        match item.file_type {
            FileType::Media => (icons::ICON_MEDIA, colors::PRIMARY, colors::SURFACE_HIGH),
            FileType::Archive => (icons::ICON_ZIP, colors::SUCCESS, colors::SURFACE_HIGH),
            FileType::Code => (icons::ICON_CODE, colors::TORRENT, colors::SURFACE_HIGH),
            FileType::Document => (icons::ICON_DOCUMENT, colors::PRIMARY, colors::SURFACE_HIGH),
            FileType::Other => {
                if item.torrent_meta().is_some() {
                    (icons::ICON_MAGNET, colors::TORRENT, colors::SURFACE_HIGH)
                } else {
                    (icons::ICON_FILE, colors::TEXT_MUTED, colors::SURFACE_HIGH)
                }
            }
        }
    };

    let icon_box = container(icon(file_icon_char).size(22).color(icon_color))
        .width(52)
        .height(52)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(box_bg)),
            border: iced::Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let display_name = truncate_filename(&item.filename, 60);
    let filename_text = text(display_name)
        .size(15)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let mut header_left = row![filename_text].spacing(10).align_y(Alignment::Center);

    if item.torrent_meta().is_some() {
        let torrent_badge = make_badge(
            row![
                icon(icons::ICON_MAGNET).size(10).color(colors::BACKGROUND),
                text("TORRENT")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            colors::TORRENT,
            None,
        );
        header_left = header_left.push(torrent_badge);
    }

    if item.is_scheduled && !matches!(item.state, DownloadState::Scheduled) {
        let sched_badge = make_badge(
            row![
                icon(icons::ICON_SCHEDULED)
                    .size(10)
                    .color(colors::BACKGROUND),
                text("SCHEDULED")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            colors::PRIMARY,
            None,
        );
        header_left = header_left.push(sched_badge);
    }

    match &item.state {
        DownloadState::FetchingMetadata => {
            let badge = make_badge(
                row![
                    icon(icons::ICON_SPINNER).size(10).color(colors::BACKGROUND),
                    text("FETCHING INFO")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                colors::PRIMARY,
                None,
            );
            header_left = header_left.push(badge);
        }
        DownloadState::Queued => {
            let badge = make_badge(
                row![
                    icon(icons::ICON_LIST_ORDER).size(10).color(colors::WARNING),
                    text("QUEUED")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::WARNING),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                colors::SURFACE_HIGH,
                Some(colors::WARNING),
            );
            header_left = header_left.push(badge);
        }
        DownloadState::Scheduled => {
            let badge = make_badge(
                row![
                    icon(icons::ICON_SCHEDULED)
                        .size(10)
                        .color(colors::BACKGROUND),
                    text("SCHEDULED")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                colors::PRIMARY,
                None,
            );
            header_left = header_left.push(badge);
        }
        DownloadState::Completed => {
            let badge = make_badge(
                text("COMPLETED")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
                colors::SUCCESS,
                None,
            );
            header_left = header_left.push(badge);
        }
        DownloadState::Paused { .. } => {
            let badge = make_badge(
                text("PAUSED")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
                colors::WARNING,
                None,
            );
            header_left = header_left.push(badge);
        }
        DownloadState::WaitingForNetwork { .. } => {
            let badge = make_badge(
                row![
                    icon(icons::ICON_WIFI_SLASH)
                        .size(10)
                        .color(colors::BACKGROUND),
                    text("WAITING FOR NETWORK")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                colors::WARNING,
                None,
            );
            header_left = header_left.push(badge);
        }
        DownloadState::Failed { .. } => {
            let badge = make_badge(
                text("FAILED")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
                colors::ERROR,
                None,
            );
            header_left = header_left.push(badge);
        }
        _ => {}
    }

    // Speed Limited badge
    if let Some(limit_bps) = item.speed_limit_bps {
        if limit_bps > 0 {
            header_left = header_left.push(make_limit_badge(limit_bps));
        }
    }

    let item_id = item.id;

    // Mirrors badge / button
    let mirrors_count = item.http_meta().map(|m| m.mirror_urls.len()).unwrap_or(0);
    let mirrors_label = if mirrors_count == 0 {
        "Mirrors".to_string()
    } else {
        format!("{} Mirrors", mirrors_count)
    };

    let mirrors_btn = button(
        row![
            icon(icons::ICON_LINK).size(12).color(if mirrors_count > 0 {
                colors::PRIMARY
            } else {
                colors::TEXT_MUTED
            }),
            text(mirrors_label).size(11).color(if mirrors_count > 0 {
                colors::TEXT_PRIMARY
            } else {
                colors::TEXT_MUTED
            }),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .style(styles::ghost_button_style)
    .on_press(on_open_mirrors(item_id));

    let info_btn = button(icon(icons::ICON_INFO).size(14))
        .style(styles::icon_button_style)
        .on_press(on_open_details(item_id));

    let actions_row: Element<Message> = match &item.state {
        DownloadState::FetchingMetadata
        | DownloadState::Downloading { .. }
        | DownloadState::WaitingForNetwork { .. }
        | DownloadState::Checking { .. } => {
            let pause_btn = button(icon(icons::ICON_PAUSE).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let cancel_btn = button(icon(icons::ICON_CANCEL).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            let mut r = row![].spacing(10).align_y(Alignment::Center);
            r = r.push(info_btn);
            if item.http_meta().is_some() {
                r = r.push(mirrors_btn);
            }
            r.push(pause_btn).push(cancel_btn).into()
        }
        DownloadState::Completed => {
            let folder_btn = button(icon(icons::ICON_FOLDER).size(14))
                .style(styles::icon_button_style)
                .on_press(on_open_folder(item_id));
            let trash_btn = button(icon(icons::ICON_TRASH).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            let mut r = row![].spacing(10).align_y(Alignment::Center);
            r = r.push(info_btn);
            if item.http_meta().is_some() {
                r = r.push(mirrors_btn);
            }
            r.push(folder_btn).push(trash_btn).into()
        }
        DownloadState::Queued | DownloadState::Paused { .. } | DownloadState::Scheduled => {
            let play_btn = button(icon(icons::ICON_PLAY).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let cancel_btn = button(icon(icons::ICON_CANCEL).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            let mut r = row![].spacing(10).align_y(Alignment::Center);
            r = r.push(info_btn);
            if item.http_meta().is_some() {
                r = r.push(mirrors_btn);
            }
            r.push(play_btn).push(cancel_btn).into()
        }
        DownloadState::Failed { .. } => {
            let retry_btn = button(icon(icons::ICON_RETRY).size(14))
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));
            let trash_btn = button(icon(icons::ICON_TRASH).size(14))
                .style(styles::icon_button_style)
                .on_press(on_cancel(item_id));
            let mut r = row![].spacing(10).align_y(Alignment::Center);
            r = r.push(info_btn);
            if item.http_meta().is_some() {
                r = r.push(mirrors_btn);
            }
            r.push(retry_btn).push(trash_btn).into()
        }
    };

    let top_row = row![
        container(header_left).width(Length::Fill),
        actions_row,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let url_str = item.get_url();
    let truncated_url = if url_str.len() > 60 {
        format!("{}…", &url_str[..60])
    } else {
        url_str.to_string()
    };

    let url_text = text(truncated_url)
        .size(12)
        .font(styles::MONO_FONT)
        .color(colors::TEXT_MUTED);

    let copy_icon_char = if is_copied {
        icons::ICON_CHECK
    } else {
        icons::ICON_COPY
    };
    let copy_icon_color = if is_copied {
        colors::SUCCESS
    } else {
        colors::TEXT_MUTED
    };

    let copy_btn = button(icon(copy_icon_char).size(12).color(copy_icon_color))
        .style(styles::icon_button_style)
        .padding(0)
        .on_press(on_copy_link(item.id));

    let url_row = row![copy_btn, url_text]
        .spacing(6)
        .align_y(Alignment::Center);

    let second_row_col: Element<Message> = if let DownloadState::Failed { error, .. } = &item.state
    {
        let err_row = row![
            icon(icons::ICON_WARN).size(13).color(colors::ERROR),
            text(format!("Error: {}", error))
                .size(12)
                .color(colors::ERROR),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        column![url_row, err_row].spacing(6).into()
    } else {
        url_row.into()
    };

    let progress_val = item.progress();

    let (bar_color, size_detail, speed_eta_col) = match &item.state {
        DownloadState::FetchingMetadata => (
            colors::PRIMARY,
            "Resolving file metadata in background...".to_string(),
            Some(
                column![
                    text("Connecting...")
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::PRIMARY),
                    text("Fetching info").size(11).color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::Queued => (
            colors::PRIMARY,
            item.formatted_size_progress(),
            Some(
                column![
                    text("Queued")
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_PRIMARY),
                    text("Waiting...").size(11).color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::Scheduled => (
            colors::PRIMARY,
            format!("Scheduled · {}", item.formatted_total_size()),
            Some(
                column![
                    text("Scheduled")
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::PRIMARY),
                    text("Waiting for window")
                        .size(11)
                        .color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::Downloading {
            speed_bps,
            eta_secs,
            ..
        } => {
            let bar_c = colors::PRIMARY;

            let size_det = if let Some(torrent) = item.torrent_meta() {
                if torrent.peers_connected > 0 || torrent.seeds_connected > 0 {
                    format!(
                        "{} · {} peers · {} seeds",
                        item.formatted_size_progress(),
                        torrent.peers_connected,
                        torrent.seeds_connected
                    )
                } else {
                    item.formatted_size_progress()
                }
            } else {
                item.formatted_size_progress()
            };

            let speed_col = if let Some(torrent) = item.torrent_meta() {
                column![
                    row![
                        icon(icons::ICON_DOWNLOAD).size(10).color(colors::PRIMARY),
                        text(format_speed(*speed_bps))
                            .size(12)
                            .font(styles::BOLD_FONT)
                            .color(colors::TEXT_PRIMARY),
                        Space::with_width(6),
                        icon(icons::ICON_UPLOAD).size(10).color(colors::PRIMARY), // Used PRIMARY instead of unused TORRENT
                        text(format_speed(torrent.upload_speed_bps))
                            .size(12)
                            .font(styles::BOLD_FONT)
                            .color(colors::TEXT_PRIMARY),
                    ]
                    .spacing(2)
                    .align_y(Alignment::Center),
                    text(format!(
                        "{} left",
                        eta_secs.map(format_eta).unwrap_or_else(|| "--".to_string())
                    ))
                    .size(11)
                    .color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End)
            } else {
                column![
                    text(format_speed(*speed_bps))
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_PRIMARY),
                    text(format!(
                        "{} left",
                        eta_secs.map(format_eta).unwrap_or_else(|| "--".to_string())
                    ))
                    .size(11)
                    .color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End)
            };

            (bar_c, size_det, Some(speed_col))
        }
        DownloadState::Checking { progress_pct, .. } => (
            colors::WARNING,
            item.formatted_size_progress(),
            Some(
                column![
                    text(format!("Verifying {}%", progress_pct))
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::WARNING),
                    text("Checking file integrity")
                        .size(11)
                        .color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::Completed => (
            colors::SUCCESS,
            format!("{}  ·  Completed", item.formatted_total_size()),
            None,
        ),
        DownloadState::Paused { .. } => (
            colors::WARNING,
            item.formatted_size_progress(),
            Some(
                column![
                    text("0.0 KB/s")
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_MUTED),
                    text("--").size(11).color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::WaitingForNetwork { .. } => (
            colors::WARNING,
            format!("Waiting for network · {}", item.formatted_size_progress()),
            Some(
                column![
                    text("Offline")
                        .size(13)
                        .font(styles::BOLD_FONT)
                        .color(colors::WARNING),
                    text("Auto-resuming when online")
                        .size(11)
                        .color(colors::TEXT_MUTED),
                ]
                .align_x(Alignment::End),
            ),
        ),
        DownloadState::Failed { .. } => (
            colors::ERROR,
            format!(
                "Failed at {:.0}% · {}",
                progress_val,
                item.formatted_size_progress()
            ),
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

    let progress_header =
        row![percent_text, Space::with_width(Length::Fill), size_text,].align_y(Alignment::Center);

    let progress_bar_col = column![progress_header, pbar,]
        .spacing(4)
        .width(Length::Fill);

    let bottom_row: Element<Message> = if let Some(speed_col) = speed_eta_col {
        row![progress_bar_col, Space::with_width(24), speed_col,]
            .align_y(Alignment::Center)
            .into()
    } else {
        progress_bar_col.into()
    };

    let card_content = column![
        top_row,
        Space::with_height(4),
        second_row_col,
        Space::with_height(8),
        bottom_row,
    ]
    .spacing(2)
    .width(Length::Fill);

    let card_layout = row![icon_box, card_content,]
        .spacing(16)
        .align_y(Alignment::Start);

    let card_container = container(card_layout)
        .width(Length::Fill)
        .padding([16, 20])
        .style(move |theme| match &item.state {
            DownloadState::Completed => styles::completed_card_style(theme),
            DownloadState::Failed { .. } => styles::failed_card_style(theme),
            _ => styles::card_style(theme),
        });

    let item_id = item.id;
    mouse_area(card_container)
        .on_press(on_item_click(item_id))
        .into()
}
