//! Window management handlers for QDM.
//!
//! Handles drag-to-move, double-click maximize, minimize, maximize/restore,
//! and close actions for the custom window frame.

use crate::app::{Message, QdmApp};
use iced::Task;
use std::time::Instant;

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

pub(crate) fn handle_window_event(
    app: &mut QdmApp,
    id: iced::window::Id,
    event: iced::window::Event,
) -> Task<Message> {
    app.window_id = Some(id);
    match event {
        iced::window::Event::Opened { .. } => Task::batch([
            iced::window::get_maximized(id).map(Message::WindowMaximizedResult),
            setup_native_resize(id),
        ]),
        iced::window::Event::Resized(_) => {
            #[cfg(target_os = "windows")]
            {
                let is_max = crate::core::window_sys::windows::is_window_maximized();
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
        _ => Task::none(),
    }
}

#[cfg(target_os = "windows")]
fn setup_native_resize(id: iced::window::Id) -> Task<Message> {
    iced::window::run_with_handle(id, |handle| {
        use raw_window_handle::RawWindowHandle;
        if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
            let hwnd = win32_handle.hwnd.get();
            unsafe {
                crate::core::window_sys::windows::init_borderless_resize(hwnd);
            }
        }
    })
    .map(|_| Message::WindowConfigured)
}

#[cfg(not(target_os = "windows"))]
fn setup_native_resize(_id: iced::window::Id) -> Task<Message> {
    Task::none()
}

pub(crate) fn handle_window_maximized_result(
    app: &mut QdmApp,
    is_maximized: bool,
) -> Task<Message> {
    app.is_maximized = is_maximized;
    crate::core::window_sys::windows::set_window_maximized(is_maximized);
    Task::none()
}

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
            crate::core::window_sys::windows::set_window_maximized(app.is_maximized);
            iced::window::toggle_maximize(id)
        } else {
            iced::window::drag(id)
        }
    } else {
        Task::none()
    }
}

pub(crate) fn handle_window_minimize(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        iced::window::minimize(id, true)
    } else {
        Task::none()
    }
}

pub(crate) fn handle_window_toggle_maximize(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        app.is_maximized = !app.is_maximized;
        crate::core::window_sys::windows::set_window_maximized(app.is_maximized);
        iced::window::toggle_maximize(id)
    } else {
        Task::none()
    }
}

pub(crate) fn handle_window_close(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        if app.settings.minimize_to_tray {
            iced::window::change_mode(id, iced::window::Mode::Hidden)
        } else {
            iced::window::close(id)
        }
    } else {
        Task::none()
    }
}

pub(crate) fn handle_restore_from_tray(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        Task::batch([
            iced::window::change_mode(id, iced::window::Mode::Windowed),
            iced::window::minimize(id, false),
            iced::window::gain_focus(id),
        ])
    } else {
        Task::none()
    }
}

pub(crate) fn handle_quit_from_tray(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        iced::window::close(id)
    } else {
        std::process::exit(0);
    }
}
