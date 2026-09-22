use iced::widget::{button, column, container, progress_bar, row, scrollable, text, Space};
use iced::{Alignment, Element, Length, Task};

use crate::icons::{self, icon};
use crate::models::download::{
    format_bytes, format_speed, DownloadItem, DownloadState, DownloadType, FileType,
};
use crate::theme::{colors, styles};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailDialogTab {
    Overview,
    Connections,
}

#[derive(Debug, Clone)]
pub struct DetailDialogModel {
    pub is_open: bool,
    pub download_id: usize,
    pub current_tab: DetailDialogTab,
    pub copy_feedback: Option<String>,
}

impl Default for DetailDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            download_id: 0,
            current_tab: DetailDialogTab::Overview,
            copy_feedback: None,
        }
    }
}

impl DetailDialogModel {
    pub fn open(&mut self, download_id: usize) {
        self.is_open = true;
        self.download_id = download_id;
        self.current_tab = DetailDialogTab::Overview;
        self.copy_feedback = None;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.copy_feedback = None;
    }
}

#[derive(Debug, Clone)]
pub enum DetailDialogueMessage {
    CloseDetailDialog,
    SelectTab(DetailDialogTab),
    CopyText(String, String),
    OpenFolder(usize),
}

#[allow(dead_code)]
pub fn update(
    state: &mut DetailDialogModel,
    message: DetailDialogueMessage,
) -> Task<DetailDialogueMessage> {
    match message {
        DetailDialogueMessage::CloseDetailDialog => {
            state.close();
            Task::none()
        }
        DetailDialogueMessage::SelectTab(tab) => {
            state.current_tab = tab;
            Task::none()
        }
        DetailDialogueMessage::CopyText(content, feedback) => {
            state.copy_feedback = Some(feedback);
            iced::clipboard::write(content)
        }
        DetailDialogueMessage::OpenFolder(_) => {
            // Handled at app level
            Task::none()
        }
    }
}

