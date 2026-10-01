#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "linux"))]
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

    // On Linux, run as a daemon. This keeps the event loop alive even when all
    // windows are closed, allowing us to destroy the window on "close to tray"
    // and open a brand new window on "Open QDM".  This is the only approach that
    // works on GNOME Wayland, which blocks apps from unminimizing/showing hidden
    // windows from a tray-icon click.
    #[cfg(target_os = "linux")]
    {
        return iced::daemon(
            "Quick Download Manager",
            QdmApp::update,
            QdmApp::view_daemon,
        )
        .subscription(QdmApp::subscription)
        .theme(QdmApp::theme_daemon)
        .style(QdmApp::style)
        .font(icons::FONTAWESOME_BYTES)
        .run_with(QdmApp::new);
    }

    // On Windows/macOS, use iced::application with a normal initial window.
    // Mode::Hidden works correctly on these platforms for tray minimize.
    #[cfg(not(target_os = "linux"))]
    {
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
}
