//! Window management handlers for QDM.
//!
//! Handles drag-to-move, double-click maximize, minimize, maximize/restore,
//! and close actions for the custom window frame.

use crate::app::{Message, QdmApp};
use iced::Task;
use std::time::Instant;

pub(crate) fn handle_window_id_retrieved(app: &mut QdmApp, id_opt: Option<iced::window::Id>) -> Task<Message> {
    if let Some(id) = id_opt {
        app.window_id = Some(id);
    }
    Task::none()
}

pub(crate) fn handle_window_event(app: &mut QdmApp, id: iced::window::Id, event: iced::window::Event) -> Task<Message> {
    app.window_id = Some(id);
    match event {
        iced::window::Event::Opened { .. } => {
            iced::window::get_maximized(id).map(Message::WindowMaximizedResult)
        }
        iced::window::Event::Resized(_) => {
            iced::window::get_maximized(id).map(Message::WindowMaximizedResult)
        }
        iced::window::Event::CloseRequested => {
            handle_window_close(app)
        }
        _ => Task::none(),
    }
}

pub(crate) fn handle_window_maximized_result(app: &mut QdmApp, is_maximized: bool) -> Task<Message> {
    app.is_maximized = is_maximized;
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
        iced::window::toggle_maximize(id)
    } else {
        Task::none()
    }
}

pub(crate) fn handle_window_close(app: &mut QdmApp) -> Task<Message> {
    if let Some(id) = app.window_id {
        if app.settings.minimize_to_tray {
            iced::window::minimize(id, true)
        } else {
            iced::window::close(id)
        }
    } else {
        Task::none()
    }
}
