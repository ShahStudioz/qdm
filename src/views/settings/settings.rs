use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    General,
    Downloads,
    Network,
    Appearance,
}

fn default_true() -> bool {
    true
}

fn default_simultaneous_downloads() -> usize {
    3
}

fn default_notification_sound() -> String {
    "Default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsModel {
    #[serde(default)]
    pub active_tab: SettingsTab,
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(skip)]
    pub launch_at_startup_anim: f32,

    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(skip)]
    pub minimize_to_tray_anim: f32,

    #[serde(default = "crate::core::utils::paths::get_default_download_dir")]
    pub download_folder: String,
    #[serde(default = "default_simultaneous_downloads")]
    pub simultaneous_downloads: usize,

    #[serde(default = "default_true")]
    pub show_notifications: bool,
    #[serde(skip)]
    pub show_notifications_anim: f32,

    #[serde(default = "default_notification_sound")]
    pub notification_sound: String,

    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    #[serde(skip)]
    pub auto_check_updates_anim: f32,
}

impl Default for SettingsModel {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::General,
            launch_at_startup: false,
            launch_at_startup_anim: 0.0,

            minimize_to_tray: true,
            minimize_to_tray_anim: 1.0,

            download_folder: crate::core::utils::paths::get_default_download_dir(),
            simultaneous_downloads: 3,

            show_notifications: true,
            show_notifications_anim: 1.0,

            notification_sound: "Default".to_string(),

            auto_check_updates: true,
            auto_check_updates_anim: 1.0,
        }
    }
}

impl SettingsModel {
    pub fn sync_animations(&mut self) {
        self.launch_at_startup_anim = if self.launch_at_startup { 1.0 } else { 0.0 };
        self.minimize_to_tray_anim = if self.minimize_to_tray { 1.0 } else { 0.0 };
        self.show_notifications_anim = if self.show_notifications { 1.0 } else { 0.0 };
        self.auto_check_updates_anim = if self.auto_check_updates { 1.0 } else { 0.0 };
    }

    pub fn is_animating(&self) -> bool {
        let target_startup = if self.launch_at_startup { 1.0 } else { 0.0 };
        let target_tray = if self.minimize_to_tray { 1.0 } else { 0.0 };
        let target_notify = if self.show_notifications { 1.0 } else { 0.0 };
        let target_updates = if self.auto_check_updates { 1.0 } else { 0.0 };

        (self.launch_at_startup_anim - target_startup).abs() > 0.005
            || (self.minimize_to_tray_anim - target_tray).abs() > 0.005
            || (self.show_notifications_anim - target_notify).abs() > 0.005
            || (self.auto_check_updates_anim - target_updates).abs() > 0.005
    }

    pub fn tick_animation(&mut self) {
        let target_startup = if self.launch_at_startup { 1.0 } else { 0.0 };
        let target_tray = if self.minimize_to_tray { 1.0 } else { 0.0 };
        let target_notify = if self.show_notifications { 1.0 } else { 0.0 };
        let target_updates = if self.auto_check_updates { 1.0 } else { 0.0 };

        self.launch_at_startup_anim += (target_startup - self.launch_at_startup_anim) * 0.35;
        self.minimize_to_tray_anim += (target_tray - self.minimize_to_tray_anim) * 0.35;
        self.show_notifications_anim += (target_notify - self.show_notifications_anim) * 0.35;
        self.auto_check_updates_anim += (target_updates - self.auto_check_updates_anim) * 0.35;

        if (self.launch_at_startup_anim - target_startup).abs() < 0.005 {
            self.launch_at_startup_anim = target_startup;
        }
        if (self.minimize_to_tray_anim - target_tray).abs() < 0.005 {
            self.minimize_to_tray_anim = target_tray;
        }
        if (self.show_notifications_anim - target_notify).abs() < 0.005 {
            self.show_notifications_anim = target_notify;
        }
        if (self.auto_check_updates_anim - target_updates).abs() < 0.005 {
            self.auto_check_updates_anim = target_updates;
        }
    }
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    TabSelected(SettingsTab),
    ToggleStartup(bool),
    ToggleTray(bool),
    FolderChanged(String),
    BrowseFolderPressed,
    StepperDecrement,
    StepperIncrement,
    ToggleNotifications(bool),
    SoundChanged(String),
    ToggleUpdates(bool),
    CheckUpdatesPressed,
    ResetDefaultsPressed,
    SaveChangesPressed,
}

