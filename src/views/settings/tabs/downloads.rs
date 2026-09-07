use crate::icons::{self, icon};
use crate::theme::{colors, styles};
use crate::views::settings::settings::{
    custom_switch, setting_row, stepper_widget, DeleteAction, FileConflictAction, SettingsMessage,
    SettingsModel, SpeedUnit,
};
use iced::widget::{button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length};

pub fn view<'a>(model: &SettingsModel) -> Element<'a, SettingsMessage> {
    // 1. Download Folder
    let folder_icon = icon(icons::ICON_FOLDER).size(14).color(colors::TEXT_MUTED);
    let folder_input = text_input("", &model.download_folder)
        .on_input(SettingsMessage::FolderChanged)
        .padding([6, 8])
        .width(180)
        .style(styles::transparent_text_input_style);

    let folder_box = container(
        row![folder_icon, folder_input]
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .padding([0, 10])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    let browse_btn = button(text("Browse").size(13).color(colors::TEXT_PRIMARY))
        .padding([7, 14])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::BrowseFolderPressed);

    let folder_control = row![folder_box, browse_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    let item_folder = setting_row(
        "Default download folder",
        "Where to save downloaded files on your disk",
        folder_control.into(),
    );

    // 2. File Conflict Action
    let conflict_options = vec![
        "Ask every time".to_string(),
        "Auto-rename file".to_string(),
        "Overwrite existing file".to_string(),
    ];
    let selected_conflict = match model.file_conflict_action {
        Some(FileConflictAction::AutoRename) => "Auto-rename file".to_string(),
        Some(FileConflictAction::Overwrite) => "Overwrite existing file".to_string(),
        None => "Ask every time".to_string(),
    };
    let conflict_dropdown = pick_list(
        conflict_options,
        Some(selected_conflict),
        SettingsMessage::FileConflictActionChanged,
    )
    .style(styles::pick_list_style)
    .menu_style(styles::pick_list_menu_style)
    .padding([8, 12])
    .width(200);

    let item_conflict = setting_row(
        "When file already exists",
        "Action to take if a file with the same name already exists",
        conflict_dropdown.into(),
    );

    // 3. Delete Action
    let delete_options = vec![
        "Ask every time".to_string(),
        "Remove from list only".to_string(),
        "Delete file from disk".to_string(),
    ];
    let selected_delete = match model.delete_action {
        Some(DeleteAction::RemoveFromList) => "Remove from list only".to_string(),
        Some(DeleteAction::DeleteFromDisk) => "Delete file from disk".to_string(),
        None => "Ask every time".to_string(),
    };
    let delete_dropdown = pick_list(
        delete_options,
        Some(selected_delete),
        SettingsMessage::DeleteActionChanged,
    )
    .style(styles::pick_list_style)
    .menu_style(styles::pick_list_menu_style)
    .padding([8, 12])
    .width(200);

    let item_delete = setting_row(
        "When deleting a download",
        "Action to take when delete button is clicked on a download item",
        delete_dropdown.into(),
    );

    // 4. Simultaneous Downloads
    let item_simultaneous = setting_row(
        "Simultaneous downloads",
        "Maximum number of active downloads running at the same time",
        stepper_widget(
            &model.simultaneous_downloads.to_string(),
            SettingsMessage::SimultaneousDownloadsDec,
            SettingsMessage::SimultaneousDownloadsInc,
        ),
    );

    // 5. Max Connections per Download
    let item_connections = setting_row(
        "Max connections per download",
        "Number of parallel TCP connection streams per download task",
        stepper_widget(
            &model.max_connections.to_string(),
            SettingsMessage::MaxConnectionsDec,
            SettingsMessage::MaxConnectionsInc,
        ),
    );

    // 6. Max Worker Threads
    let item_threads = setting_row(
        "Max worker threads",
        "Maximum Tokio background runtime worker threads",
        stepper_widget(
            &model.max_threads.to_string(),
            SettingsMessage::MaxThreadsDec,
            SettingsMessage::MaxThreadsInc,
        ),
    );

    // 7. Auto-Retry Failed Downloads
    let item_auto_retry = setting_row(
        "Auto-retry failed downloads",
        "Automatically retry downloads that encounter transient server errors",
        custom_switch(
            model.auto_retry_downloads,
            model.auto_retry_downloads_anim,
            SettingsMessage::ToggleAutoRetry,
        ),
    );

    // 8. Max Auto-Retries
    let item_max_retries = setting_row(
        "Max auto-retries",
        "Number of automatic retry attempts before marking download as failed",
        stepper_widget(
            &model.max_auto_retries.to_string(),
            SettingsMessage::MaxAutoRetriesDec,
            SettingsMessage::MaxAutoRetriesInc,
        ),
    );

    // 9. Speed Limit with Unit Dropdown
    let speed_input = text_input("Unlimited", &model.speed_limit_value)
        .on_input(SettingsMessage::SpeedLimitValueChanged)
        .padding([8, 10])
        .width(110)
        .style(styles::dark_input_style);

    let unit_dropdown = pick_list(
        SpeedUnit::ALL,
        Some(model.speed_limit_unit),
        SettingsMessage::SpeedLimitUnitChanged,
    )
    .style(styles::pick_list_style)
    .menu_style(styles::pick_list_menu_style)
    .padding([8, 10])
    .width(85);

    let speed_control = row![speed_input, unit_dropdown]
        .spacing(8)
        .align_y(Alignment::Center);

    let item_speed_limit = setting_row(
        "Global download speed limit",
        "Maximum speed applied per download (leave empty for unlimited)",
        speed_control.into(),
    );

    column![
        item_folder,
        item_conflict,
        item_delete,
        item_simultaneous,
        item_connections,
        item_threads,
        item_auto_retry,
        item_max_retries,
        item_speed_limit,
    ]
    .spacing(18)
    .padding([20, 24])
    .width(Length::Fill)
    .into()
}
