use crate::theme::colors;
use iced::widget::{button, container, progress_bar, text_input};
use iced::{Background, Border, Color, Font, Shadow, Theme};

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
            radius: 10.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
    }
}

pub fn completed_card_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::COMPLETED_BG)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border {
            radius: 10.0.into(),
            ..Default::default()
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
            radius: 10.0.into(),
            ..Default::default()
        },
        shadow: Shadow::default(),
    }
}

pub fn sidebar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

pub fn toolbar_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(colors::SURFACE)),
        text_color: Some(colors::TEXT_PRIMARY),
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

// --- Button Styles ---

pub fn active_nav_button_style(_theme: &Theme, _status: button::Status) -> button::Style {
    button::Style {
        background: Some(Background::Color(colors::SURFACE_HIGH)),
        text_color: colors::PRIMARY,
        shadow: Shadow::default(),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
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
            radius: 8.0.into(),
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
        button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(colors::SURFACE_HIGH))
        }
        _ => None,
    };

    let border = match status {
        button::Status::Hovered | button::Status::Pressed => Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        _ => Border {
            ..Default::default()
        },
    };

    button::Style {
        background: bg,
        text_color: colors::TEXT_PRIMARY,
        shadow: Shadow::default(),
        border: border,
    }
}

pub fn window_control_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Pressed | button::Status::Hovered => (
            Some(Background::Color(colors::SURFACE_HIGH)),
            colors::TEXT_PRIMARY,
        ),
        _ => (None, colors::TEXT_MUTED),
    };
    button::Style {
        background: bg,
        text_color,
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

pub fn window_close_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let (bg, text_color) = match status {
        button::Status::Pressed => (Some(Background::Color(colors::ERROR)), colors::TEXT_PRIMARY),
        button::Status::Hovered => (Some(Background::Color(colors::ERROR)), Color::WHITE),
        _ => (None, colors::TEXT_MUTED),
    };
    button::Style {
        background: bg,
        text_color,
        border: Border::default(),
        shadow: Shadow::default(),
    }
}

// --- Progress Bar Styles ---

pub fn progress_bar_style(theme: &Theme) -> progress_bar::Style {
    progress_bar_style_with_color(colors::PRIMARY)(theme)
}

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

pub fn dark_input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused => colors::PRIMARY,
        _ => colors::BORDER,
    };
    text_input::Style {
        background: Background::Color(colors::SURFACE_HIGH),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: colors::TEXT_MUTED,
        placeholder: colors::TEXT_MUTED,
        value: colors::TEXT_PRIMARY,
        selection: colors::PRIMARY,
    }
}

pub fn transparent_text_input_style(
    _theme: &Theme,
    _status: text_input::Status,
) -> text_input::Style {
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

// --- Pick List (Single-Select Dropdown) Styles ---

pub fn pick_list_style(
    _theme: &Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let border_color = match status {
        iced::widget::pick_list::Status::Opened | iced::widget::pick_list::Status::Hovered => {
            colors::PRIMARY
        }
        _ => colors::BORDER,
    };
    iced::widget::pick_list::Style {
        text_color: colors::TEXT_PRIMARY,
        placeholder_color: colors::TEXT_MUTED,
        handle_color: colors::TEXT_MUTED,
        background: Background::Color(colors::SURFACE_HIGH),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
    }
}

pub fn pick_list_menu_style(_theme: &Theme) -> iced::overlay::menu::Style {
    iced::overlay::menu::Style {
        text_color: colors::TEXT_PRIMARY,
        background: Background::Color(colors::SURFACE),
        border: Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        selected_text_color: colors::BACKGROUND,
        selected_background: Background::Color(colors::PRIMARY),
    }
}

// --- Scrollable Styles ---

pub fn scrollable_style(
    _theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let scroller_color = match status {
        iced::widget::scrollable::Status::Hovered { .. }
        | iced::widget::scrollable::Status::Dragged { .. } => colors::PRIMARY_HOVER,
        iced::widget::scrollable::Status::Active { .. } => colors::PRIMARY,
    };

    iced::widget::scrollable::Style {
        container: iced::widget::container::Style::default(),
        vertical_rail: iced::widget::scrollable::Rail {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            scroller: iced::widget::scrollable::Scroller {
                color: scroller_color,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            },
        },
        horizontal_rail: iced::widget::scrollable::Rail {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            scroller: iced::widget::scrollable::Scroller {
                color: scroller_color,
                border: Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 4.0.into(),
                },
            },
        },
        gap: None,
    }
}
