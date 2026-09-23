use iced::widget::{button, column, container, image, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

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

    // 3. Window Control Buttons
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
    .style(styles::window_close_button_style)
    .on_press(on_close);

    let controls = row![min_btn, max_btn, close_btn]
        .spacing(0)
        .align_y(Alignment::Center);

    let bar_row = row![
        Space::with_width(12),
        left_branding_drag,
        drag_area,
        controls,
    ]
    .align_y(Alignment::Center);

    column![
        container(bar_row)
            .width(Length::Fill)
            .height(Length::Fixed(32.0))
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE)),
                ..Default::default()
            }),
    ]
    .into()
}
