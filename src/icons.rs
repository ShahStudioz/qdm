#![allow(dead_code)]

use iced::widget::{text, Text};
use iced::Font;
use std::sync::OnceLock;

pub const FONTAWESOME_BYTES: &[u8] = include_bytes!("../assets/fonts/fa-solid-900.ttf");
pub const LOGO_PNG_BYTES: &[u8] = include_bytes!("../assets/images/logo.png");

static LOGO_HANDLE: OnceLock<iced::widget::image::Handle> = OnceLock::new();

pub fn get_logo_handle() -> iced::widget::image::Handle {
    LOGO_HANDLE
        .get_or_init(|| {
            if let Ok(img) = image::load_from_memory(LOGO_PNG_BYTES) {
                // Downscale the 2816x1536 logo to 160x88 (crisp even on High-DPI 4K displays)
                let resized = img.resize(160, 88, image::imageops::FilterType::Lanczos3);
                let rgba = resized.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                iced::widget::image::Handle::from_rgba(w, h, rgba.into_raw())
            } else {
                iced::widget::image::Handle::from_bytes(LOGO_PNG_BYTES)
            }
        })
        .clone()
}

/// Returns a square RGBA image buffer containing the logo centered with transparent padding,
/// suitable for OS window icons and system tray icons.
pub fn load_logo_square_rgba(size: u32) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(LOGO_PNG_BYTES).ok()?;
    let resized = img.resize(size, size, image::imageops::FilterType::Lanczos3);
    let mut square = image::RgbaImage::new(size, size);
    let x_offset = (size.saturating_sub(resized.width())) / 2;
    let y_offset = (size.saturating_sub(resized.height())) / 2;
    image::imageops::overlay(&mut square, &resized, x_offset as i64, y_offset as i64);
    Some((square.into_raw(), size, size))
}

pub fn load_window_icon() -> Option<iced::window::Icon> {
    let (rgba, width, height) = load_logo_square_rgba(64)?;
    iced::window::icon::from_rgba(rgba, width, height).ok()
}

// Font Awesome 6 Free Solid requires Weight::Black (900) in fontdb
pub const FONTAWESOME: Font = Font {
    family: iced::font::Family::Name("Font Awesome 6 Free"),
    weight: iced::font::Weight::Black,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub fn icon(unicode: char) -> Text<'static> {
    text(unicode.to_string()).font(FONTAWESOME)
}

// Sidebar & App Icons
pub const ICON_LOGO: char = '\u{f0ed}'; // fa-cloud-arrow-down
pub const ICON_DOWNLOADS: char = '\u{f03a}'; // fa-list-check
pub const ICON_DOWNLOADING: char = '\u{f0ed}'; // fa-cloud-arrow-down
pub const ICON_COMPLETED: char = '\u{f058}'; // fa-circle-check
pub const ICON_FAILED: char = '\u{f06a}'; // fa-circle-exclamation
pub const ICON_SCHEDULED: char = '\u{f017}'; // fa-clock
pub const ICON_CLOCK: char = '\u{f017}'; // fa-clock
pub const ICON_SETTINGS: char = '\u{f013}'; // fa-gear
pub const ICON_BELL: char = '\u{f0f3}'; // fa-bell
pub const ICON_SEARCH: char = '\u{f002}'; // fa-magnifying-glass
pub const ICON_PLUS: char = '\u{f067}'; // fa-plus

// Action Buttons & Modal Icons
pub const ICON_PASTE: char = '\u{f0ea}'; // fa-paste
pub const ICON_PAUSE: char = '\u{f04c}'; // fa-pause
pub const ICON_PLAY: char = '\u{f04b}'; // fa-play
pub const ICON_CANCEL: char = '\u{f00d}'; // fa-xmark
pub const ICON_FOLDER: char = '\u{f07b}'; // fa-folder-open
pub const ICON_RETRY: char = '\u{f01e}'; // fa-rotate-right
pub const ICON_TRASH: char = '\u{f1f8}'; // fa-trash-can
pub const ICON_WARN: char = '\u{f071}'; // fa-triangle-exclamation
pub const ICON_WAND: char = '\u{f0d0}'; // fa-wand-magic-sparkles
pub const ICON_CHEVRON_DOWN: char = '\u{f078}'; // fa-chevron-down
pub const ICON_CHEVRON_UP: char = '\u{f077}'; // fa-chevron-up
pub const ICON_LINK: char = '\u{f0c1}'; // fa-link
pub const ICON_SERVER: char = '\u{f233}'; // fa-server
pub const ICON_CHECK: char = '\u{f00c}'; // fa-check
pub const ICON_SPINNER: char = '\u{f110}'; // fa-spinner
pub const ICON_GLOBE: char = '\u{f0ac}'; // fa-globe
pub const ICON_GAUGE: char = '\u{f624}'; // fa-gauge-high
pub const ICON_WIFI: char = '\u{f1eb}'; // fa-wifi
pub const ICON_WIFI_SLASH: char = '\u{e073}'; // fa-wifi-slash
pub const ICON_ELLIPSIS_V: char = '\u{f142}'; // fa-ellipsis-vertical
pub const ICON_ARROW_UP: char = '\u{f062}'; // fa-arrow-up
pub const ICON_ARROW_DOWN: char = '\u{f063}'; // fa-arrow-down
pub const ICON_LIST_ORDER: char = '\u{f0cb}'; // fa-list-ol
pub const ICON_XMARK: char = '\u{f00d}'; // fa-xmark

pub const ICON_MAGNET: char = '\u{f076}'; // fa-magnet
pub const ICON_COPY: char = '\u{f0c5}'; // fa-copy
pub const ICON_USERS: char = '\u{f0c0}'; // fa-users
pub const ICON_DOWNLOAD: char = '\u{f019}'; // fa-download
pub const ICON_UPLOAD: char = '\u{f093}'; // fa-upload

// Window Control Icons
pub const ICON_WINDOW_MINIMIZE: char = '\u{f068}'; // fa-minus
pub const ICON_WINDOW_MAXIMIZE: char = '\u{f2d0}'; // fa-window-maximize
pub const ICON_WINDOW_RESTORE: char = '\u{f2d2}'; // fa-window-restore
pub const ICON_WINDOW_CLOSE: char = '\u{f00d}'; // fa-xmark

// File Type Icons
pub const ICON_MEDIA: char = '\u{f008}'; // fa-film (Video/Audio/Media)
pub const ICON_DISC: char = '\u{f51f}'; // fa-compact-disc (ISO/CD/DVD)
pub const ICON_ZIP: char = '\u{f1c6}'; // fa-file-zipper (ZIP/Archive)
pub const ICON_BOX: char = '\u{f466}'; // fa-box (Package)
pub const ICON_DOCUMENT: char = '\u{f15c}'; // fa-file-lines (Document/PDF/Office)
pub const ICON_CODE: char = '\u{f1c9}'; // fa-file-code (Code/Script/Developer)
pub const ICON_FILE: char = '\u{f15b}'; // fa-file (Generic file)
pub const ICON_GRID: char = '\u{f009}'; // fa-th-large (MSI/Installer)
pub const ICON_DATABASE: char = '\u{f1c0}'; // fa-database (SQL)
