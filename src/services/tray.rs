//! System tray service for QDM.
//!
//! Manages the lifecycle of the system notification tray icon,
//! tray context menu, and click events.
use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Restore,
    Quit,
}

pub struct TrayManager {
    _tray_icon: TrayIcon,
    pub show_item_id: tray_icon::menu::MenuId,
    pub quit_item_id: tray_icon::menu::MenuId,
}

impl TrayManager {
    pub fn new(logo_rgba: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        // On Linux, `tray-icon` depends on `libappindicator-sys` which panics
        // if the native appindicator/ayatana library is missing or incompatible.
        // Wrap in catch_unwind so the app degrades gracefully without a tray.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::try_build(logo_rgba, width, height)
        })) {
            Ok(result) => result,
            Err(_) => {
                eprintln!("Warning: System tray initialization failed (missing library). Running without tray icon.");
                None
            }
        }
    }

    fn try_build(logo_rgba: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        let icon = tray_icon::Icon::from_rgba(logo_rgba, width, height).ok()?;

        let menu = Menu::new();
        let show_item = MenuItem::new("Open Quick Download Manager", true, None);
        let quit_item = MenuItem::new("Exit", true, None);

        let show_item_id = show_item.id().clone();
        let quit_item_id = quit_item.id().clone();

        let _ = menu.append(&show_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Quick Download Manager")
            .with_icon(icon)
            .build()
            .ok()?;

        Some(Self {
            _tray_icon: tray_icon,
            show_item_id,
            quit_item_id,
        })
    }

    /// Polls pending tray icon and context menu events without blocking.
    pub fn poll_events(&self) -> Option<TrayAction> {
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            match event {
                tray_icon::TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
                | tray_icon::TrayIconEvent::DoubleClick {
                    button: tray_icon::MouseButton::Left,
                    ..
                } => {
                    return Some(TrayAction::Restore);
                }
                _ => {}
            }
        }

        while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if event.id == self.show_item_id {
                return Some(TrayAction::Restore);
            } else if event.id == self.quit_item_id {
                return Some(TrayAction::Quit);
            }
        }

        None
    }
}