pub fn view<'a>(
    state: &'a DetailDialogModel,
    item: &'a DownloadItem,
) -> Element<'a, DetailDialogueMessage> {
    // --- Header ---
    let title_row = row![
        icon(icons::ICON_INFO).size(18).color(colors::PRIMARY),
        text("Download Details")
            .size(18)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY),
        Space::with_width(Length::Fill),
        button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
            .style(styles::icon_button_style)
            .on_press(DetailDialogueMessage::CloseDetailDialog)
    ]
    .padding([14, 20])
    .align_y(Alignment::Center);

    let header_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    // --- File Banner ---
    let (icon_char, icon_color) = if item.is_folder() {
        (icons::ICON_FOLDER, colors::WARNING)
    } else {
        match item.file_type {
            FileType::Media => (icons::ICON_MEDIA, colors::PRIMARY),
            FileType::Archive => (icons::ICON_ZIP, colors::SUCCESS),
            FileType::Code => (icons::ICON_CODE, colors::TORRENT),
            FileType::Document => (icons::ICON_DOCUMENT, colors::PRIMARY),
            FileType::Other => {
                if item.torrent_meta().is_some() {
                    (icons::ICON_MAGNET, colors::TORRENT)
                } else {
                    (icons::ICON_FILE, colors::TEXT_MUTED)
                }
            }
        }
    };

    let file_icon_box = container(icon(icon_char).size(24).color(icon_color))
        .width(48)
        .height(48)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let (status_label, status_color) = match &item.state {
        DownloadState::FetchingMetadata => ("Fetching Info".to_string(), colors::PRIMARY),
        DownloadState::Queued => ("Queued".to_string(), colors::WARNING),
        DownloadState::Scheduled => ("Scheduled".to_string(), colors::PRIMARY),
        DownloadState::Downloading { .. } => ("Downloading".to_string(), colors::PRIMARY),
        DownloadState::Checking { progress_pct, .. } => {
            (format!("Checking ({}%)", progress_pct), colors::PRIMARY)
        }
        DownloadState::Completed => ("Completed".to_string(), colors::SUCCESS),
        DownloadState::Paused { .. } => ("Paused".to_string(), colors::WARNING),
        DownloadState::WaitingForNetwork { .. } => ("Waiting for Network".to_string(), colors::WARNING),
        DownloadState::Failed { .. } => ("Failed".to_string(), colors::ERROR),
    };

    let status_badge = container(
        text(status_label)
            .size(11)
            .font(styles::BOLD_FONT)
            .color(colors::BACKGROUND),
    )
    .padding([3, 8])
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(status_color)),
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let download_type_badge = match &item.download_type {
        DownloadType::Http(_) => container(
            text("HTTP Multi-Stream")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::PRIMARY),
        )
        .padding([2, 6])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                color: colors::PRIMARY,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }),
        DownloadType::Torrent(_) => container(
            text("BitTorrent P2P")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::TORRENT),
        )
        .padding([2, 6])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                color: colors::TORRENT,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        }),
    };

    let file_info_col = column![
        text(&item.filename)
            .size(14)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY),
        row![
            text(format_bytes(item.total_bytes.unwrap_or(item.downloaded_bytes)))
                .size(11)
                .color(colors::TEXT_MUTED),
            text("•").size(11).color(colors::TEXT_MUTED),
            download_type_badge,
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(4);

    let banner_row = row![
        file_icon_box,
        container(file_info_col).width(Length::Fill),
        status_badge,
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .padding([12, 20]);

    // --- Live Progress & Speeds ---
    let progress_pct = item.progress();
    let pbar = progress_bar(0.0..=100.0, progress_pct)
        .height(Length::Fixed(6.0))
        .style(styles::progress_bar_style_with_color(status_color));

    let (speed_text, eta_text) = match &item.state {
        DownloadState::Downloading {
            speed_bps,
            eta_secs,
            ..
        } => {
            let spd = format!("{} /s", format_speed(*speed_bps));
            let eta = match eta_secs {
                Some(secs) if *secs < 60 => format!("{}s", secs),
                Some(secs) if *secs < 3600 => format!("{}m {}s", secs / 60, secs % 60),
                Some(secs) => format!("{}h {}m", secs / 3600, (secs % 3600) / 60),
                None => "--".to_string(),
            };
            (spd, eta)
        }
        DownloadState::Completed => ("Done".to_string(), "Finished".to_string()),
        DownloadState::Paused { .. } => ("Paused".to_string(), "--".to_string()),
        DownloadState::WaitingForNetwork { .. } => ("Reconnecting...".to_string(), "--".to_string()),
        DownloadState::Failed { .. } => ("Error".to_string(), "--".to_string()),
        _ => ("--".to_string(), "--".to_string()),
    };

    let mut metrics_row = row![
        text(format!(
            "{:.1}% ({} / {})",
            progress_pct,
            format_bytes(item.downloaded_bytes),
            item.total_bytes.map(format_bytes).unwrap_or_else(|| "Unknown".to_string())
        ))
        .size(11)
        .font(styles::MONO_FONT)
        .color(colors::TEXT_PRIMARY),
        Space::with_width(Length::Fill),
        icon(icons::ICON_DOWNLOAD).size(11).color(colors::PRIMARY),
        text(speed_text).size(11).font(styles::MONO_FONT).color(colors::TEXT_PRIMARY),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    if let Some(torrent) = item.torrent_meta() {
        if torrent.upload_speed_bps > 0 {
            metrics_row = metrics_row
                .push(Space::with_width(8))
                .push(icon(icons::ICON_UPLOAD).size(11).color(colors::TORRENT))
                .push(
                    text(format!("{} /s", format_speed(torrent.upload_speed_bps)))
                        .size(11)
                        .font(styles::MONO_FONT)
                        .color(colors::TEXT_PRIMARY),
                );
        }
    }

    metrics_row = metrics_row
        .push(Space::with_width(8))
        .push(icon(icons::ICON_CLOCK).size(11).color(colors::TEXT_MUTED))
        .push(text(eta_text).size(11).font(styles::MONO_FONT).color(colors::TEXT_MUTED));

    let progress_box = container(column![pbar, metrics_row].spacing(8))
        .padding([6, 20])
        .width(Length::Fill);

    // --- Tab Switcher ---
    let tab_streams_label = match &item.download_type {
        DownloadType::Http(h) => format!("Streams & Connections ({})", h.chunks.len()),
        DownloadType::Torrent(_) => "Swarm & Diagnostics".to_string(),
    };

    let tab_overview_btn = button(
        text("Overview & Metadata")
            .size(12)
            .font(styles::BOLD_FONT)
            .color(if state.current_tab == DetailDialogTab::Overview {
                colors::PRIMARY
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding([6, 14])
    .style(if state.current_tab == DetailDialogTab::Overview {
        styles::active_nav_button_style
    } else {
        styles::ghost_button_style
    })
    .on_press(DetailDialogueMessage::SelectTab(DetailDialogTab::Overview));

    let tab_streams_btn = button(
        text(tab_streams_label)
            .size(12)
            .font(styles::BOLD_FONT)
            .color(if state.current_tab == DetailDialogTab::Connections {
                colors::PRIMARY
            } else {
                colors::TEXT_MUTED
            }),
    )
    .padding([6, 14])
    .style(if state.current_tab == DetailDialogTab::Connections {
        styles::active_nav_button_style
    } else {
        styles::ghost_button_style
    })
    .on_press(DetailDialogueMessage::SelectTab(DetailDialogTab::Connections));

    let tab_bar = container(
        row![tab_overview_btn, tab_streams_btn]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([4, 20])
    .width(Length::Fill);

    let tab_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    // --- Tab Body Content ---
    let body_content: Element<'a, DetailDialogueMessage> = match state.current_tab {
        DetailDialogTab::Overview => render_overview_tab(item),
        DetailDialogTab::Connections => match &item.download_type {
            DownloadType::Http(http) => render_http_connections_tab(item, http),
            DownloadType::Torrent(torrent) => render_torrent_swarm_tab(item, torrent),
        },
    };

    let scrollable_body = scrollable(
        container(body_content)
            .width(Length::Fill)
            .padding([14, 20]),
    )
    .height(Length::Fixed(270.0))
    .style(styles::scrollable_style);

    // --- Footer ---
    let footer_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let copy_feedback_el: Element<'a, DetailDialogueMessage> = if let Some(fb) = &state.copy_feedback {
        row![
            icon(icons::ICON_CHECK).size(12).color(colors::SUCCESS),
            text(fb).size(12).color(colors::SUCCESS),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        Space::with_width(1).into()
    };

    let open_folder_btn = button(
        row![
            icon(icons::ICON_FOLDER).size(13),
            text("Open Folder").size(13).font(styles::BOLD_FONT),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(styles::ghost_button_style)
    .on_press(DetailDialogueMessage::OpenFolder(item.id));

    let done_btn = button(
        text("Done")
            .size(13)
            .font(styles::BOLD_FONT)
            .color(colors::BACKGROUND),
    )
    .padding([8, 22])
    .style(styles::primary_button_style)
    .on_press(DetailDialogueMessage::CloseDetailDialog);

    let footer_row = row![
        copy_feedback_el,
        Space::with_width(Length::Fill),
        open_folder_btn,
        done_btn,
    ]
    .spacing(10)
    .padding([12, 20])
    .align_y(Alignment::Center);

    // --- Modal Assembly ---
    let modal_card = container(column![
        title_row,
        header_divider,
        banner_row,
        progress_box,
        tab_bar,
        tab_divider,
        scrollable_body,
        footer_divider,
        footer_row,
    ])
    .width(680)
    .style(styles::card_style);

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.68,
            ))),
            ..Default::default()
        })
        .into()
}

fn render_overview_tab<'a>(item: &'a DownloadItem) -> Element<'a, DetailDialogueMessage> {
    let url_str = item.get_url().to_string();
    let url_copy_target = url_str.clone();

    // Source Card
    let source_box = container(
        column![
            row![
                text("DOWNLOAD SOURCE")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::PRIMARY),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_COPY).size(11).color(colors::TEXT_MUTED),
                        text("Copy URL").size(11).color(colors::TEXT_MUTED),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .style(styles::ghost_button_style)
                .padding([2, 6])
                .on_press(DetailDialogueMessage::CopyText(
                    url_copy_target,
                    "✓ Download URL copied!".to_string()
                )),
            ]
            .align_y(Alignment::Center),
            text(url_str)
                .size(11)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(6),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(card_section_style);

    // Save Location Card
    let save_path_str = item.save_path.clone();
    let save_path_copy = save_path_str.clone();
    let location_box = container(
        column![
            row![
                text("SAVE DESTINATION")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::PRIMARY),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_COPY).size(11).color(colors::TEXT_MUTED),
                        text("Copy Path").size(11).color(colors::TEXT_MUTED),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .style(styles::ghost_button_style)
                .padding([2, 6])
                .on_press(DetailDialogueMessage::CopyText(
                    save_path_copy,
                    "✓ Save path copied!".to_string()
                )),
            ]
            .align_y(Alignment::Center),
            text(save_path_str)
                .size(11)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(6),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(card_section_style);

    // Specs Grid
    let connections_val = format!("{} Max Streams", item.max_connections);
    let speed_limit_val = if let Some(limit) = item.speed_limit_bps {
        format_speed(limit)
    } else {
        "Unlimited".to_string()
    };
    let file_category_val = match item.file_type {
        FileType::Media => "Media (Audio / Video / Images)",
        FileType::Archive => "Archive / Compressed Disk Image",
        FileType::Code => "Executable / Code / Script",
        FileType::Document => "Document / PDF / Office",
        FileType::Other => "General File",
    };

    let (resumable_val, etag_val, last_mod_val) = if let Some(http) = item.http_meta() {
        let res = if http.resumable { "Yes (Byte Ranges)" } else { "No (Single stream)" };
        let et = http.etag.as_deref().unwrap_or("None");
        let lm = http.last_modified.as_deref().unwrap_or("None");
        (res, et, lm)
    } else {
        ("Yes (P2P Piece Verification)", "N/A", "N/A")
    };

    let added_date_val = item.formatted_created_date();
    let completed_date_val = if let Some(ts) = item.completed_at {
        if let Some(dt) = chrono::DateTime::from_timestamp(ts as i64, 0) {
            let local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
            local.format("%b %d, %Y %I:%M %p").to_string()
        } else {
            "Completed".to_string()
        }
    } else {
        "In progress".to_string()
    };

    let specs_col = column![
        make_spec_row("Connections:", connections_val, "Resumable:", resumable_val),
        make_spec_row("Speed Limit:", speed_limit_val, "Category:", file_category_val),
        make_spec_row("ETag:", etag_val, "Last-Modified:", last_mod_val),
        make_spec_row("Date Added:", added_date_val, "Completed At:", completed_date_val),
    ]
    .spacing(8);

    let specs_box = container(
        column![
            text("TRANSFER & FILE SPECIFICATIONS")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::PRIMARY),
            specs_col,
        ]
        .spacing(10),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(card_section_style);

    // Integrity Card (SHA256 if available)
    let hash_col: Element<'a, DetailDialogueMessage> = if let Some(hash) = &item.sha256_hash {
        let hash_copy = hash.clone();
        container(
            column![
                row![
                    text("SHA-256 INTEGRITY DIGEST")
                        .size(10)
                        .font(styles::BOLD_FONT)
                        .color(colors::SUCCESS),
                    Space::with_width(Length::Fill),
                    button(
                        row![
                            icon(icons::ICON_COPY).size(11).color(colors::TEXT_MUTED),
                            text("Copy Hash").size(11).color(colors::TEXT_MUTED),
                        ]
                        .spacing(4)
                        .align_y(Alignment::Center),
                    )
                    .style(styles::ghost_button_style)
                    .padding([2, 6])
                    .on_press(DetailDialogueMessage::CopyText(
                        hash_copy,
                        "✓ SHA-256 hash copied!".to_string()
                    )),
                ]
                .align_y(Alignment::Center),
                text(hash).size(11).font(styles::MONO_FONT).color(colors::SUCCESS),
            ]
            .spacing(6),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(card_section_style)
        .into()
    } else {
        Space::with_height(0).into()
    };

    column![source_box, location_box, specs_box, hash_col]
        .spacing(10)
        .width(Length::Fill)
        .into()
}

