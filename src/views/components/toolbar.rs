use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

pub fn toolbar_view<'a, Message>(
    title: &'a str,
    search_text: &'a str,
    active_count: usize,
    total_speed: impl Into<String>,
    is_menu_open: bool,
    on_search_changed: impl Fn(String) -> Message + 'a,
    on_add_url_pressed: Message,
    on_toggle_menu: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let title_text = text(title).size(22).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY);

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

    let metric_header = text("Active / Speed").size(10).color(colors::TEXT_MUTED);
    let metric_val = row![
        text(format!("{} Active  ·  ", active_count)).size(13).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY),
        text(total_speed.into()).size(13).font(styles::BOLD_FONT).color(colors::PRIMARY),
    ];

    let speed_badge = column![metric_header, metric_val].spacing(1).align_x(Alignment::End);

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

    let divider = container(Space::with_width(1))
        .width(1)
        .height(24)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let menu_btn = button(
        icon(icons::ICON_ELLIPSIS_V).size(16).color(if is_menu_open { colors::PRIMARY } else { colors::TEXT_MUTED })
    )
    .padding([8, 10])
    .style(styles::icon_button_style)
    .on_press(on_toggle_menu);

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
        menu_btn,
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    let toolbar_container = container(toolbar_row)
        .width(Length::Fill)
        .padding([16, 24])
        .style(styles::toolbar_style);

    toolbar_container.into()
}

pub fn dropdown_overlay<'a, Message: Clone + 'static>(
    on_open_queue: Message,
    on_open_settings: Message,
) -> Element<'a, Message> {
    let queue_item = button(
        row![
            icon(icons::ICON_LIST_ORDER).size(14).color(colors::PRIMARY),
            text("Downloads Queue").size(13).color(colors::TEXT_PRIMARY),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .width(Length::Fill)
    .style(styles::ghost_button_style)
    .on_press(on_open_queue);

    let settings_item = button(
        row![
            icon(icons::ICON_SETTINGS).size(14).color(colors::PRIMARY),
            text("Settings").size(13).color(colors::TEXT_PRIMARY),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .width(Length::Fill)
    .style(styles::ghost_button_style)
    .on_press(on_open_settings);

    let dropdown_menu = container(column![queue_item, settings_item].spacing(2))
        .width(180)
        .padding(4)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.6),
                offset: iced::Vector::new(0.0, 6.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        });

    container(dropdown_menu)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::End)
        .align_y(Alignment::Start)
        .padding(iced::padding::top(68).right(24))
        .into()
}
