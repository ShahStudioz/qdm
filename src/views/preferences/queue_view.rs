use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::theme::{colors, styles};

pub fn queue_view<'a, Message>(
    downloads: impl IntoIterator<Item = &'a DownloadItem>,
    on_move_up: impl Fn(usize) -> Message + 'a + Clone,
    on_move_down: impl Fn(usize) -> Message + 'a + Clone,
    on_toggle_pause: impl Fn(usize) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // Header
    let header_title = text("Downloads Queue Manager")
        .size(20)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let header_desc = text(
        "Manage the priority and execution order of your downloads. Downloads are processed from top to bottom up to your simultaneous download limit.",
    )
    .size(13)
    .color(colors::TEXT_MUTED);

    let header_box = column![header_title, Space::with_height(4), header_desc].padding([16, 24]);

    let divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    // Filter active/queued/paused items
    let queue_items: Vec<&DownloadItem> = downloads
        .into_iter()
        .filter(|d| !matches!(d.state, DownloadState::Completed))
        .collect();

    let total_items = queue_items.len();

    let content: Element<'a, Message> = if total_items == 0 {
        container(
            column![
                icon(icons::ICON_LIST_ORDER)
                    .size(32)
                    .color(colors::TEXT_MUTED),
                Space::with_height(12),
                text("No active or queued downloads")
                    .size(16)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
                Space::with_height(4),
                text("When downloads are added or paused, they will appear here in queue order.")
                    .size(13)
                    .color(colors::TEXT_MUTED),
            ]
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
        let mut list_col = column![].spacing(10).width(Length::Fill);

        for (rank, item) in queue_items.into_iter().enumerate() {
            let item_id = item.id;
            let is_first = rank == 0;
            let is_last = rank + 1 == total_items;

            // 1. Rank badge
            let rank_badge = container(
                text(format!("#{}", rank + 1))
                    .size(12)
                    .font(styles::BOLD_FONT)
                    .color(colors::PRIMARY),
            )
            .padding([4, 8])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            // 2. File type icon
            let file_icon = match item.file_type {
                FileType::Archive => icons::ICON_ZIP,
                FileType::Media => icons::ICON_DOWNLOADING,
                FileType::Code => icons::ICON_GRID,
                FileType::Document => icons::ICON_BOX,
                FileType::Other => icons::ICON_DISC,
            };

            let type_icon_widget = icon(file_icon).size(16).color(colors::PRIMARY);

            // 3. Name & Sub-details
            let name_text = text(&item.filename)
                .size(14)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY);

            let mut sub_row = row![
                text(item.formatted_size_progress())
                    .size(11)
                    .color(colors::TEXT_MUTED),
                text("·").size(11).color(colors::TEXT_MUTED),
                text(if item.resumable { "Resumable" } else { "Non-resumable" })
                    .size(11)
                    .color(if item.resumable { colors::TEXT_MUTED } else { colors::WARNING }),
            ]
            .spacing(6)
            .align_y(Alignment::Center);

            if item.is_scheduled {
                let sched_badge = container(
                    row![
                        icon(icons::ICON_SCHEDULED).size(9).color(colors::BACKGROUND),
                        text("SCHEDULED").size(9).font(styles::BOLD_FONT).color(colors::BACKGROUND),
                    ]
                    .spacing(3)
                    .align_y(Alignment::Center),
                )
                .padding([1, 6])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::PRIMARY)),
                    border: iced::Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                });

                sub_row = sub_row.push(sched_badge);
            }

            let info_col = column![name_text, sub_row].spacing(3).width(Length::Fill);

            // 4. Status badge
            let status_badge: Element<'a, Message> = match &item.state {
                DownloadState::Downloading { speed_bps, .. } => container(
                    row![
                        icon(icons::ICON_DOWNLOADING).size(11).color(colors::BACKGROUND),
                        text(format!("Downloading · {}", crate::models::download::format_speed(*speed_bps)))
                            .size(11)
                            .font(styles::BOLD_FONT)
                            .color(colors::BACKGROUND),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .padding([3, 8])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::SUCCESS)),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),

                DownloadState::Queued => container(
                    text("QUEUED")
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                )
                .padding([3, 8])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::PRIMARY)),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),

                DownloadState::Paused { .. } => container(
                    text("PAUSED")
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                )
                .padding([3, 8])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::WARNING)),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),

                DownloadState::WaitingForNetwork { .. } => container(
                    text("OFFLINE")
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                )
                .padding([3, 8])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::WARNING)),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),

                DownloadState::Scheduled => container(
                    text("SCHEDULED")
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                )
                .padding([3, 8])
                .style(|_| container::Style {
                    background: Some(iced::Background::Color(colors::PRIMARY)),
                    border: iced::Border { radius: 8.0.into(), ..Default::default() },
                    ..Default::default()
                })
                .into(),

                _ => Space::with_width(0).into(),
            };

            // 5. Reorder buttons
            let mut up_btn = button(icon(icons::ICON_ARROW_UP).size(13))
                .padding([6, 8])
                .style(styles::icon_button_style);
            if !is_first {
                up_btn = up_btn.on_press(on_move_up(item_id));
            }

            let mut down_btn = button(icon(icons::ICON_ARROW_DOWN).size(13))
                .padding([6, 8])
                .style(styles::icon_button_style);
            if !is_last {
                down_btn = down_btn.on_press(on_move_down(item_id));
            }

            // Quick Play/Pause button
            let play_pause_icon = if matches!(item.state, DownloadState::Downloading { .. }) {
                icons::ICON_PAUSE
            } else {
                icons::ICON_PLAY
            };
            let play_btn = button(icon(play_pause_icon).size(13))
                .padding([6, 8])
                .style(styles::icon_button_style)
                .on_press(on_toggle_pause(item_id));

            let actions = row![play_btn, up_btn, down_btn]
                .spacing(4)
                .align_y(Alignment::Center);

            let row_card = container(
                row![
                    rank_badge,
                    type_icon_widget,
                    info_col,
                    status_badge,
                    Space::with_width(12),
                    actions,
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(styles::card_style);

            list_col = list_col.push(row_card);
        }

        scrollable(container(list_col).padding([16, 24]))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    };

    column![header_box, divider, content]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