fn render_http_connections_tab<'a>(
    item: &'a DownloadItem,
    http: &'a crate::models::download::HttpMetadata,
) -> Element<'a, DetailDialogueMessage> {
    let total_chunks = http.chunks.len();
    let completed_chunks = http.chunks.iter().filter(|c| c.is_completed).count();
    let active_chunks = total_chunks.saturating_sub(completed_chunks);

    let summary_box = container(
        row![
            column![
                text("MAX CONCURRENT LIMIT")
                    .size(9)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_MUTED),
                text(format!("{} Streams", item.max_connections))
                    .size(13)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            column![
                text("ACTIVE STREAMS")
                    .size(9)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_MUTED),
                text(format!("{} Streaming", active_chunks))
                    .size(13)
                    .font(styles::BOLD_FONT)
                    .color(if active_chunks > 0 { colors::PRIMARY } else { colors::TEXT_MUTED }),
            ]
            .spacing(2),
            Space::with_width(Length::Fill),
            column![
                text("COMPLETED CHUNKS")
                    .size(9)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_MUTED),
                text(format!("{} Finished", completed_chunks))
                    .size(13)
                    .font(styles::BOLD_FONT)
                    .color(colors::SUCCESS),
            ]
            .spacing(2),
        ]
        .align_y(Alignment::Center),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(card_section_style);

    if http.chunks.is_empty() {
        let empty_msg = container(
            column![
                icon(icons::ICON_MICROCHIP).size(26).color(colors::TEXT_MUTED),
                Space::with_height(6),
                text("No active chunk streams allocated yet.")
                    .size(12)
                    .color(colors::TEXT_MUTED),
                text("Multi-stream worker connections will appear here once active transfer begins.")
                    .size(11)
                    .color(colors::TEXT_MUTED),
            ]
            .align_x(Alignment::Center),
        )
        .padding([24, 16])
        .width(Length::Fill)
        .align_x(Alignment::Center);

        return column![summary_box, empty_msg].spacing(10).into();
    }

    let mut chunk_list = column![].spacing(8).width(Length::Fill);

    for (idx, chunk) in http.chunks.iter().enumerate() {
        let chunk_total = chunk.total_chunk_bytes();
        let chunk_done = chunk.downloaded_bytes();
        let chunk_pct = if chunk_total > 0 {
            ((chunk_done as f64 / chunk_total as f64) * 100.0).min(100.0) as f32
        } else if chunk.is_completed {
            100.0
        } else {
            0.0
        };

        let (chunk_badge_label, chunk_badge_color) = if chunk.is_completed {
            ("COMPLETED", colors::SUCCESS)
        } else if matches!(item.state, DownloadState::Downloading { .. }) {
            ("STREAMING", colors::PRIMARY)
        } else {
            ("PAUSED", colors::WARNING)
        };

        let chunk_badge = container(
            text(chunk_badge_label)
                .size(9)
                .font(styles::BOLD_FONT)
                .color(colors::BACKGROUND),
        )
        .padding([2, 5])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(chunk_badge_color)),
            border: iced::Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

        let chunk_pbar = progress_bar(0.0..=100.0, chunk_pct)
            .height(Length::Fixed(4.0))
            .style(styles::progress_bar_style_with_color(chunk_badge_color));

        let source_display = if chunk.url.is_empty() {
            "Primary Server".to_string()
        } else if chunk.url == http.primary_url.url {
            "Primary Server".to_string()
        } else {
            format!("Mirror: {}", truncate_str(&chunk.url, 45))
        };

        let range_display = if chunk.end_byte == u64::MAX {
            format!("Offset: {} .. End", format_bytes(chunk.start_byte))
        } else {
            format!(
                "{} - {} (Size: {})",
                format_bytes(chunk.start_byte),
                format_bytes(chunk.end_byte),
                format_bytes(chunk_total)
            )
        };

        let chunk_card = container(
            column![
                row![
                    icon(icons::ICON_MICROCHIP).size(11).color(colors::PRIMARY),
                    text(format!("Thread #{}", idx + 1))
                        .size(12)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_PRIMARY),
                    Space::with_width(6),
                    chunk_badge,
                    Space::with_width(Length::Fill),
                    text(format!("{:.1}%", chunk_pct))
                        .size(11)
                        .font(styles::MONO_FONT)
                        .color(colors::TEXT_PRIMARY),
                ]
                .align_y(Alignment::Center),
                row![
                    text(source_display).size(10).color(colors::TEXT_MUTED),
                    Space::with_width(Length::Fill),
                    text(format!("{} / {}", format_bytes(chunk_done), format_bytes(chunk_total)))
                        .size(10)
                        .font(styles::MONO_FONT)
                        .color(colors::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
                text(range_display)
                    .size(10)
                    .font(styles::MONO_FONT)
                    .color(colors::TEXT_MUTED),
                chunk_pbar,
            ]
            .spacing(5),
        )
        .padding([8, 12])
        .width(Length::Fill)
        .style(card_section_style);

        chunk_list = chunk_list.push(chunk_card);
    }

    column![summary_box, chunk_list]
        .spacing(10)
        .width(Length::Fill)
        .into()
}

