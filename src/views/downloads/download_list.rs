use crate::models::download::DownloadItem;
use crate::theme::{colors, styles};
use crate::views::downloads::download_item::download_item_view;
use iced::widget::{column, container, scrollable, text, Space};
use iced::{Alignment, Element, Length};

#[allow(clippy::too_many_arguments)]
pub fn download_list_view<'a, Message>(
    items: impl IntoIterator<Item = &'a DownloadItem>,
    copied_ids: &'a std::collections::HashSet<usize>,
    on_toggle_pause: impl Fn(usize) -> Message + 'a + Clone,
    on_cancel: impl Fn(usize) -> Message + 'a + Clone,
    on_open_folder: impl Fn(usize) -> Message + 'a + Clone,
    on_open_mirrors: impl Fn(usize) -> Message + 'a + Clone,
    on_open_details: impl Fn(usize) -> Message + 'a + Clone,
    on_copy_link: impl Fn(usize) -> Message + 'a + Clone,
    on_item_click: impl Fn(usize) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: 'a + Clone + 'static,
{
    let mut list_col = column![].spacing(12).width(Length::Fill);
    let mut count = 0;

    for item in items {
        count += 1;
        list_col = list_col.push(download_item_view(
            item,
            copied_ids.contains(&item.id),
            on_toggle_pause.clone(),
            on_cancel.clone(),
            on_open_folder.clone(),
            on_open_mirrors.clone(),
            on_open_details.clone(),
            on_copy_link.clone(),
            on_item_click.clone(),
        ));
    }

    if count == 0 {
        let empty_msg = container(
            column![
                text("No downloads found")
                    .size(16)
                    .font(styles::BOLD_FONT)
                    .color(colors::TEXT_PRIMARY),
                Space::with_height(4),
                text("Click '+ Add URL' to start downloading files.")
                    .size(13)
                    .color(colors::TEXT_MUTED),
            ]
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

        return empty_msg.into();
    }

    let scroll = scrollable(container(list_col).width(Length::Fill).padding([20, 24]))
        .style(styles::scrollable_style)
        .width(Length::Fill)
        .height(Length::Fill);

    scroll.into()
}
