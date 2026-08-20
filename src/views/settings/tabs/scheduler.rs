use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::models::download::{DownloadItem, DownloadState, FileType};
use crate::models::schedule::{format_12h, OnCompleteAction};
use crate::theme::{colors, styles};
use crate::views::settings::settings::{
    custom_switch, setting_row, ScheduleSubTab, SettingsMessage, SettingsModel,
};

pub fn view<'a>(
    model: &'a SettingsModel,
    downloads: &'a [DownloadItem],
) -> Element<'a, SettingsMessage> {
    let scheduled_items: Vec<&DownloadItem> = downloads
        .iter()
        .filter(|d| d.is_scheduled)
        .collect();

    let scheduled_count = scheduled_items.len();

    // 1. Horizontal Sub-Tab Bar
    let settings_tab_btn = subtab_button(
        "Settings",
        ScheduleSubTab::Settings,
        model.schedule_subtab,
        None,
    );

    let downloads_tab_btn = subtab_button(
        "Scheduled Downloads",
        ScheduleSubTab::ScheduledDownloads,
        model.schedule_subtab,
        Some(scheduled_count),
    );

    let subtab_row = row![settings_tab_btn, downloads_tab_btn]
        .spacing(12)
        .align_y(Alignment::Center);

    let subtab_bar = container(subtab_row)
        .padding(iced::padding::top(16).bottom(12).left(24).right(24))
        .width(Length::Fill);

    let subtab_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let content: Element<'a, SettingsMessage> = match model.schedule_subtab {
        ScheduleSubTab::Settings => view_settings_subtab(model),
        ScheduleSubTab::ScheduledDownloads => view_scheduled_subtab(scheduled_items),
    };

    column![subtab_bar, subtab_divider, content]
        .width(Length::Fill)
        .into()
}

fn subtab_button<'a>(
    label: &'static str,
    tab: ScheduleSubTab,
    active_tab: ScheduleSubTab,
    count: Option<usize>,
) -> Element<'a, SettingsMessage> {
    let is_active = tab == active_tab;

    let text_widget = text(label)
        .size(13)
        .font(if is_active { styles::BOLD_FONT } else { iced::Font::DEFAULT })
        .color(if is_active { colors::PRIMARY } else { colors::TEXT_MUTED });

    let mut content_row = row![text_widget].spacing(6).align_y(Alignment::Center);

    if let Some(c) = count {
        let badge = container(
            text(c.to_string())
                .size(10)
                .font(styles::BOLD_FONT)
                .color(if is_active { colors::BACKGROUND } else { colors::TEXT_MUTED }),
        )
        .padding([2, 6])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(if is_active {
                colors::PRIMARY
            } else {
                colors::SURFACE_HIGH
            })),
            border: iced::Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        });

        content_row = content_row.push(badge);
    }

    button(content_row)
        .padding([6, 12])
        .style(move |_, _| {
            if is_active {
                button::Style {
                    background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                    text_color: colors::PRIMARY,
                    border: iced::Border {
                        color: colors::PRIMARY,
                        width: 1.0,
                        radius: 6.0.into(),
                    },
                    ..Default::default()
                }
            } else {
                button::Style {
                    background: None,
                    text_color: colors::TEXT_MUTED,
                    ..Default::default()
                }
            }
        })
        .on_press(SettingsMessage::ScheduleSubTabSelected(tab))
        .into()
}

