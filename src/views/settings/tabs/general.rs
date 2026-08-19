use crate::views::settings::settings::{
    custom_switch, setting_row, SettingsMessage, SettingsModel,
};
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(model: &SettingsModel) -> Element<'a, SettingsMessage> {
    let item_startup = setting_row(
        "Launch at startup",
        "Automatically start QDM when you log in to your computer",
        custom_switch(
            model.launch_at_startup,
            model.launch_at_startup_anim,
            SettingsMessage::ToggleStartup,
        ),
    );

    let item_tray = setting_row(
        "Minimize to system tray",
        "Keep running in the background when window is closed",
        custom_switch(
            model.minimize_to_tray,
            model.minimize_to_tray_anim,
            SettingsMessage::ToggleTray,
        ),
    );

    let item_notifications = setting_row(
        "Show notifications",
        "Display desktop alerts for completed or failed downloads",
        custom_switch(
            model.show_notifications,
            model.show_notifications_anim,
            SettingsMessage::ToggleNotifications,
        ),
    );

    column![item_startup, item_tray, item_notifications,]
        .spacing(20)
        .padding([20, 24])
        .width(Length::Fill)
        .into()
}