pub fn update(model: &mut SettingsModel, message: SettingsMessage) {
    match message {
        SettingsMessage::TabSelected(tab) => model.active_tab = tab,
        SettingsMessage::ToggleStartup(val) => model.launch_at_startup = val,
        SettingsMessage::ToggleTray(val) => model.minimize_to_tray = val,
        SettingsMessage::FolderChanged(folder) => model.download_folder = folder,
        SettingsMessage::BrowseFolderPressed => {
            println!("[QDM] Browse download folder pressed");
        }
        SettingsMessage::StepperDecrement => {
            if model.simultaneous_downloads > 1 {
                model.simultaneous_downloads -= 1;
            }
        }
        SettingsMessage::StepperIncrement => {
            if model.simultaneous_downloads < 16 {
                model.simultaneous_downloads += 1;
            }
        }
        SettingsMessage::ToggleNotifications(val) => model.show_notifications = val,
        SettingsMessage::SoundChanged(sound) => model.notification_sound = sound,
        SettingsMessage::ToggleUpdates(val) => model.auto_check_updates = val,
        SettingsMessage::CheckUpdatesPressed => {
            println!("[QDM] Checking for updates...");
        }
        SettingsMessage::ResetDefaultsPressed => {
            *model = SettingsModel::default();
            let _ = crate::services::storage::json_store::save_settings(model);
        }
        SettingsMessage::SaveChangesPressed => {
            let _ = crate::services::storage::json_store::save_settings(model);
            println!("[QDM] Settings saved: {:?}", model.download_folder);
        }
    }
}

