//! # Quick Download Manager (QDM)
//!
//! Modern, high-performance, open-source download accelerator and torrent client
//! built with Rust and the Iced GUI toolkit.
//!
//! ## Platform Runtime Architecture
//! - **Linux**: Runs via [`iced::daemon`]. Wayland compositors (specifically GNOME Mutter)
//!   enforce strict window activation security policies that disallow background DBus
//!   status notifier (tray) messages from de-minimizing hidden windows. Running as an
//!   `iced::daemon` allows QDM to close and destroy the window when minimizing to tray
//!   (removing it from the dock), and spawn a brand-new window via [`iced::window::open`]
//!   when the user clicks the tray icon.
//! - **Windows & macOS**: Runs via [`iced::application`] with a frameless, transparent
//!   window configured with custom DWM hit-testing and traffic-light controls.
//!
//! ## Single Instance & CLI Arguments
//! - Ensures only one process instance runs at any time via platform IPC
//!   (Named Pipes on Windows, Unix domain sockets on Linux/macOS).
//! - Passes URL and torrent arguments from secondary launches to the primary instance.

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

/// Main application entry point.
fn main() -> iced::Result {
    // On Linux, configure runtime environment safeguards:
    #[cfg(target_os = "linux")]
    {
        // Enable fallback graphics backends and allow software rendering (Mesa/LLVMpipe in VMs)
        if std::env::var_os("WGPU_BACKEND").is_none() {
            std::env::set_var("WGPU_BACKEND", "vulkan,gl");
        }
        if std::env::var_os("WGPU_ALLOW_INSECURE").is_none() {
            std::env::set_var("WGPU_ALLOW_INSECURE", "1");
        }
        // When running inside an AppImage, prevent bundled GLib from loading incompatible host
        // GIO/GVFS modules (which cause undefined symbol crashes like `g_task_set_static_name`)
        if (std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some())
            && std::env::var_os("GIO_MODULE_DIR").is_none()
        {
            std::env::set_var("GIO_MODULE_DIR", "");
        }
    }
    // On Linux, initialize GTK runtime for AppIndicator / Ayatana tray support
    #[cfg(target_os = "linux")]
    if let Err(err) = gtk::init() {
        eprintln!("Failed to initialize GTK: {}", err);
    }

    // On Windows, set explicit AppUserModelID so the taskbar groups properly with shortcuts
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let aumid: Vec<u16> = std::ffi::OsStr::new("shahstudioz.qdm.app")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(aumid.as_ptr());
        }
    }

    // Parse command line flags (--minimized, --startup, --silent)
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
    // and open a brand new window on "Open QDM". This is the only approach that
    // works on GNOME Wayland, which blocks apps from unminimizing/showing hidden
    // windows from a tray-icon click.
    #[cfg(target_os = "linux")]
    {
        iced::daemon(
            "Quick Download Manager",
            QdmApp::update,
            QdmApp::view_daemon,
        )
        .subscription(QdmApp::subscription)
        .theme(QdmApp::theme_daemon)
        .style(QdmApp::style)
        .font(icons::FONTAWESOME_BYTES)
        .run_with(QdmApp::new)
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
