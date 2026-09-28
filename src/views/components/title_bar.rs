use crate::icons::{self, icon};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, image, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};

pub fn title_bar_view<'a, Message>(
    is_maximized: bool,
    on_drag: Message,
    on_minimize: Message,
    on_toggle_maximize: Message,
    on_close: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // 1. Left Branding
    let app_icon = image(icons::get_logo_handle()).height(18);
    let app_title = text("Quick Download Manager")
        .size(12)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let version_badge = container(
        text(crate::core::version::APP_VERSION_TAG)
            .size(9)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_MUTED),
    )
    .padding([1, 6])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    });

    let left_branding = row![
        app_icon,
        Space::with_width(8),
        app_title,
        Space::with_width(8),
        version_badge,
    ]
    .align_y(Alignment::Center);

    // Left branding also allows dragging!
    let left_branding_drag = mouse_area(left_branding).on_press(on_drag.clone());

    // 2. Drag Region (spans all center space)
    let drag_area = mouse_area(
        container(Space::with_width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(on_drag);

    // 3. Window Control Buttons (Platform-specific styling, placed on the right side)
    let controls: Element<'a, Message> = if cfg!(target_os = "macos") {
        let close_btn = button(
            container(icon(icons::ICON_WINDOW_CLOSE).size(8))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(14)
        .height(14)
        .padding(0)
        .style(styles::mac_traffic_light_button_style(
            iced::Color::from_rgb(1.0, 0.373, 0.337),   // #FF5F56 (Mac Close Red)
            iced::Color::from_rgb(1.0, 0.451, 0.420),   // Hover brighter red
            iced::Color::from_rgb(0.878, 0.267, 0.243), // Pressed deeper red
        ))
        .on_press(on_close);

        let min_btn = button(
            container(icon(icons::ICON_WINDOW_MINIMIZE).size(8))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(14)
        .height(14)
        .padding(0)
        .style(styles::mac_traffic_light_button_style(
            iced::Color::from_rgb(1.0, 0.741, 0.180),   // #FFBD2E (Mac Minimize Yellow)
            iced::Color::from_rgb(1.0, 0.788, 0.302),   // Hover brighter yellow
            iced::Color::from_rgb(0.871, 0.631, 0.137), // Pressed deeper yellow
        ))
        .on_press(on_minimize);

        let mac_max_icon = if is_maximized {
            icons::ICON_WINDOW_RESTORE
        } else {
            icons::ICON_PLUS
        };

        let max_btn = button(
            container(icon(mac_max_icon).size(8))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(14)
        .height(14)
        .padding(0)
        .style(styles::mac_traffic_light_button_style(
            iced::Color::from_rgb(0.153, 0.788, 0.247), // #27C93F (Mac Maximize Green)
            iced::Color::from_rgb(0.227, 0.851, 0.322), // Hover brighter green
            iced::Color::from_rgb(0.102, 0.671, 0.161), // Pressed deeper green
        ))
        .on_press(on_toggle_maximize);

        row![Space::with_width(6), close_btn, min_btn, max_btn]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
    } else {
        let min_btn = button(
            container(icon(icons::ICON_WINDOW_MINIMIZE).size(10))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(44)
        .height(32)
        .padding(0)
        .style(styles::window_control_button_style)
        .on_press(on_minimize);

        let max_icon = if is_maximized {
            icons::ICON_WINDOW_RESTORE
        } else {
            icons::ICON_WINDOW_MAXIMIZE
        };

        let max_btn = button(
            container(icon(max_icon).size(10))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(44)
        .height(32)
        .padding(0)
        .style(styles::window_control_button_style)
        .on_press(on_toggle_maximize);

        let close_btn = button(
            container(icon(icons::ICON_WINDOW_CLOSE).size(11))
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center),
        )
        .width(44)
        .height(32)
        .padding(0)
        .style(styles::window_close_button_style(is_maximized))
        .on_press(on_close);

        row![min_btn, max_btn, close_btn]
            .spacing(0)
            .align_y(Alignment::Center)
            .into()
    };

    let bar_row = if cfg!(target_os = "macos") {
        row![
            controls,
            drag_area,
            left_branding_drag,
            Space::with_width(14),
        ]
        .align_y(Alignment::Center)
    } else {
        row![
            Space::with_width(12),
            left_branding_drag,
            drag_area,
            controls,
        ]
        .align_y(Alignment::Center)
    };

    let top_radius = if is_maximized {
        0.0
    } else {
        styles::WINDOW_INNER_CORNER_RADIUS
    };

    column![container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(32.0))
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                radius: iced::border::Radius {
                    top_left: top_radius,
                    top_right: top_radius,
                    bottom_right: 0.0,
                    bottom_left: 0.0,
                },
                ..Default::default()
            },
            ..Default::default()
        }),]
    .into()
}
