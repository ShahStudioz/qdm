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
    #[cfg(target_os = "linux")]
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", err);
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    let start_minimized = args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--minimized" | "--startup" | "--silent"));
    core::single_instance::set_started_minimized(start_minimized);
    let cli_arg = args.into_iter().find(|arg| !arg.starts_with('-'));

    // Single-instance check: notify existing instance if running, then exit immediately
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
            visible: !start_minimized,
            decorations: false,
            transparent: true,
            icon: window_icon,
            ..Default::default()
        })
        .font(icons::FONTAWESOME_BYTES)
        .run_with(QdmApp::new)
}
