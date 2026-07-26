use iced::widget::{button, container, progress_bar, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme};
use crate::theme::colors;

pub const BOLD_FONT: Font = Font {
    family: iced::font::Family::SansSerif,
    weight: iced::font::Weight::Bold,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

pub const MONO_FONT: Font = Font {
    family: iced::font::Family::Monospace,
    weight: iced::font::Weight::Normal,
    stretch: iced::font::Stretch::Normal,
    style: iced::font::Style::Normal,
};

// --- Container Styles ---

pub fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn completed_card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::COMPLETED_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn failed_card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::FAILED_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            color: colors::FAILED_BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn sidebar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn toolbar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        shadow: Shadow::default(),
    }
}

// --- Button Styles ---

pub fn active_nav_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(colors::SURFACE_HIGH)),
        text_color: colors::PRIMARY,
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

pub fn primary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => colors::PRIMARY_HOVER,
        _ => colors::PRIMARY,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: colors::BACKGROUND,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
    }
}

pub fn icon_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let text_col = match status {
        button::Status::Hovered | button::Status::Pressed => colors::TEXT_PRIMARY,
        _ => colors::TEXT_MUTED,
    };
    button::Style {
        background: None,
        text_color: text_col,
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

pub fn ghost_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Some(Background::Color(colors::SURFACE_HIGH)),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: colors::TEXT_PRIMARY,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
    }
}

// --- Progress Bar Styles ---

pub fn progress_bar_style_with_color(bar_color: Color) -> impl Fn(&Theme) -> progress_bar::Style {
    move |_theme: &Theme| progress_bar::Style {
        background: Background::Color(colors::SURFACE_HIGH),
        bar: Background::Color(bar_color),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 4.0.into(),
        },
    }
}

// --- Text Input Styles ---

pub fn transparent_text_input_style(_theme: &Theme, _status: text_input::Status) -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: colors::TEXT_MUTED,
        placeholder: colors::TEXT_MUTED,
        value: colors::TEXT_PRIMARY,
        selection: colors::PRIMARY,
    }
}
