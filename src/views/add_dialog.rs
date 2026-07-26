use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length};
use crate::icons::{self, icon};
use crate::theme::{colors, styles};

#[derive(Debug, Clone)]
pub struct AddDialogModel {
    pub is_open: bool,
    pub url: String,
    pub filename: String,
    pub save_to: String,
    pub is_advanced_expanded: bool,
    pub max_connections: String,
    pub speed_limit: String,
}

impl Default for AddDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            url: String::new(),
            filename: String::new(),
            save_to: "C:\\Users\\Downloads".to_string(),
            is_advanced_expanded: true,
            max_connections: "8".to_string(),
            speed_limit: String::new(),
        }
    }
}

pub fn add_dialog_view<'a, Message>(
    model: &'a AddDialogModel,
    on_close: Message,
    on_url_change: impl Fn(String) -> Message + 'a,
    on_filename_change: impl Fn(String) -> Message + 'a,
    on_save_to_change: impl Fn(String) -> Message + 'a,
    on_browse_folder: Message,
    on_toggle_advanced: Message,
    on_max_connections_change: impl Fn(String) -> Message + 'a,
    on_speed_limit_change: impl Fn(String) -> Message + 'a,
    on_cancel: Message,
    on_submit: Message,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    // 1. Header
    let title_text = text("Add New Download")
        .size(18)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let close_btn = button(
        icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED)
    )
    .style(styles::icon_button_style)
    .on_press(on_close);

    let header_row = row![
        title_text,
        Space::with_width(Length::Fill),
        close_btn,
    ]
    .padding([16, 20])
    .align_y(Alignment::Center);

    let header_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    // 2. Form Inputs
    // DOWNLOAD URL
    let url_label = form_label("DOWNLOAD URL");
    let url_input = text_input("https://example.com/file.zip", &model.url)
        .on_input(on_url_change)
        .padding([10, 12])
        .width(Length::Fill)
        .style(styles::dark_input_style);

    let url_group = column![url_label, url_input].spacing(6);

    // FILENAME (with magic wand icon inside)
    let filename_label = form_label("FILENAME");
    let wand_icon = icon(icons::ICON_WAND).size(14).color(colors::PRIMARY);
    let filename_widget = text_input("file.zip", &model.filename)
        .on_input(on_filename_change)
        .padding([10, 12])
        .width(Length::Fill)
        .style(styles::transparent_text_input_style);

    let filename_box = container(
        row![filename_widget, wand_icon]
            .spacing(8)
            .align_y(Alignment::Center)
    )
    .padding([0, 10])
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border { color: colors::BORDER, width: 1.0, radius: 6.0.into() },
        ..Default::default()
    });

    let filename_group = column![filename_label, filename_box].spacing(6);

    // SAVE TO (with folder browse button)
    let save_to_label = form_label("SAVE TO");
    let save_to_input = text_input("C:\\Users\\Downloads", &model.save_to)
        .on_input(on_save_to_change)
        .padding([10, 12])
        .width(Length::Fill)
        .style(styles::dark_input_style);

    let folder_btn = button(
        icon(icons::ICON_FOLDER).size(16).color(colors::TEXT_PRIMARY)
    )
    .padding([10, 12])
    .style(styles::ghost_button_style)
    .on_press(on_browse_folder);

    let save_to_row = row![save_to_input, folder_btn].spacing(8).align_y(Alignment::Center);
    let save_to_group = column![save_to_label, save_to_row].spacing(6);

    // Collapsible Advanced Options
    let chevron_char = if model.is_advanced_expanded {
        icons::ICON_CHEVRON_UP
    } else {
        icons::ICON_CHEVRON_DOWN
    };

    let advanced_header_btn = button(
        row![
            text("Advanced Options").size(14).color(colors::TEXT_PRIMARY),
            Space::with_width(Length::Fill),
            icon(chevron_char).size(12).color(colors::TEXT_MUTED),
        ]
        .align_y(Alignment::Center)
    )
    .width(Length::Fill)
    .style(styles::icon_button_style)
    .on_press(on_toggle_advanced);

    let advanced_content: Element<Message> = if model.is_advanced_expanded {
        let max_conn_label = form_label("MAX CONNECTIONS");
        let max_conn_input = text_input("8", &model.max_connections)
            .on_input(on_max_connections_change)
            .padding([10, 12])
            .width(Length::Fill)
            .style(styles::dark_input_style);

        let max_conn_col = column![max_conn_label, max_conn_input].spacing(6).width(Length::FillPortion(1));

        let speed_limit_label = form_label("SPEED LIMIT");
        let speed_limit_input = text_input("Unlimited", &model.speed_limit)
            .on_input(on_speed_limit_change)
            .padding([10, 12])
            .width(Length::Fill)
            .style(styles::dark_input_style);

        let speed_limit_col = column![speed_limit_label, speed_limit_input].spacing(6).width(Length::FillPortion(1));

        row![max_conn_col, speed_limit_col].spacing(16).into()
    } else {
        Space::with_height(0).into()
    };

    let advanced_group = column![
        advanced_header_btn,
        Space::with_height(8),
        advanced_content,
    ];

    let form_body = column![
        url_group,
        filename_group,
        save_to_group,
        advanced_group,
    ]
    .spacing(18)
    .padding([20, 20]);

    // 3. Footer with Cancel & Start Download
    let footer_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let cancel_btn = button(text("Cancel").size(14).color(colors::TEXT_PRIMARY))
        .padding([10, 16])
        .style(styles::ghost_button_style)
        .on_press(on_cancel);

    let start_btn = button(text("Start Download").size(14).font(styles::BOLD_FONT).color(colors::BACKGROUND))
        .padding([10, 20])
        .style(styles::primary_button_style)
        .on_press(on_submit);

    let footer_row = row![
        Space::with_width(Length::Fill),
        cancel_btn,
        start_btn,
    ]
    .spacing(12)
    .padding([16, 20])
    .align_y(Alignment::Center);

    // Assembly Modal Card (520px wide)
    let modal_card = container(
        column![
            header_row,
            header_divider,
            form_body,
            footer_divider,
            footer_row,
        ]
    )
    .width(520)
    .style(styles::card_style);

    // Semi-transparent Backdrop Overlay
    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
            ..Default::default()
        })
        .into()
}

fn form_label(label: &'static str) -> text::Text<'static> {
    text(label)
        .size(11)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_MUTED)
}
