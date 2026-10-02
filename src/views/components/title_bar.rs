//! # Custom Title Bar Component
//!
//! Renders the top window navigation bar for QDM's custom frameless window.
//!
//! ## Platform Behaviors
//! - **macOS**: Places standard 14px "traffic light" window controls on the top-left
//!   (Red = Close, Yellow = Minimize, Green = Maximize/Restore). The application title
//!   and version badge are shifted to the right of the center drag region to honor
//!   macOS human interface guidelines.
//! - **Windows & Linux**: Places application branding and version badge on the top-left,
//!   a large draggable area across the center, and standard 44x32px rectangular control
//!   buttons on the far right. Windows 11 rounded corners are applied to the top edges
//!   when windowed, and squared when maximized.

use crate::icons::{self, icon};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, image, mouse_area, row, text, Space};
use iced::{Alignment, Element, Length};

/// Renders the complete application title bar with window controls, draggable areas,
/// and branding.
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
    // 1. Branding: Logo, App Name, and Version Tag
    let left_branding = branding_section(on_drag.clone());

    // 2. Drag Region: Invisible spacer that triggers window dragging on mouse press
    let drag_area = mouse_area(
        container(Space::with_width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(on_drag);

    // 3. Platform-Specific Window Controls
    let is_macos = cfg!(target_os = "macos");
    let controls = if is_macos {
        mac_window_controls(is_maximized, on_close, on_minimize, on_toggle_maximize)
    } else {
        standard_window_controls(is_maximized, on_close, on_minimize, on_toggle_maximize)
    };

    // 4. Bar Row Layout Assembly
    // macOS: [Traffic Lights, Center Drag Area, Branding, Right Padding]
    // Windows/Linux: [Left Padding, Branding, Center Drag Area, Right Control Buttons]
    let bar_row = if is_macos {
        row![controls, drag_area, left_branding, Space::with_width(14),].align_y(Alignment::Center)
    } else {
        row![Space::with_width(12), left_branding, drag_area, controls,].align_y(Alignment::Center)
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

/// Builds the application logo, title text, and version badge wrapped in a draggable mouse area.
fn branding_section<'a, Message>(on_drag: Message) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
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

    let row_content = row![
        app_icon,
        Space::with_width(8),
        app_title,
        Space::with_width(8),
        version_badge,
    ]
    .align_y(Alignment::Center);

    mouse_area(row_content).on_press(on_drag).into()
}

/// Renders macOS-styled circular traffic light buttons (Close, Minimize, Maximize).
fn mac_window_controls<'a, Message>(
    is_maximized: bool,
    on_close: Message,
    on_minimize: Message,
    on_toggle_maximize: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
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
        iced::Color::from_rgb(1.0, 0.373, 0.337), // #FF5F56 (Close Red)
        iced::Color::from_rgb(1.0, 0.451, 0.420), // Hover bright red
        iced::Color::from_rgb(0.878, 0.267, 0.243), // Pressed dark red
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
        iced::Color::from_rgb(1.0, 0.741, 0.180), // #FFBD2E (Minimize Yellow)
        iced::Color::from_rgb(1.0, 0.788, 0.302), // Hover bright yellow
        iced::Color::from_rgb(0.871, 0.631, 0.137), // Pressed dark yellow
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
        iced::Color::from_rgb(0.153, 0.788, 0.247), // #27C93F (Maximize Green)
        iced::Color::from_rgb(0.227, 0.851, 0.322), // Hover bright green
        iced::Color::from_rgb(0.102, 0.671, 0.161), // Pressed dark green
    ))
    .on_press(on_toggle_maximize);

    row![Space::with_width(6), close_btn, min_btn, max_btn]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
}

/// Renders standard Windows and Linux rectangular control buttons on the top right.
fn standard_window_controls<'a, Message>(
    is_maximized: bool,
    on_close: Message,
    on_minimize: Message,
    on_toggle_maximize: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
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
}