fn view_settings_subtab<'a>(model: &'a SettingsModel) -> Element<'a, SettingsMessage> {
    // 1. Master Enable Toggle
    let item_enable = setting_row(
        "Enable download scheduler",
        "Automatically start and pause scheduled downloads based on your schedule",
        custom_switch(
            model.schedule.enabled,
            model.schedule.enabled_anim,
            SettingsMessage::ToggleScheduleEnabled,
        ),
    );

    // 2. Start Time Row
    let start_12h_label = text(format!("({})", format_12h(&model.schedule.start_time)))
        .size(12)
        .font(styles::BOLD_FONT)
        .color(colors::PRIMARY);

    let start_input = text_input("23:00", &model.schedule.start_time)
        .on_input(SettingsMessage::ScheduleStartTimeChanged)
        .padding([8, 12])
        .width(90)
        .style(styles::dark_input_style);

    let start_control = row![start_input, start_12h_label]
        .spacing(10)
        .align_y(Alignment::Center);

    let item_start_time = setting_row(
        "Start download at",
        "Time of day to start scheduled downloads (24-hour HH:MM format)",
        start_control.into(),
    );

    // 3. Stop Time Row
    let stop_switch = custom_switch(
        model.schedule.stop_enabled,
        model.schedule.stop_enabled_anim,
        SettingsMessage::ToggleScheduleStopEnabled,
    );

    let stop_12h_label = text(format!("({})", format_12h(&model.schedule.stop_time)))
        .size(12)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_MUTED);

    let stop_input = text_input("07:00", &model.schedule.stop_time)
        .on_input(SettingsMessage::ScheduleStopTimeChanged)
        .padding([8, 12])
        .width(90)
        .style(styles::dark_input_style);

    let stop_control = row![stop_switch, stop_input, stop_12h_label]
        .spacing(10)
        .align_y(Alignment::Center);

    let item_stop_time = setting_row(
        "Stop download at",
        "Automatically pause scheduled downloads when this time is reached",
        stop_control.into(),
    );

    // 4. Prioritize Scheduled Downloads Switch
    let priority_switch = custom_switch(
        model.schedule.prioritize_scheduled,
        model.schedule.prioritize_scheduled_anim,
        SettingsMessage::TogglePrioritizeScheduled,
    );

    let item_priority = setting_row(
        "Prioritize scheduled downloads",
        "Push scheduled downloads to the front of the download queue when schedule starts",
        priority_switch,
    );

    // 5. Days of the Week Pills
    let day_names = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let mut day_buttons = row![].spacing(6).align_y(Alignment::Center);

    for (idx, name) in day_names.iter().enumerate() {
        let is_active = model.schedule.active_days[idx];
        let day_btn = button(
            text(*name)
                .size(12)
                .font(styles::BOLD_FONT)
                .color(if is_active { colors::BACKGROUND } else { colors::TEXT_MUTED }),
        )
        .padding([6, 12])
        .style(move |_, _| {
            if is_active {
                button::Style {
                    background: Some(iced::Background::Color(colors::PRIMARY)),
                    text_color: colors::BACKGROUND,
                    border: iced::Border { radius: 14.0.into(), ..Default::default() },
                    ..Default::default()
                }
            } else {
                button::Style {
                    background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                    text_color: colors::TEXT_MUTED,
                    border: iced::Border {
                        color: colors::BORDER,
                        width: 1.0,
                        radius: 14.0.into(),
                    },
                    ..Default::default()
                }
            }
        })
        .on_press(SettingsMessage::ScheduleToggleDay(idx));

        day_buttons = day_buttons.push(day_btn);
    }

    let item_days = setting_row(
        "Active days of the week",
        "Days when the scheduler should run automatically",
        day_buttons.into(),
    );

    // 6. On Complete Action
    let on_complete_dropdown = pick_list(
        OnCompleteAction::ALL,
        Some(model.schedule.on_complete_action),
        SettingsMessage::ScheduleOnCompleteChanged,
    )
    .style(styles::pick_list_style)
    .menu_style(styles::pick_list_menu_style)
    .padding([8, 12])
    .width(280);

    let item_on_complete = setting_row(
        "When scheduled downloads complete",
        "Action to perform once all scheduled items finish downloading",
        on_complete_dropdown.into(),
    );

    column![
        item_enable,
        item_start_time,
        item_stop_time,
        item_priority,
        item_days,
        item_on_complete,
    ]
    .spacing(20)
    .padding([20, 24])
    .width(Length::Fill)
    .into()
}

