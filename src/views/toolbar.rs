use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

pub fn toolbar_view<'a, Message>(
    title: &'a str,
    search_text: &'a str,
    active_count: usize,
    total_speed: &'a str,
    on_search_changed: impl Fn(String) -> Message + 'a,
    on_add_url_pressed: Message,
    on_notification_pressed: Message,
    on_settings_pressed: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // Left side: Title
    let title_text = text(title).size(22).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY);

    // Single unified search bar component (320px wide) with left magnifying glass icon
    let search_icon = icon(icons::ICON_SEARCH).size(14).color(colors::TEXT_MUTED);
    let search_input_widget = text_input("Search downloads...", search_text)
        .on_input(on_search_changed)
        .padding([8, 8])
        .width(280)
        .style(styles::transparent_text_input_style);

    let search_bar = container(
        row![search_icon, search_input_widget]
            .spacing(6)
            .align_y(Alignment::Center)
    )
    .width(320)
    .padding([0, 10])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..Default::default()
    });

    // Global speed metric badge: Header "Active / Speed" above "3 Active · 12.4 MB/s"
    let metric_header = text("Active / Speed").size(10).color(colors::TEXT_MUTED);
    let metric_val = row![
        text(format!("{} Active  ·  ", active_count)).size(13).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY),
        text(total_speed).size(13).font(styles::BOLD_FONT).color(colors::PRIMARY),
    ];

    let speed_badge = column![metric_header, metric_val].spacing(1).align_x(Alignment::End);

    // "+ Add URL" button
    let add_icon = icon(icons::ICON_PLUS).size(14).color(colors::BACKGROUND);
    let add_text = text("Add URL").size(14).font(styles::BOLD_FONT).color(colors::BACKGROUND);

    let add_button = button(
        row![add_icon, add_text]
            .spacing(8)
            .align_y(Alignment::Center)
    )
    .padding([8, 16])
    .style(styles::primary_button_style)
    .on_press(on_add_url_pressed);

    // Vertical Divider Line |
    let divider = container(Space::with_width(1))
        .width(1)
        .height(24)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    // Top Right Icon Buttons: Notification Bell & Settings Gear
    let bell_btn = button(
        icon(icons::ICON_BELL).size(16).color(colors::TEXT_MUTED)
    )
    .padding([8, 8])
    .style(styles::icon_button_style)
    .on_press(on_notification_pressed);

    let gear_btn = button(
        icon(icons::ICON_SETTINGS).size(16).color(colors::TEXT_MUTED)
    )
    .padding([8, 8])
    .style(styles::icon_button_style)
    .on_press(on_settings_pressed);

    let toolbar_row = row![
        title_text,
        Space::with_width(Length::Fill),
        search_bar,
        Space::with_width(16),
        speed_badge,
        Space::with_width(16),
        add_button,
        Space::with_width(8),
        divider,
        Space::with_width(8),
        bell_btn,
        gear_btn,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    container(toolbar_row)
        .width(Length::Fill)
        .padding([16, 24])
        .style(styles::toolbar_style)
        .into()
}
