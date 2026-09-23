use crate::icons::{self, icon};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, image, row, text, Space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavFilter {
    All,
    Downloading,
    Completed,
    Failed,
    Scheduled,
    Queue,
    Settings,
}

pub fn sidebar_view<'a, Message>(
    current_filter: NavFilter,
    downloading_count: usize,
    completed_count: usize,
    failed_count: usize,
    scheduled_count: usize,
    update_status: &'a crate::services::updater::UpdateStatus,
    on_select: impl Fn(NavFilter) -> Message + 'a + Clone,
    on_open_updates: impl Fn() -> Message + 'a + Clone,
    on_install_update: impl Fn() -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let logo_img = image(icons::get_logo_handle()).height(22);
    let logo_box = container(logo_img)
        .width(42)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    let logo_title = text("QDM")
        .size(18)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);
    let logo_sub = text(crate::core::version::APP_VERSION_SIDEBAR)
        .size(10)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_MUTED);

    let header = container(
        row![logo_box, column![logo_title, logo_sub].spacing(0)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([20, 20]);

    let items = column![
        nav_item(
            "All Downloads",
            icons::ICON_DOWNLOADS,
            NavFilter::All,
            current_filter,
            None,
            on_select.clone()
        ),
        nav_item(
            "Downloading",
            icons::ICON_DOWNLOADING,
            NavFilter::Downloading,
            current_filter,
            Some(downloading_count),
            on_select.clone()
        ),
        nav_item(
            "Completed",
            icons::ICON_COMPLETED,
            NavFilter::Completed,
            current_filter,
            Some(completed_count),
            on_select.clone()
        ),
        nav_item(
            "Failed",
            icons::ICON_FAILED,
            NavFilter::Failed,
            current_filter,
            Some(failed_count),
            on_select.clone()
        ),
        nav_item(
            "Scheduled",
            icons::ICON_SCHEDULED,
            NavFilter::Scheduled,
            current_filter,
            Some(scheduled_count),
            on_select.clone()
        ),
    ]
    .spacing(6)
    .padding([0, 12]);

    let mut footer_col = column![].spacing(6);

    match update_status {
        crate::services::updater::UpdateStatus::UpdateAvailable { .. } => {
            footer_col = footer_col.push(update_sidebar_btn(
                "Update Available",
                icons::ICON_DOWNLOAD,
                colors::ERROR,
                on_open_updates(),
            ));
        }
        crate::services::updater::UpdateStatus::Downloading { .. } => {
            footer_col = footer_col.push(update_sidebar_btn(
                "Downloading Update...",
                icons::ICON_DOWNLOAD,
                colors::WARNING,
                on_open_updates(),
            ));
        }
        crate::services::updater::UpdateStatus::ReadyToInstall { .. } => {
            footer_col = footer_col.push(update_sidebar_btn(
                "Install Update",
                icons::ICON_RETRY,
                colors::SUCCESS,
                on_install_update(),
            ));
        }
        _ => {}
    }

    footer_col = footer_col.push(nav_item(
        "Settings",
        icons::ICON_SETTINGS,
        NavFilter::Settings,
        current_filter,
        None,
        on_select,
    ));

    let footer = container(footer_col).padding([12, 12]);

    let top_border = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let content = column![
        top_border,
        header,
        items,
        Space::with_height(Length::Fill),
        footer,
    ]
    .width(240);

    container(content)
        .width(240)
        .height(Length::Fill)
        .style(styles::sidebar_style)
        .into()
}

fn update_sidebar_btn<'a, Message>(
    label: &'static str,
    icon_char: char,
    dot_color: iced::Color,
    on_click: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let icon_element = icon(icon_char).size(15).color(colors::PRIMARY);
    let label_element = text(label)
        .size(13)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let dot_indicator = container(Space::with_width(8))
        .width(8)
        .height(8)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(dot_color)),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });

    let inner = row![
        Space::with_width(12),
        icon_element,
        label_element,
        Space::with_width(Length::Fill),
        dot_indicator,
        Space::with_width(12),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    button(inner)
        .width(Length::Fill)
        .padding([8, 0])
        .style(move |_theme, status| {
            let bg = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(iced::Background::Color(colors::SURFACE_HIGH))
                }
                _ => Some(iced::Background::Color(iced::Color::from_rgba(0.0, 0.7, 0.85, 0.08))),
            };
            button::Style {
                background: bg,
                border: iced::Border {
                    color: iced::Color::from_rgba(0.0, 0.7, 0.85, 0.25),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                shadow: Default::default(),
                text_color: colors::TEXT_PRIMARY,
            }
        })
        .on_press(on_click)
        .into()
}

fn nav_item<'a, Message>(
    label: &'static str,
    icon_char: char,
    item_filter: NavFilter,
    active_filter: NavFilter,
    badge_count: Option<usize>,
    on_select: impl Fn(NavFilter) -> Message + 'a,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let is_active = item_filter == active_filter;

    let left_indicator = container(Space::with_width(4))
        .width(4)
        .height(24)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(if is_active {
                colors::PRIMARY
            } else {
                iced::Color::TRANSPARENT
            })),
            border: iced::Border {
                radius: iced::border::Radius {
                    top_left: 4.0,
                    bottom_left: 4.0,
                    top_right: 0.0,
                    bottom_right: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        });

    let icon_element = icon(icon_char).size(16).color(if is_active {
        colors::PRIMARY
    } else {
        colors::TEXT_MUTED
    });

    let label_element = text(label)
        .size(14)
        .font(if is_active {
            styles::BOLD_FONT
        } else {
            iced::Font::DEFAULT
        })
        .color(if is_active {
            colors::PRIMARY
        } else {
            colors::TEXT_MUTED
        });

    let mut inner_row = row![icon_element, label_element]
        .spacing(12)
        .align_y(Alignment::Center);

    if let Some(count) = badge_count {
        if count > 0 {
            let badge_bg = if is_active {
                colors::PRIMARY
            } else {
                colors::SURFACE_HIGH
            };
            let badge_fg = if is_active {
                colors::BACKGROUND
            } else {
                colors::TEXT_MUTED
            };

            let badge = container(
                text(count.to_string())
                    .size(11)
                    .font(styles::BOLD_FONT)
                    .color(badge_fg),
            )
            .padding([2, 8])
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(badge_bg)),
                border: iced::Border {
                    radius: 10.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            });

            inner_row = inner_row.push(Space::with_width(Length::Fill));
            inner_row = inner_row.push(badge);
        }
    }

    let item_content = row![
        left_indicator,
        Space::with_width(8),
        inner_row,
        Space::with_width(8),
    ]
    .align_y(Alignment::Center);

    let btn = button(item_content)
        .width(Length::Fill)
        .padding([8, 0])
        .style(move |theme, status| {
            if is_active {
                styles::active_nav_button_style(theme, status)
            } else {
                styles::ghost_button_style(theme, status)
            }
        })
        .on_press(on_select(item_filter));

    btn.into()
}