fn render_torrent_swarm_tab<'a>(
    _item: &'a DownloadItem,
    torrent: &'a crate::models::download::TorrentMetadata,
) -> Element<'a, DetailDialogueMessage> {
    // Swarm stats metrics
    let peers_box = container(
        column![
            row![
                icon(icons::ICON_USERS).size(12).color(colors::PRIMARY),
                text("CONNECTED PEERS").size(9).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            text(format!("{}", torrent.peers_connected))
                .size(16)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(4),
    )
    .padding([10, 14])
    .width(Length::FillPortion(1))
    .style(card_section_style);

    let seeds_box = container(
        column![
            row![
                icon(icons::ICON_SERVER).size(12).color(colors::SUCCESS),
                text("SEEDS SEEN").size(9).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            text(format!("{}", torrent.seeds_connected))
                .size(16)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(4),
    )
    .padding([10, 14])
    .width(Length::FillPortion(1))
    .style(card_section_style);

    let upload_box = container(
        column![
            row![
                icon(icons::ICON_UPLOAD).size(12).color(colors::TORRENT),
                text("UPLOAD SPEED").size(9).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            text(format!("{} /s", format_speed(torrent.upload_speed_bps)))
                .size(14)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(4),
    )
    .padding([10, 14])
    .width(Length::FillPortion(1))
    .style(card_section_style);

    let swarm_row = row![peers_box, seeds_box, upload_box]
        .spacing(10)
        .width(Length::Fill);

    // Info Hash Box
    let info_hash_str = torrent.info_hash.as_deref().unwrap_or("Unavailable");
    let info_hash_copy = info_hash_str.to_string();

    let info_hash_card = container(
        column![
            row![
                text("TORRENT INFO HASH (SHA-1)")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::TORRENT),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_COPY).size(11).color(colors::TEXT_MUTED),
                        text("Copy Hash").size(11).color(colors::TEXT_MUTED),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .style(styles::ghost_button_style)
                .padding([2, 6])
                .on_press(DetailDialogueMessage::CopyText(
                    info_hash_copy,
                    "✓ Info Hash copied!".to_string()
                )),
            ]
            .align_y(Alignment::Center),
            text(info_hash_str)
                .size(11)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(6),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(card_section_style);

    // Magnet URI Box
    let magnet_copy = torrent.magnet_uri.clone();
    let magnet_card = container(
        column![
            row![
                text("MAGNET URI")
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::TORRENT),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_COPY).size(11).color(colors::TEXT_MUTED),
                        text("Copy Magnet").size(11).color(colors::TEXT_MUTED),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .style(styles::ghost_button_style)
                .padding([2, 6])
                .on_press(DetailDialogueMessage::CopyText(
                    magnet_copy,
                    "✓ Magnet URI copied!".to_string()
                )),
            ]
            .align_y(Alignment::Center),
            text(&torrent.magnet_uri)
                .size(10)
                .font(styles::MONO_FONT)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(6),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .style(card_section_style);

    // Structure Card
    let struct_val = if torrent.is_folder {
        "Multi-File Folder Torrent"
    } else {
        "Single File Torrent"
    };
    let struct_card = container(
        row![
            text("Payload Structure:")
                .size(11)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_MUTED),
            text(struct_val)
                .size(11)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .width(Length::Fill)
    .style(card_section_style);

    column![swarm_row, info_hash_card, magnet_card, struct_card]
        .spacing(10)
        .width(Length::Fill)
        .into()
}

fn make_spec_row<'a>(
    k1: &'static str,
    v1: impl Into<String>,
    k2: &'static str,
    v2: impl Into<String>,
) -> Element<'a, DetailDialogueMessage> {
    row![
        row![
            text(k1).size(11).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
            text(v1.into()).size(11).color(colors::TEXT_PRIMARY),
        ]
        .spacing(6)
        .width(Length::FillPortion(1))
        .align_y(Alignment::Center),
        row![
            text(k2).size(11).font(styles::BOLD_FONT).color(colors::TEXT_MUTED),
            text(v2.into()).size(11).color(colors::TEXT_PRIMARY),
        ]
        .spacing(6)
        .width(Length::FillPortion(1))
        .align_y(Alignment::Center),
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn card_section_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(iced::Background::Color(colors::SURFACE)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detail_dialog_model_lifecycle() {
        let mut model = DetailDialogModel::default();
        assert!(!model.is_open);
        assert_eq!(model.download_id, 0);
        assert_eq!(model.current_tab, DetailDialogTab::Overview);

        model.open(42);
        assert!(model.is_open);
        assert_eq!(model.download_id, 42);
        assert_eq!(model.current_tab, DetailDialogTab::Overview);

        model.current_tab = DetailDialogTab::Connections;
        model.copy_feedback = Some("✓ Copied!".to_string());
        assert_eq!(model.current_tab, DetailDialogTab::Connections);

        model.close();
        assert!(!model.is_open);
        assert!(model.copy_feedback.is_none());
    }

    #[test]
    fn test_truncate_str() {
        assert_eq!(truncate_str("hello", 10), "hello");
        assert_eq!(truncate_str("hello world", 5), "hello…");
    }
}
