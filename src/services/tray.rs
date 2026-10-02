//! # System Tray Service
//!
//! Manages the lifecycle of the system notification tray icon, native context menu,
//! and background event polling across Windows, macOS, and Linux.
//!
//! ## Linux Library Graceful Degradation
//! On Linux, `tray-icon` depends on `libappindicator-sys` / `libayatana-appindicator3`.
//! If the system does not have these dynamic libraries installed, native dlopen calls
//! panic within the underlying C-binding. We isolate initialization inside
//! [`std::panic::catch_unwind`] to allow the application to degrade gracefully into a
//! normal desktop app rather than crashing on launch.

use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

/// Represents user actions dispatched from the system tray icon or context menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Restore or focus the primary application window.
    Restore,
    /// Terminate the application process cleanly.
    Quit,
}

/// Encapsulates the native tray icon handle and menu item identifiers.
pub struct TrayManager {
    _tray_icon: TrayIcon,
    /// Menu item ID for "Open Quick Download Manager".
    pub show_item_id: tray_icon::menu::MenuId,
    /// Menu item ID for "Exit".
    pub quit_item_id: tray_icon::menu::MenuId,
}

impl TrayManager {
    /// Attempts to build and display the system tray icon.
    ///
    /// Returns `None` if tray icon creation fails or if native libraries are missing.
    pub fn new(logo_rgba: Vec<u8>, width: u32, height: u32) -> Option<Self> {
        // On Linux, `tray-icon` depends on `libappindicator-sys` which panics
        // if the native appindicator/ayatana library is missing or incompatible.
        // Wrap in catch_unwind so the app degrades gracefully without a tray.
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::try_build(logo_rgba, width, height)
        })) {
            Ok(result) => result,
            Err(_) => {
                eprintln!("[QDM Tray] System tray initialization failed (missing native library). Running in window-only mode.");
                None
            }
        }
    }

    /// Internal builder that configures the icon, menu hierarchy, and tooltip.
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

    /// Non-blocking check for tray icon clicks or menu selections.
    ///
    /// Returns `Some(TrayAction)` when the user interacts with the tray, or `None` if idle.
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
