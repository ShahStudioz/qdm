use crate::icons::{self, icon};
use crate::theme::{colors, styles};
use iced::widget::{button, checkbox, column, container, row, text, Space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub struct DeletePendingItem {
    pub id: usize,
    pub filename: String,
    pub save_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct DeleteDialogModel {
    pub is_open: bool,
    pub pending: Option<DeletePendingItem>,
    pub remember_choice: bool,
}

impl DeleteDialogModel {
    pub fn open(&mut self, pending: DeletePendingItem) {
        self.is_open = true;
        self.pending = Some(pending);
        self.remember_choice = false;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.pending = None;
        self.remember_choice = false;
    }
}

#[derive(Debug, Clone)]
pub enum DeleteDialogMessage {
    Close,
    ToggleRemember(bool),
    RemoveFromListChosen,
    DeleteFromDiskChosen,
}

pub fn view(state: &DeleteDialogModel) -> Element<'_, DeleteDialogMessage> {
    let raw_filename = state
        .pending
        .as_ref()
        .map(|p| p.filename.as_str())
        .unwrap_or("this download");
    let filename = crate::models::download::truncate_filename(raw_filename, 48);

    let title_text = text("Delete Download")
        .size(17)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let close_btn = button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(DeleteDialogMessage::Close);

    let header_row = row![
        icon(icons::ICON_TRASH).size(18).color(colors::ERROR),
        title_text,
        Space::with_width(Length::Fill),
        close_btn,
    ]
    .spacing(10)
    .padding([16, 20])
    .align_y(Alignment::Center);

    let header_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let message_text = column![
        text(format!("Are you sure you want to delete \"{}\"?", filename))
            .size(13)
            .color(colors::TEXT_PRIMARY),
        Space::with_height(2),
        text("You can remove it from QDM's download list or permanently delete the file from your disk.")
            .size(12)
            .color(colors::TEXT_MUTED),
    ]
    .spacing(4);

    let remember_chk = checkbox(
        "Remember my choice for future deletions",
        state.remember_choice,
    )
    .on_toggle(DeleteDialogMessage::ToggleRemember)
    .size(16)
    .text_size(13)
    .style(|_theme, _status| checkbox::Style {
        background: iced::Background::Color(colors::SURFACE_HIGH),
        icon_color: colors::PRIMARY,
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 4.0.into(),
        },
        text_color: Some(colors::TEXT_PRIMARY),
    });

    let cancel_btn = button(text("Cancel").size(13).color(colors::TEXT_MUTED))
        .padding([8, 14])
        .style(styles::ghost_button_style)
        .on_press(DeleteDialogMessage::Close);

    let remove_list_btn = button(
        row![
            icon(icons::ICON_CANCEL)
                .size(14)
                .color(colors::TEXT_PRIMARY),
            text("Remove from List")
                .size(13)
                .color(colors::TEXT_PRIMARY),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .style(styles::ghost_button_style)
    .on_press(DeleteDialogMessage::RemoveFromListChosen);

    let delete_disk_btn = button(
        row![
            icon(icons::ICON_TRASH).size(14).color(colors::BACKGROUND),
            text("Delete File from Disk")
                .size(13)
                .font(styles::BOLD_FONT)
                .color(colors::BACKGROUND),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(|_theme, status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => {
                iced::Color::from_rgb(0.95, 0.35, 0.35)
            }
            _ => colors::ERROR,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: colors::BACKGROUND,
            border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow::default(),
        }
    })
    .on_press(DeleteDialogMessage::DeleteFromDiskChosen);

    let buttons_row = row![
        cancel_btn,
        Space::with_width(Length::Fill),
        remove_list_btn,
        delete_disk_btn,
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let content_col = column![
        message_text,
        Space::with_height(10),
        remember_chk,
        Space::with_height(14),
        buttons_row,
    ]
    .spacing(8)
    .padding([20, 20]);

    let modal_card = container(column![header_row, header_divider, content_col])
        .width(540)
        .style(styles::card_style);

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(styles::modal_backdrop_style)
        .into()
}