pub fn settings_view(model: &SettingsModel) -> Element<'_, SettingsMessage> {
    let tabs_row = row![
        tab_item("General", SettingsTab::General, model.active_tab),
        tab_item("Downloads", SettingsTab::Downloads, model.active_tab),
        tab_item("Network", SettingsTab::Network, model.active_tab),
        tab_item("Appearance", SettingsTab::Appearance, model.active_tab),
    ]
    .spacing(24)
    .align_y(Alignment::Center);

    let tab_bar = container(tabs_row)
        .padding([12, 24])
        .width(Length::Fill);

    let tab_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let item_startup = setting_row(
        "Launch at startup",
        "Automatically start QDM when you log in",
        custom_switch(model.launch_at_startup, model.launch_at_startup_anim, SettingsMessage::ToggleStartup),
    );

    let item_tray = setting_row(
        "Minimize to system tray",
        "Keep running in the background when closed",
        custom_switch(model.minimize_to_tray, model.minimize_to_tray_anim, SettingsMessage::ToggleTray),
    );

    let folder_icon = icon(icons::ICON_FOLDER).size(14).color(colors::TEXT_MUTED);
    let folder_input = text_input("", &model.download_folder)
        .on_input(SettingsMessage::FolderChanged)
        .padding([6, 8])
        .width(260)
        .style(styles::transparent_text_input_style);

    let folder_box = container(
        row![folder_icon, folder_input]
            .spacing(6)
            .align_y(Alignment::Center)
    )
    .padding([0, 10])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border { color: colors::BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    });

    let browse_btn = button(text("Browse").size(13).color(colors::TEXT_PRIMARY))
        .padding([8, 16])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::BrowseFolderPressed);

    let folder_control = row![folder_box, browse_btn].spacing(8).align_y(Alignment::Center);

    let item_folder = setting_row(
        "Default download folder",
        "Where to save files",
        folder_control.into(),
    );

    let minus_btn = button(text("-").size(14).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY))
        .padding([4, 12])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::StepperDecrement);

    let count_text = text(model.simultaneous_downloads.to_string())
        .size(13)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let count_box = container(count_text)
        .width(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    let plus_btn = button(text("+").size(14).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY))
        .padding([4, 12])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::StepperIncrement);

    let stepper_control = container(
        row![minus_btn, count_box, plus_btn]
            .spacing(4)
            .align_y(Alignment::Center)
    )
    .padding([2, 4])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border { color: colors::BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    });

    let item_stepper = setting_row(
        "Simultaneous downloads",
        "Maximum active downloads at once",
        stepper_control.into(),
    );

    let item_notifications = setting_row(
        "Show notifications",
        "Alerts for completed or failed downloads",
        custom_switch(model.show_notifications, model.show_notifications_anim, SettingsMessage::ToggleNotifications),
    );

    let sounds = vec!["Default".to_string(), "Chime".to_string(), "Mute".to_string()];
    let sound_dropdown = pick_list(
        sounds,
        Some(model.notification_sound.clone()),
        SettingsMessage::SoundChanged,
    )
    .padding([8, 12])
    .width(240);

    let item_sound = setting_row(
        "Notification sound",
        "",
        sound_dropdown.into(),
    );

    let section_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let updates_header = text("UPDATES")
        .size(11)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_MUTED);

    let item_updates_toggle = setting_row(
        "Check for updates automatically",
        "",
        custom_switch(model.auto_check_updates, model.auto_check_updates_anim, SettingsMessage::ToggleUpdates),
    );

    let check_updates_btn = button(text("Check for Updates").size(13).color(colors::TEXT_PRIMARY))
        .padding([8, 16])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::CheckUpdatesPressed);

    let item_version = setting_row(
        "Current version",
        "v1.0.0",
        check_updates_btn.into(),
    );

    let form_content = column![
        item_startup,
        item_tray,
        item_folder,
        item_stepper,
        item_notifications,
        item_sound,
        Space::with_height(12),
        section_divider,
        Space::with_height(12),
        updates_header,
        item_updates_toggle,
        item_version,
    ]
    .spacing(16)
    .padding([20, 24]);

    let footer_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let reset_btn = button(text("Reset to Defaults").size(13).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(SettingsMessage::ResetDefaultsPressed);

    let save_btn = button(text("Save Changes").size(14).font(styles::BOLD_FONT).color(colors::BACKGROUND))
        .padding([10, 20])
        .style(styles::primary_button_style)
        .on_press(SettingsMessage::SaveChangesPressed);

    let footer_row = row![
        reset_btn,
        Space::with_width(Length::Fill),
        save_btn,
    ]
    .padding([16, 24])
    .align_y(Alignment::Center);

    let card_content = column![
        tab_bar,
        tab_divider,
        scrollable(form_content).height(Length::Fill),
        footer_divider,
        footer_row,
    ];

    let settings_card = container(card_content)
        .width(820)
        .style(styles::card_style);

    container(settings_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .padding([24, 32])
        .into()
}

fn custom_switch<'a>(
    _is_on: bool,
    anim_progress: f32,
    on_toggle: impl Fn(bool) -> SettingsMessage + 'a,
) -> Element<'a, SettingsMessage> {
    let track_bg = if anim_progress > 0.5 {
        colors::PRIMARY
    } else {
        colors::SURFACE_HIGH
    };

    let thumb_offset = (2.0 + anim_progress * 20.0) as u16;

    let thumb = container(Space::with_width(18))
        .width(18)
        .height(18)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::TEXT_PRIMARY)),
            border: iced::Border {
                radius: 9.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            },
            ..Default::default()
        });

    let track = container(
        row![
            Space::with_width(thumb_offset),
            thumb,
        ]
        .align_y(Alignment::Center)
    )
    .width(44)
    .height(24)
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(iced::Background::Color(track_bg)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    });

    let target_state = anim_progress <= 0.5;

    button(track)
        .style(styles::icon_button_style)
        .padding(0)
        .on_press(on_toggle(target_state))
        .into()
}

fn tab_item<'a>(
    label: &'static str,
    tab: SettingsTab,
    active_tab: SettingsTab,
) -> Element<'a, SettingsMessage> {
    let is_active = tab == active_tab;

    let label_text = text(label)
        .size(14)
        .font(if is_active { styles::BOLD_FONT } else { iced::Font::DEFAULT })
        .color(if is_active { colors::PRIMARY } else { colors::TEXT_MUTED });

    let underline = container(Space::with_height(2))
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(if is_active { colors::PRIMARY } else { iced::Color::TRANSPARENT })),
            ..Default::default()
        });

    let tab_col = column![label_text, Space::with_height(6), underline].align_x(Alignment::Center);

    button(tab_col)
        .style(styles::icon_button_style)
        .on_press(SettingsMessage::TabSelected(tab))
        .into()
}

fn setting_row<'a>(
    title: &'static str,
    description: &'static str,
    control: Element<'a, SettingsMessage>,
) -> Element<'a, SettingsMessage> {
    let title_text = text(title).size(14).font(styles::BOLD_FONT).color(colors::TEXT_PRIMARY);

    let left_col = if description.is_empty() {
        column![title_text]
    } else {
        let desc_text = text(description).size(12).color(colors::TEXT_MUTED);
        column![title_text, desc_text].spacing(2)
    };

    row![
        left_col,
        Space::with_width(Length::Fill),
        control,
    ]
    .align_y(Alignment::Center)
    .into()
}
