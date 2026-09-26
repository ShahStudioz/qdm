use crate::icons::{self, icon};
use crate::models::download::format_bytes;
use crate::services::updater::UpdateStatus;
use crate::theme::{colors, styles};
use crate::views::settings::settings::{
    custom_switch, setting_row, SettingsMessage, SettingsModel,
};
use iced::widget::{button, column, container, progress_bar, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    model: &'a SettingsModel,
    status: &'a UpdateStatus,
) -> Element<'a, SettingsMessage> {
    // 1. Automatic Update Toggle Row
    let item_updates_toggle = setting_row(
        "Check for updates automatically",
        "Periodically check for new releases and security patches",
        custom_switch(
            model.auto_check_updates,
            model.auto_check_updates_anim,
            SettingsMessage::ToggleUpdates,
        ),
    );

    // 2. Version Row with Action Button
    let check_btn = match status {
        UpdateStatus::Checking => button(
            row![
                icon(icons::ICON_SPINNER).size(13).color(colors::TEXT_MUTED),
                text("Checking...").size(13).color(colors::TEXT_MUTED),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 16])
        .style(styles::ghost_button_style),
        _ => button(
            row![
                icon(icons::ICON_RETRY).size(13).color(colors::TEXT_PRIMARY),
                text("Check for Updates")
                    .size(13)
                    .color(colors::TEXT_PRIMARY),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .padding([8, 16])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::CheckUpdatesPressed),
    };

    let item_version = setting_row(
        "Current version",
        crate::core::version::APP_VERSION_BUILD,
        check_btn.into(),
    );

    // 3. Status Section / Update Details Card
    let mut content = column![item_updates_toggle, item_version].spacing(16);

    match status {
        UpdateStatus::Idle => {
            // Nothing extra shown
        }
        UpdateStatus::Checking => {
            let checking_banner = container(
                row![
                    icon(icons::ICON_SPINNER).size(15).color(colors::PRIMARY),
                    text("Contacting update server for latest release notes...")
                        .size(13)
                        .color(colors::TEXT_MUTED),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(checking_banner);
        }
        UpdateStatus::UpToDate { latest_version, .. } => {
            let up_to_date_banner = container(
                row![
                    icon(icons::ICON_CHECK).size(16).color(colors::SUCCESS),
                    column![
                        text("QDM is up to date")
                            .size(13)
                            .font(styles::BOLD_FONT)
                            .color(colors::SUCCESS),
                        text(format!(
                            "Version {} is currently the newest available build.",
                            latest_version
                        ))
                        .size(12)
                        .color(colors::TEXT_MUTED),
                    ]
                    .spacing(2),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.06, 0.72, 0.50, 0.08,
                ))),
                border: iced::Border {
                    color: iced::Color::from_rgba(0.06, 0.72, 0.50, 0.3),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(up_to_date_banner);
        }
        UpdateStatus::UpdateAvailable { info, .. } => {
            let size_text = if let Some(bytes) = info.file_size_bytes {
                format!(" • {}", format_bytes(bytes))
            } else if let Some(ref s) = info.formatted_size {
                format!(" • {}", s)
            } else {
                String::new()
            };

            let tag_badge = container(
                text(&info.version)
                    .size(11)
                    .font(styles::BOLD_FONT)
                    .color(colors::BACKGROUND),
            )
            .padding([2, 8])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::PRIMARY)),
                border: iced::Border {
                    radius: 12.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

            let header_row = row![
                column![
                    row![
                        text(&info.title)
                            .size(15)
                            .font(styles::BOLD_FONT)
                            .color(colors::TEXT_PRIMARY),
                        tag_badge,
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                    text(format!(
                        "Format: {}{}",
                        info.file_format.to_uppercase(),
                        size_text
                    ))
                    .size(12)
                    .color(colors::TEXT_MUTED),
                ]
                .spacing(4),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_DOWNLOAD)
                            .size(14)
                            .color(colors::BACKGROUND),
                        text("Download Update")
                            .size(13)
                            .font(styles::BOLD_FONT)
                            .color(colors::BACKGROUND),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([8, 18])
                .style(styles::primary_button_style)
                .on_press(SettingsMessage::StartUpdateDownload),
            ]
            .align_y(Alignment::Center);

            let changelog_box = container(
                scrollable(
                    column![
                        text("Release Notes:")
                            .size(12)
                            .font(styles::BOLD_FONT)
                            .color(colors::TEXT_PRIMARY),
                        text(&info.changelog).size(12).color(colors::TEXT_MUTED),
                    ]
                    .spacing(6),
                )
                .height(Length::Shrink)
                .style(styles::scrollable_style),
            )
            .padding([12, 14])
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

            let update_card = container(
                column![header_row, changelog_box]
                    .spacing(14)
                    .padding([16, 18]),
            )
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(update_card);
        }
        UpdateStatus::Downloading {
            info,
            downloaded_bytes,
            total_bytes,
            speed_bps,
            eta_secs,
            ..
        } => {
            let progress_ratio = if let Some(total) = total_bytes {
                if *total > 0 {
                    (*downloaded_bytes as f32 / *total as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let percent_str = format!("{:.1}%", progress_ratio * 100.0);
            let size_str = if let Some(total) = total_bytes {
                format!(
                    "{} of {}",
                    format_bytes(*downloaded_bytes),
                    format_bytes(*total)
                )
            } else {
                format_bytes(*downloaded_bytes)
            };

            let speed_str = format!("{}/s", format_bytes(*speed_bps));
            let eta_str = if let Some(secs) = eta_secs {
                if *secs < 60 {
                    format!("{}s remaining", secs)
                } else {
                    format!("{}m {}s remaining", secs / 60, secs % 60)
                }
            } else {
                "Calculating ETA...".to_string()
            };

            let header = row![
                text(format!("Downloading QDM {}...", info.version))
                    .size(14)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
                Space::with_width(Length::Fill),
                text(percent_str)
                    .size(13)
                    .font(styles::BOLD_FONT)
                    .color(colors::PRIMARY),
            ]
            .align_y(Alignment::Center);

            let bar = progress_bar(0.0..=1.0, progress_ratio)
                .height(6)
                .style(styles::progress_bar_style);

            let stats_row = row![
                text(size_str).size(12).color(colors::TEXT_MUTED),
                text(" • ").size(12).color(colors::TEXT_MUTED),
                text(speed_str).size(12).color(colors::TEXT_MUTED),
                text(" • ").size(12).color(colors::TEXT_MUTED),
                text(eta_str).size(12).color(colors::TEXT_MUTED),
                Space::with_width(Length::Fill),
                button(
                    row![
                        icon(icons::ICON_CANCEL).size(12).color(colors::ERROR),
                        text("Cancel").size(12).color(colors::ERROR),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .padding([4, 10])
                .style(styles::ghost_button_style)
                .on_press(SettingsMessage::CancelUpdateDownload),
            ]
            .align_y(Alignment::Center);

            let downloading_card = container(
                column![header, bar, stats_row]
                    .spacing(10)
                    .padding([16, 18]),
            )
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(downloading_card);
        }
        UpdateStatus::ReadyToInstall {
            info,
            file_path,
            file_size,
        } => {
            let ready_card = container(
                column![
                    row![
                        icon(icons::ICON_CHECK).size(20).color(colors::SUCCESS),
                        column![
                            text(format!("Update {} is ready to install!", info.version))
                                .size(15)
                                .font(styles::BOLD_FONT)
                                .color(colors::SUCCESS),
                            text(format!(
                                "Installer verified & saved ({})",
                                format_bytes(*file_size)
                            ))
                            .size(12)
                            .color(colors::TEXT_MUTED),
                        ]
                        .spacing(2),
                        Space::with_width(Length::Fill),
                        button(
                            row![
                                icon(icons::ICON_RETRY).size(14).color(colors::BACKGROUND),
                                text("Install Update & Restart")
                                    .size(13)
                                    .font(styles::BOLD_FONT)
                                    .color(colors::BACKGROUND),
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                        )
                        .padding([10, 20])
                        .style(styles::primary_button_style)
                        .on_press(SettingsMessage::InstallUpdatePressed),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                    container(
                        text(format!("File location: {}", file_path.display()))
                            .size(11)
                            .font(styles::MONO_FONT)
                            .color(colors::TEXT_MUTED),
                    )
                    .padding([6, 10])
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                        border: iced::Border {
                            color: colors::BORDER,
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }),
                ]
                .spacing(12)
                .padding([16, 18]),
            )
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.06, 0.72, 0.50, 0.08,
                ))),
                border: iced::Border {
                    color: iced::Color::from_rgba(0.06, 0.72, 0.50, 0.35),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(ready_card);
        }
        UpdateStatus::Error { message } => {
            let error_card = container(
                row![
                    icon(icons::ICON_WARN).size(18).color(colors::ERROR),
                    column![
                        text("Update Check or Download Failed")
                            .size(13)
                            .font(styles::BOLD_FONT)
                            .color(colors::ERROR),
                        text(message).size(12).color(colors::TEXT_MUTED),
                    ]
                    .spacing(2),
                    Space::with_width(Length::Fill),
                    button(text("Try Again").size(12).color(colors::TEXT_PRIMARY))
                        .padding([6, 14])
                        .style(styles::ghost_button_style)
                        .on_press(SettingsMessage::CheckUpdatesPressed),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.93, 0.26, 0.26, 0.10,
                ))),
                border: iced::Border {
                    color: iced::Color::from_rgba(0.93, 0.26, 0.26, 0.35),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                ..Default::default()
            });

            content = content.push(error_card);
        }
    }

    content.padding([20, 24]).width(Length::Fill).into()
}
