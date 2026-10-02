//! # Window Management Handlers
//!
//! Handles drag-to-move, double-click maximize, minimize, maximize/restore,
//! close-to-tray, and restore-from-tray policies for QDM's custom frameless window.
//!
//! ## Platform Architecture Divergence
//! - **Windows / macOS**: Runs under [`iced::application`]. Minimizing to tray changes the
//!   window visibility mode to [`iced::window::Mode::Hidden`]. Restoring changes mode back
//!   to [`iced::window::Mode::Windowed`] and calls [`iced::window::gain_focus`].
//! - **Linux (Wayland & X11)**: Runs under [`iced::daemon`]. Wayland security protocols
//!   (specifically under GNOME Mutter / Wayland) strictly block background tray events
//!   from de-minimizing or activating existing hidden windows. To ensure 100% reliable
//!   behavior, closing to tray destroys the window ([`iced::window::close`]), clearing it
//!   from the system dock. Restoring from tray uses [`iced::window::open`] to create a
//!   fresh window, which Wayland compositors always allow to focus and render immediately.

use crate::app::{Message, QdmApp};
use iced::Task;
use std::time::Instant;

/// Invoked when the window ID is first resolved during startup.
pub(crate) fn handle_window_id_retrieved(
    app: &mut QdmApp,
    id_opt: Option<iced::window::Id>,
) -> Task<Message> {
    if let Some(id) = id_opt {
        app.window_id = Some(id);
        let resize_task = setup_native_resize(id);
        if crate::core::single_instance::take_started_minimized() {
            Task::batch([
                resize_task,
                iced::window::change_mode(id, iced::window::Mode::Hidden),
            ])
        } else {
            resize_task
        }
    } else {
        Task::none()
    }
}

/// Routes raw Iced window lifecycle events to specific action handlers.
pub(crate) fn handle_window_event(
    app: &mut QdmApp,
    id: iced::window::Id,
    event: iced::window::Event,
) -> Task<Message> {
    // Keep window_id accurate; ignore Closed event as the window is being destroyed
    if !matches!(event, iced::window::Event::Closed) {
        app.window_id = Some(id);
    }

    match event {
        iced::window::Event::Opened { .. } => Task::batch([
            iced::window::get_maximized(id).map(Message::WindowMaximizedResult),
            setup_native_resize(id),
        ]),
        iced::window::Event::Resized(_) => {
            #[cfg(target_os = "windows")]
            {
                let is_max = crate::core::window_sys::is_window_maximized();
                if app.is_maximized != is_max {
                    app.is_maximized = is_max;
                }
                Task::none()
            }
            #[cfg(not(target_os = "windows"))]
            {
                iced::window::get_maximized(id).map(Message::WindowMaximizedResult)
            }
        }
        iced::window::Event::CloseRequested => handle_window_close(app),
        iced::window::Event::Closed => {
            // Safety net: ensure window_id is cleared when a window is destroyed
            if app.window_id == Some(id) {
                app.window_id = None;
            }
            Task::none()
        }
        _ => Task::none(),
    }
}

/// Subclasses the Win32 window to enable edge resize handles and DWM corner preferences.
#[cfg(target_os = "windows")]
fn setup_native_resize(id: iced::window::Id) -> Task<Message> {
    iced::window::run_with_handle(id, |handle| {
        use raw_window_handle::RawWindowHandle;
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            let hwnd = win32_handle.hwnd.get();
            unsafe {
                crate::core::window_sys::init_borderless_resize(hwnd);
            }
        }
    })
    .map(|_| Message::WindowConfigured)
}

/// No-op on non-Windows platforms.
#[cfg(not(target_os = "windows"))]
fn setup_native_resize(_id: iced::window::Id) -> Task<Message> {
    Task::none()
}

/// Updates internal state following an asynchronous maximize query.
pub(crate) fn handle_window_maximized_result(
    app: &mut QdmApp,
    is_maximized: bool,
) -> Task<Message> {
    app.is_maximized = is_maximized;
    crate::core::window_sys::set_window_maximized(is_maximized);
    Task::none()
}

