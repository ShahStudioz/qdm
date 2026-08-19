use crate::theme::{colors, styles};
use crate::views::settings::settings::{
    custom_switch, setting_row, SettingsMessage, SettingsModel,
};
use iced::widget::{button, column, text};
use iced::{Element, Length};

pub fn view<'a>(model: &SettingsModel) -> Element<'a, SettingsMessage> {
    let item_updates_toggle = setting_row(
        "Check for updates automatically",
        "Periodically check for new releases and feature updates",
        custom_switch(
            model.auto_check_updates,
            model.auto_check_updates_anim,
            SettingsMessage::ToggleUpdates,
        ),
    );

    let check_updates_btn = button(
        text("Check for Updates")
            .size(13)
            .color(colors::TEXT_PRIMARY),
    )
    .padding([8, 16])
    .style(styles::ghost_button_style)
    .on_press(SettingsMessage::CheckUpdatesPressed);

    let item_version = setting_row(
        "Current version",
        "v0.1.0 (Latest build)",
        check_updates_btn.into(),
    );

    column![item_updates_toggle, item_version,]
        .spacing(20)
        .padding([20, 24])
        .width(Length::Fill)
        .into()
}
