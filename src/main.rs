#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::{window, Size};

mod app;
mod core;
mod icons;
mod models;
mod services;
mod theme;
mod views;

use app::QdmApp;

fn main() -> iced::Result {
    // Single-instance check: notify existing instance if running, then exit immediately
    let cli_arg = std::env::args().nth(1);
    if core::single_instance::notify_existing_or_acquire(cli_arg) {
        return Ok(());
    }

    let window_icon = icons::load_window_icon();

    iced::application("Quick Download Manager", QdmApp::update, QdmApp::view)
        .subscription(QdmApp::subscription)
        .theme(QdmApp::theme)
        .style(QdmApp::style)
        .window(window::Settings {
            size: Size::new(1200.0, 760.0),
            min_size: Some(Size::new(900.0, 600.0)),
            position: window::Position::Centered,
            decorations: false,
            transparent: true,
            icon: window_icon,
            ..Default::default()
        })
        .font(icons::FONTAWESOME_BYTES)
        .run_with(QdmApp::new)
}
