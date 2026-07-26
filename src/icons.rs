use iced::widget::{text, Text};
use iced::Font;

pub const FONTAWESOME_BYTES: &[u8] = include_bytes!("../assets/fonts/fa-solid-900.ttf");

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
pub const ICON_LOGO: char = '\u{f0ed}';          // fa-cloud-arrow-down
pub const ICON_DOWNLOADS: char = '\u{f03a}';     // fa-list-check
pub const ICON_DOWNLOADING: char = '\u{f0ed}';   // fa-cloud-arrow-down
pub const ICON_COMPLETED: char = '\u{f058}';     // fa-circle-check
pub const ICON_FAILED: char = '\u{f06a}';        // fa-circle-exclamation
pub const ICON_SCHEDULED: char = '\u{f017}';     // fa-clock
pub const ICON_SETTINGS: char = '\u{f013}';      // fa-gear
pub const ICON_BELL: char = '\u{f0f3}';          // fa-bell
pub const ICON_SEARCH: char = '\u{f002}';        // fa-magnifying-glass
pub const ICON_PLUS: char = '\u{f067}';          // fa-plus

// Action Buttons & Modal Icons
pub const ICON_PAUSE: char = '\u{f04c}';         // fa-pause
pub const ICON_PLAY: char = '\u{f04b}';          // fa-play
pub const ICON_CANCEL: char = '\u{f00d}';        // fa-xmark
pub const ICON_FOLDER: char = '\u{f07b}';        // fa-folder-open
pub const ICON_RETRY: char = '\u{f01e}';         // fa-rotate-right
pub const ICON_TRASH: char = '\u{f1f8}';         // fa-trash-can
pub const ICON_WARN: char = '\u{f071}';          // fa-triangle-exclamation
pub const ICON_WAND: char = '\u{f0d0}';          // fa-wand-magic-sparkles
pub const ICON_CHEVRON_DOWN: char = '\u{f078}';  // fa-chevron-down
pub const ICON_CHEVRON_UP: char = '\u{f077}';    // fa-chevron-up

// File Type Icons
pub const ICON_DISC: char = '\u{f51e}';          // fa-compact-disc (ISO)
pub const ICON_ZIP: char = '\u{f1c6}';           // fa-file-zipper (ZIP)
pub const ICON_BOX: char = '\u{f49e}';           // fa-box-archive (TAR.GZ)
pub const ICON_GRID: char = '\u{f009}';          // fa-th-large (MSI/Installer)
pub const ICON_DATABASE: char = '\u{f1c0}';      // fa-database (SQL)