/// Handles title bar mouse press, supporting single-click window dragging
/// and double-click window maximize/restore toggling.
pub(crate) fn handle_window_drag(app: &mut QdmApp) -> Task<Message> {
    let now = Instant::now();
    let is_double_click = if let Some(last) = app.last_title_bar_click {
        now.duration_since(last).as_millis() < 350
    } else {
        false
    };
    app.last_title_bar_click = Some(now);

    if let Some(id) = app.window_id {
        if is_double_click {
            app.last_title_bar_click = None;
            app.is_maximized = !app.is_maximized;
            crate::core::window_sys::set_window_maximized(app.is_maximized);
            iced::window::toggle_maximize(id)
        } else {
            iced::window::drag(id)
        }
    } else {
        Task::none()
    }
}

/// Minimizes the current window to the taskbar/dock.
pub(crate) fn handle_window_minimize(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        iced::window::minimize(id, true)
    } else {
        Task::none()
    }
}

/// Toggles between maximized and restored window states.
pub(crate) fn handle_window_toggle_maximize(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        app.is_maximized = !app.is_maximized;
        crate::core::window_sys::set_window_maximized(app.is_maximized);
        iced::window::toggle_maximize(id)
    } else {
        Task::none()
    }
}

/// Handles a window close request based on the user's tray minimization settings.
///
/// - When `minimize_to_tray` is enabled:
///   - **Linux**: Destroys the window to clear it from dock/taskbar while keeping the daemon event loop running.
///   - **Windows / macOS**: Hides the window (`Mode::Hidden`).
/// - When `minimize_to_tray` is disabled:
///   - **Linux**: Persists download state and terminates the daemon process.
///   - **Windows / macOS**: Closes the window, allowing the application loop to finish naturally.
pub(crate) fn handle_window_close(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        if app.settings.minimize_to_tray {
            #[cfg(target_os = "linux")]
            {
                app.window_id = None;
                iced::window::close(id)
            }
            #[cfg(not(target_os = "linux"))]
            {
                iced::window::change_mode(id, iced::window::Mode::Hidden)
            }
        } else {
            #[cfg(target_os = "linux")]
            {
                let _ =
                    crate::services::shared::storage::json_store::save_downloads(&app.downloads);
                app.tray.take();
                std::process::exit(0);
            }
            #[cfg(not(target_os = "linux"))]
            {
                iced::window::close(id)
            }
        }
    } else {
        Task::none()
    }
}

/// Restores the application window when clicked from the system tray menu.
///
/// - If a window exists: un-hides and focuses it.
/// - If no window exists (Linux daemon mode): spawns a brand new window via [`iced::window::open`].
pub(crate) fn handle_restore_from_tray(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        #[cfg(not(target_os = "linux"))]
        {
            Task::batch([
                iced::window::change_mode(id, iced::window::Mode::Windowed),
                iced::window::gain_focus(id),
            ])
        }
        #[cfg(target_os = "linux")]
        {
            iced::window::gain_focus(id)
        }
    } else {
        #[cfg(target_os = "linux")]
        {
            let (id, open_task) = iced::window::open(create_main_window_settings());
            app.window_id = Some(id);
            open_task.map(Message::NewWindowOpened)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Task::none()
        }
    }
}

/// Saves application state, removes the tray icon, and cleanly exits the application.
pub(crate) fn handle_quit_from_tray(app: &mut QdmApp) -> Task<Message> {
    let _ = crate::services::shared::storage::json_store::save_downloads(&app.downloads);
    app.tray.take();
    std::process::exit(0);
}

/// Builds the standard window settings for QDM's main window.
///
/// On Linux, `application_id: "qdm"` is explicitly specified in [`iced::window::settings::PlatformSpecific`]
/// so that GNOME Shell and KDE Plasma associate the window with `packaging/qdm.desktop` (`StartupWMClass=qdm`).
#[cfg(target_os = "linux")]
pub(crate) fn create_main_window_settings() -> iced::window::Settings {
    let window_icon = crate::icons::load_window_icon();
    iced::window::Settings {
        size: iced::Size::new(1200.0, 760.0),
        min_size: Some(iced::Size::new(900.0, 600.0)),
        position: iced::window::Position::Centered,
        visible: true,
        decorations: false,
        transparent: true,
        icon: window_icon,
        exit_on_close_request: false,
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "qdm".to_string(),
            ..Default::default()
        },
        ..Default::default()
    }
}