fn view_scheduled_subtab<'a>(items: Vec<&'a DownloadItem>) -> Element<'a, SettingsMessage> {
    if items.is_empty() {
        return container(
            column![
                icon(icons::ICON_SCHEDULED)
                    .size(32)
                    .color(colors::TEXT_MUTED),
                Space::with_height(12),
                text("No downloads in scheduler")
                    .size(16)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
                Space::with_height(4),
                text("Click 'Schedule for Later' when adding a download to manage it here.")
                    .size(13)
                    .color(colors::TEXT_MUTED),
            ]
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(260)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into();
    }

    let total = items.len();
    let mut list_col = column![].spacing(8).width(Length::Fill);

    for (rank, item) in items.into_iter().enumerate() {
        let item_id = item.id;
        let is_first = rank == 0;
        let is_last = rank + 1 == total;

        // Rank Badge
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

        // File icon
        let file_icon = match item.file_type {
            FileType::Archive => icons::ICON_ZIP,
            FileType::Media => icons::ICON_DOWNLOADING,
            FileType::Code => icons::ICON_GRID,
            FileType::Document => icons::ICON_BOX,
            FileType::Other => icons::ICON_DISC,
        };

        // File Info
        let name_label = text(&item.filename)
            .size(14)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY);

        let details_row = row![
            text(item.formatted_total_size()).size(11).color(colors::TEXT_MUTED),
            text("·").size(11).color(colors::TEXT_MUTED),
            text(format!("Added: {}", item.formatted_created_date())).size(11).color(colors::TEXT_MUTED),
        ]
        .spacing(6)
        .align_y(Alignment::Center);

        let info_box = column![name_label, details_row].spacing(2).width(Length::Fill);

        // Status badge
        let status_badge = match &item.state {
            DownloadState::Completed => container(
                text("COMPLETED").size(10).font(styles::BOLD_FONT).color(colors::BACKGROUND),
            )
            .padding([2, 6])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SUCCESS)),
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }),

            DownloadState::Downloading { speed_bps, .. } => container(
                text(format!("RUNNING · {}", crate::models::download::format_speed(*speed_bps)))
                    .size(10)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
            )
            .padding([2, 6])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SUCCESS)),
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }),

            _ => container(
                text("SCHEDULED").size(10).font(styles::BOLD_FONT).color(colors::BACKGROUND),
            )
            .padding([2, 6])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::PRIMARY)),
                border: iced::Border { radius: 6.0.into(), ..Default::default() },
                ..Default::default()
            }),
        };

        // Action Buttons: Up, Down, Remove
        let mut up_btn = button(icon(icons::ICON_ARROW_UP).size(13))
            .padding([6, 8])
            .style(styles::icon_button_style);
        if !is_first {
            up_btn = up_btn.on_press(SettingsMessage::MoveScheduledItemUp(item_id));
        }

        let mut down_btn = button(icon(icons::ICON_ARROW_DOWN).size(13))
            .padding([6, 8])
            .style(styles::icon_button_style);
        if !is_last {
            down_btn = down_btn.on_press(SettingsMessage::MoveScheduledItemDown(item_id));
        }

        let remove_btn = button(
            icon(icons::ICON_XMARK).size(13).color(colors::ERROR),
        )
        .padding([6, 8])
        .style(styles::icon_button_style)
        .on_press(SettingsMessage::RemoveFromSchedule(item_id));

        let actions = row![up_btn, down_btn, remove_btn]
            .spacing(4)
            .align_y(Alignment::Center);

        let row_card = container(
            row![
                rank_badge,
                icon(file_icon).size(16).color(colors::PRIMARY),
                info_box,
                status_badge,
                Space::with_width(12),
                actions,
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([10, 16])
        .width(Length::Fill)
        .style(styles::card_style);

        list_col = list_col.push(row_card);
    }

    scrollable(container(list_col).padding([16, 24]))
        .width(Length::Fill)
        .into()
}
