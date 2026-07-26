use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavFilter {
    All,
    Downloading,
    Completed,
    Failed,
    Scheduled,
    Settings,
}

pub fn sidebar_view<'a, Message>(
    current_filter: NavFilter,
    downloading_count: usize,
    completed_count: usize,
    failed_count: usize,
    scheduled_count: usize,
    on_select: impl Fn(NavFilter) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // App Header / Logo: Cyan rounded square container with white download arrow
    let logo_box = container(
        icon(icons::ICON_LOGO).size(16).color(colors::BACKGROUND)
    )
    .width(36)
    .height(36)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::PRIMARY)),
        border: iced::Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let logo_title = text("QDM").size(18).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY);
    let logo_sub = text("V1.0.0-RUST").size(10).font(styles::BOLD_FONT).color(colors::TEXT_MUTED);

    let header = container(
        row![
            logo_box,
            column![logo_title, logo_sub].spacing(0)
        ]
        .spacing(12)
        .align_y(Alignment::Center)
    )
    .padding([20, 20]);

    // Navigation Items
    let items = column![
        nav_item("All Downloads", icons::ICON_DOWNLOADS, NavFilter::All, current_filter, None, on_select.clone()),
        nav_item("Downloading", icons::ICON_DOWNLOADING, NavFilter::Downloading, current_filter, Some(downloading_count), on_select.clone()),
        nav_item("Completed", icons::ICON_COMPLETED, NavFilter::Completed, current_filter, Some(completed_count), on_select.clone()),
        nav_item("Failed", icons::ICON_FAILED, NavFilter::Failed, current_filter, Some(failed_count), on_select.clone()),
        nav_item("Scheduled", icons::ICON_SCHEDULED, NavFilter::Scheduled, current_filter, Some(scheduled_count), on_select.clone()),
    ]
    .spacing(6)
    .padding([0, 12]);

    let footer = container(
        nav_item("Settings", icons::ICON_SETTINGS, NavFilter::Settings, current_filter, None, on_select)
    )
    .padding([12, 12]);

    let content = column![
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
    
    let icon_element = icon(icon_char)
        .size(16)
        .color(if is_active { colors::PRIMARY } else { colors::TEXT_MUTED });

    let label_element = text(label)
        .size(14)
        .font(if is_active { styles::BOLD_FONT } else { iced::Font::DEFAULT })
        .color(if is_active { colors::PRIMARY } else { colors::TEXT_MUTED });

    let mut row_content = row![icon_element, label_element].spacing(12).align_y(Alignment::Center);

    if let Some(count) = badge_count {
        if count > 0 {
            let badge_bg = if is_active { colors::PRIMARY } else { colors::SURFACE_HIGH };
            let badge_fg = if is_active { colors::BACKGROUND } else { colors::TEXT_MUTED };
            
            let badge = container(
                text(count.to_string()).size(11).font(styles::BOLD_FONT).color(badge_fg)
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

            row_content = row_content.push(Space::with_width(Length::Fill));
            row_content = row_content.push(badge);
        }
    }

    let btn = button(row_content)
        .width(Length::Fill)
        .padding([10, 12])
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
