use crate::core::utils::paths;
pub use crate::core::utils::paths::sanitize_filename;
use crate::icons::{self, icon};
use crate::models::download::format_bytes;
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, pick_list, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Task};

use crate::views::settings::settings::SpeedUnit;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddDialogStep {
    UrlInput,
    FetchingInfo,
    DownloadDetails,
}

#[derive(Debug, Clone)]
pub struct AddDialogModel {
    pub is_open: bool,
    pub step: AddDialogStep,
    pub url: String,
    pub filename: String,
    pub save_to: String,
    pub is_advanced_expanded: bool,
    pub max_connections: String,
    pub speed_limit: String,
    pub speed_unit: SpeedUnit,
    pub mirror_urls_text: String,
    pub engine: Option<crate::services::engine::AppEngine>,
    pub has_error: bool,
    pub error: String,
    pub probe_result: Option<crate::services::engine::ProbeResult>,
    pub selected_files: std::collections::HashSet<usize>,
}

impl Default for AddDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            step: AddDialogStep::UrlInput,
            url: String::new(),
            filename: String::new(),
            save_to: paths::get_default_download_dir(),
            is_advanced_expanded: false,
            max_connections: "8".to_string(),
            speed_limit: String::new(),
            speed_unit: SpeedUnit::KBps,
            mirror_urls_text: String::new(),
            engine: None,
            has_error: false,
            error: String::new(),
            probe_result: None,
            selected_files: std::collections::HashSet::new(),
        }
    }
}

impl AddDialogModel {
    pub fn reset(&mut self) {
        self.step = AddDialogStep::UrlInput;
        self.url.clear();
        self.filename.clear();
        self.save_to = paths::get_default_download_dir();
        self.has_error = false;
        self.error.clear();
        self.probe_result = None;
        self.selected_files.clear();
        self.is_advanced_expanded = false;
        self.mirror_urls_text.clear();
    }

    pub fn parsed_mirrors(&self) -> Vec<String> {
        self.mirror_urls_text
            .lines()
            .flat_map(|line| line.split(','))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && (s.starts_with("http://") || s.starts_with("https://")))
            .collect()
    }
}

pub fn view(state: &AddDialogModel) -> Element<'_, AddDialogueModalMessage> {
    let title_text = text(match state.step {
        AddDialogStep::DownloadDetails => "Download Details",
        _ => "Add New Download",
    })
    .size(18)
    .font(styles::BOLD_FONT)
    .color(colors::TEXT_PRIMARY);

    let close_btn = button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(AddDialogueModalMessage::CloseAddDialog);

    let header_row = row![title_text, Space::with_width(Length::Fill), close_btn]
        .padding([16, 20])
        .align_y(Alignment::Center);

    let header_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let error_view: Element<AddDialogueModalMessage> = if state.has_error && !state.error.is_empty()
    {
        let err_row = row![
            icon(icons::ICON_WARN).size(14).color(colors::ERROR),
            text(&state.error).color(colors::ERROR).size(12),
        ]
        .spacing(8)
        .align_y(Alignment::Center);

        let add_anyway_btn: Element<AddDialogueModalMessage> = if !state.url.is_empty() {
            button(
                row![
                    icon(icons::ICON_PLUS).size(12).color(colors::ERROR),
                    text("Add Anyway (Fetch in Background)")
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::ERROR),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([4, 8])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::QuickAddDownload)
            .into()
        } else {
            Space::with_height(0).into()
        };

        container(
            column![
                err_row,
                Space::with_height(4),
                row![Space::with_width(Length::Fill), add_anyway_btn].align_y(Alignment::Center),
            ]
            .spacing(2),
        )
        .padding([10, 14])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.93, 0.26, 0.26, 0.12,
            ))),
            border: iced::Border {
                color: colors::ERROR,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        })
        .into()
    } else {
        Space::with_height(0).into()
    };

    let form_body: Element<AddDialogueModalMessage> = match state.step {
        AddDialogStep::UrlInput | AddDialogStep::FetchingInfo => {
            let url_label = form_label("DOWNLOAD URL");

            let paste_btn = button(
                row![
                    icon(icons::ICON_PASTE).size(13).color(colors::PRIMARY),
                    text("Paste").size(12).color(colors::PRIMARY),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([4, 8])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::PasteFromClipboard);

            let browse_torrent_btn = button(
                row![
                    icon(icons::ICON_FOLDER).size(13).color(colors::TORRENT),
                    text(".torrent")
                        .size(12)
                        .font(styles::BOLD_FONT)
                        .color(colors::TORRENT),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::AddBrowseTorrentPressed);

            let url_widget = text_input("https://... or magnet: or .torrent file", &state.url)
                .on_input(AddDialogueModalMessage::AddUrlChanged)
                .on_submit(AddDialogueModalMessage::FetchFileInfoPressed)
                .padding([10, 10])
                .width(Length::Fill)
                .style(styles::transparent_text_input_style);

            let url_box = container(
                row![paste_btn, url_widget]
                    .spacing(4)
                    .align_y(Alignment::Center),
            )
            .padding([0, 6])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                border: iced::Border {
                    color: if state.has_error {
                        colors::ERROR
                    } else {
                        colors::BORDER
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            let url_input_row = row![url_box, browse_torrent_btn]
                .spacing(8)
                .align_y(Alignment::Center);

            let url_group = column![url_label, url_input_row].spacing(6);

            let info_note = text("Tip: You can Inspect to configure details, or click 'Quick Add' to start immediately while metadata loads in background.")
                .size(11)
                .color(colors::TEXT_MUTED);

            column![error_view, url_group, info_note]
                .spacing(14)
                .padding([20, 20])
                .into()
        }
        AddDialogStep::DownloadDetails => {
            let metadata_summary: Element<AddDialogueModalMessage> =
                if let Some(ref probe) = state.probe_result {
                    match probe {
                        crate::services::engine::ProbeResult::Http(meta) => {
                            let size_str = meta
                                .content_length
                                .map(format_bytes)
                                .unwrap_or_else(|| "Unknown size".to_string());

                            let resume_str = if meta.supports_resume {
                                "Resume: Supported"
                            } else {
                                "Resume: Not supported"
                            };

                            let type_str = meta.content_type.as_deref().unwrap_or("Unknown type");

                            container(
                                row![
                                    text(format!("Size: {}", size_str))
                                        .size(12)
                                        .font(styles::BOLD_FONT)
                                        .color(colors::TEXT_PRIMARY),
                                    text("|").size(12).color(colors::TEXT_MUTED),
                                    text(resume_str).size(12).color(if meta.supports_resume {
                                        colors::SUCCESS
                                    } else {
                                        colors::TEXT_MUTED
                                    }),
                                    text("|").size(12).color(colors::TEXT_MUTED),
                                    text(type_str).size(12).color(colors::TEXT_MUTED),
                                ]
                                .spacing(10)
                                .align_y(Alignment::Center),
                            )
                            .padding([8, 12])
                            .width(Length::Fill)
                            .style(|_| container::Style {
                                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                                border: iced::Border {
                                    color: colors::BORDER,
                                    width: 1.0,
                                    radius: 6.0.into(),
                                },
                                ..Default::default()
                            })
                            .into()
                        }
                        crate::services::engine::ProbeResult::Torrent(info) => {
                            let total_size: u64 = info.files.iter().map(|f| f.size).sum();
                            let file_count = info.files.len();
                            let selected_count = state.selected_files.len();

                            let mut file_list = column![].spacing(4);
                            for file in &info.files {
                                let is_selected = state.selected_files.contains(&file.id);
                                let file_row = row![
                                    iced::widget::checkbox("", is_selected).on_toggle({
                                        let id = file.id;
                                        move |_| AddDialogueModalMessage::ToggleFileSelection(id)
                                    }),
                                    text(&file.path)
                                        .size(12)
                                        .color(colors::TEXT_PRIMARY)
                                        .width(Length::Fill),
                                    text(format_bytes(file.size))
                                        .size(11)
                                        .color(colors::TEXT_MUTED),
                                ]
                                .spacing(8)
                                .align_y(Alignment::Center);
                                file_list = file_list.push(file_row);
                            }

                            let scrollable_files = iced::widget::scrollable(file_list)
                                .height(Length::Fixed(150.0))
                                .width(Length::Fill);

                            container(column![
                                row![
                                    text(format!(
                                        "Torrent: {} files ({} selected)",
                                        file_count, selected_count
                                    ))
                                    .size(12)
                                    .font(styles::BOLD_FONT)
                                    .color(colors::TEXT_PRIMARY),
                                    Space::with_width(Length::Fill),
                                    text(format!("Total: {}", format_bytes(total_size)))
                                        .size(12)
                                        .color(colors::TEXT_MUTED),
                                ]
                                .align_y(Alignment::Center),
                                Space::with_height(8),
                                scrollable_files
                            ])
                            .padding([8, 12])
                            .width(Length::Fill)
                            .style(|_| container::Style {
                                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                                border: iced::Border {
                                    color: colors::BORDER,
                                    width: 1.0,
                                    radius: 6.0.into(),
                                },
                                ..Default::default()
                            })
                            .into()
                        }
                    }
                } else {
                    Space::with_height(0).into()
                };

            let url_label = form_label("DOWNLOAD URL");
            let url_input = text_input("", &state.url)
                .on_input(AddDialogueModalMessage::AddUrlChanged)
                .padding([8, 10])
                .width(Length::Fill)
                .style(styles::dark_input_style);
            let url_group = column![url_label, url_input].spacing(4);

            let is_torrent_folder = matches!(
                state.probe_result,
                Some(crate::services::engine::ProbeResult::Torrent(ref info)) if info.is_folder
            );
            let filename_label = form_label(if is_torrent_folder {
                "FOLDER NAME"
            } else {
                "FILENAME"
            });
            let wand_icon = icon(icons::ICON_WAND).size(14).color(colors::PRIMARY);
            let filename_widget = text_input(
                if is_torrent_folder {
                    "Folder Name"
                } else {
                    "file.zip"
                },
                &state.filename,
            )
            .on_input(AddDialogueModalMessage::AddFilenameChanged)
            .padding([10, 12])
            .width(Length::Fill)
            .style(styles::transparent_text_input_style);

            let filename_box = container(
                row![filename_widget, wand_icon]
                    .spacing(8)
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
            let filename_group = column![filename_label, filename_box].spacing(6);

            let has_conflict = if is_torrent_folder {
                crate::core::utils::paths::folder_exists(&state.save_to, &state.filename)
            } else {
                crate::core::utils::paths::file_exists_or_downloading(
                    &state.save_to,
                    &state.filename,
                )
            };
            let collision_warning_text = if is_torrent_folder {
                "Folder with this name already exists in destination."
            } else {
                "File with this name already exists in destination."
            };
            let collision_warning: Element<AddDialogueModalMessage> =
                if has_conflict && !state.filename.trim().is_empty() {
                    container(
                        row![
                            icon(icons::ICON_WARN).size(14).color(colors::WARNING),
                            text(collision_warning_text)
                                .size(12)
                                .color(colors::WARNING)
                                .width(Length::Fill),
                            button(
                                text("Auto-Rename")
                                    .size(11)
                                    .font(styles::BOLD_FONT)
                                    .color(colors::TEXT_PRIMARY)
                            )
                            .padding([4, 10])
                            .style(styles::ghost_button_style)
                            .on_press(AddDialogueModalMessage::AutoRenameFilename),
                        ]
                        .spacing(10)
                        .align_y(Alignment::Center),
                    )
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(iced::Background::Color(iced::Color::from_rgba(
                            0.95, 0.70, 0.20, 0.12,
                        ))),
                        border: iced::Border {
                            color: colors::WARNING,
                            width: 1.0,
                            radius: 6.0.into(),
                        },
                        ..Default::default()
                    })
                    .into()
                } else {
                    Space::with_height(0).into()
                };

            let save_to_label = form_label("SAVE TO");
            let save_to_input = text_input(&paths::get_default_download_dir(), &state.save_to)
                .on_input(AddDialogueModalMessage::AddSaveToChanged)
                .padding([10, 12])
                .width(Length::Fill)
                .style(styles::dark_input_style);

            let folder_btn = button(
                icon(icons::ICON_FOLDER)
                    .size(16)
                    .color(colors::TEXT_PRIMARY),
            )
            .padding([10, 12])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::AddBrowseFolderPressed);

            let save_to_row = row![save_to_input, folder_btn]
                .spacing(8)
                .align_y(Alignment::Center);
            let save_to_group = column![save_to_label, save_to_row].spacing(6);

            let chevron_char = if state.is_advanced_expanded {
                icons::ICON_CHEVRON_UP
            } else {
                icons::ICON_CHEVRON_DOWN
            };

            let is_torrent = match &state.probe_result {
                Some(crate::services::engine::ProbeResult::Torrent(_)) => true,
                _ => state.url.starts_with("magnet:") || state.url.ends_with(".torrent"),
            };

            let advanced_header_text = if is_torrent {
                "Advanced Options"
            } else {
                "Advanced Options & Mirrors"
            };

            let advanced_header_btn = button(
                row![
                    text(advanced_header_text)
                        .size(13)
                        .color(colors::TEXT_PRIMARY),
                    Space::with_width(Length::Fill),
                    icon(chevron_char).size(12).color(colors::TEXT_MUTED),
                ]
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .style(styles::icon_button_style)
            .on_press(AddDialogueModalMessage::ToggleAdvancedOptions);

            let advanced_content: Element<AddDialogueModalMessage> = if state.is_advanced_expanded {
                let speed_limit_label = form_label("SPEED LIMIT");
                let speed_limit_input = text_input("Unlimited", &state.speed_limit)
                    .on_input(AddDialogueModalMessage::AddSpeedLimitChanged)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(styles::dark_input_style);

                let speed_unit_dropdown = pick_list(
                    SpeedUnit::ALL,
                    Some(state.speed_unit),
                    AddDialogueModalMessage::AddSpeedUnitChanged,
                )
                .style(styles::pick_list_style)
                .menu_style(styles::pick_list_menu_style)
                .padding([10, 12])
                .width(85);

                let speed_control_row = row![speed_limit_input, speed_unit_dropdown]
                    .spacing(6)
                    .align_y(Alignment::Center);

                let speed_limit_col = column![speed_limit_label, speed_control_row]
                    .spacing(6)
                    .width(Length::FillPortion(1));

                if is_torrent {
                    column![speed_limit_col].spacing(6).into()
                } else {
                    let max_conn_label = form_label("MAX CONNECTIONS");
                    let max_conn_input = text_input("8", &state.max_connections)
                        .on_input(AddDialogueModalMessage::AddMaxConnectionsChanged)
                        .padding([10, 12])
                        .width(Length::Fill)
                        .style(styles::dark_input_style);

                    let max_conn_col = column![max_conn_label, max_conn_input]
                        .spacing(6)
                        .width(Length::FillPortion(1));

                    let conn_speed_row = row![max_conn_col, speed_limit_col].spacing(16);

                    let mirrors_label =
                        form_label("OPTIONAL MIRROR LINKS (COMMA OR NEWLINE SEPARATED)");
                    let mirrors_input = text_input(
                        "https://mirror1.example.com/file.zip, https://mirror2...",
                        &state.mirror_urls_text,
                    )
                    .on_input(AddDialogueModalMessage::AddMirrorUrlsChanged)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(styles::dark_input_style);

                    column![
                        conn_speed_row,
                        Space::with_height(6),
                        mirrors_label,
                        mirrors_input
                    ]
                    .spacing(6)
                    .into()
                }
            } else {
                Space::with_height(0).into()
            };

            let advanced_group =
                column![advanced_header_btn, Space::with_height(6), advanced_content];

            column![
                error_view,
                metadata_summary,
                url_group,
                filename_group,
                collision_warning,
                save_to_group,
                advanced_group
            ]
            .spacing(14)
            .padding([16, 20])
            .into()
        }
    };

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
        .on_press(AddDialogueModalMessage::CloseAddDialog);

    let footer_buttons: Element<AddDialogueModalMessage> = match state.step {
        AddDialogStep::UrlInput => {
            let quick_add_btn = button(
                row![
                    icon(icons::ICON_PLUS).size(13).color(colors::TEXT_PRIMARY),
                    text("Quick Add").size(14).color(colors::TEXT_PRIMARY),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::QuickAddDownload);

            let inspect_btn = button(
                row![
                    icon(icons::ICON_SEARCH).size(13).color(colors::BACKGROUND),
                    text("Inspect & Configure")
                        .size(14)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([10, 20])
            .style(styles::primary_button_style)
            .on_press(AddDialogueModalMessage::FetchFileInfoPressed);

            row![
                Space::with_width(Length::Fill),
                cancel_btn,
                quick_add_btn,
                inspect_btn
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }

        AddDialogStep::FetchingInfo => {
            let add_anyway_btn = button(
                row![text("Add Without Waiting")
                    .size(13)
                    .color(colors::TEXT_PRIMARY),]
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::QuickAddDownload);

            let fetching_badge = button(
                row![
                    icon(icons::ICON_SPINNER).size(13).color(colors::TEXT_MUTED),
                    text("Fetching Info...").size(14).color(colors::TEXT_MUTED),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .style(styles::ghost_button_style);

            row![
                Space::with_width(Length::Fill),
                cancel_btn,
                add_anyway_btn,
                fetching_badge
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }

        AddDialogStep::DownloadDetails => {
            let schedule_btn = button(
                row![
                    icon(icons::ICON_SCHEDULED)
                        .size(13)
                        .color(colors::TEXT_PRIMARY),
                    text("Schedule for Later")
                        .size(13)
                        .color(colors::TEXT_PRIMARY),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .style(styles::ghost_button_style)
            .on_press(AddDialogueModalMessage::ScheduleNewDownload);

            let start_btn = button(
                row![
                    icon(icons::ICON_DOWNLOADING)
                        .size(14)
                        .color(colors::BACKGROUND),
                    text("Start Download")
                        .size(14)
                        .font(styles::BOLD_FONT)
                        .color(colors::BACKGROUND),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([10, 20])
            .style(styles::primary_button_style)
            .on_press(AddDialogueModalMessage::SubmitNewDownload);

            row![
                Space::with_width(Length::Fill),
                cancel_btn,
                schedule_btn,
                start_btn
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }
    };

    let footer_row = container(footer_buttons)
        .padding([16, 20])
        .width(Length::Fill);

    let modal_card = container(column![
        header_row,
        header_divider,
        form_body,
        footer_divider,
        footer_row,
    ])
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

fn form_label(label: &'static str) -> text::Text<'static> {
    text(label)
        .size(11)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_MUTED)
}

#[derive(Debug, Clone)]
pub enum AddDialogueModalMessage {
    CloseAddDialog,
    PasteFromClipboard,
    ClipboardContentRead(Option<String>),
    AddBrowseTorrentPressed,
    TorrentFilePicked(Option<String>),
    AddUrlChanged(String),
    FetchFileInfoPressed,
    FileMetaDataFetched(Result<crate::services::engine::ProbeResult, String>),
    ToggleFileSelection(usize),
    AddFilenameChanged(String),
    AutoRenameFilename,
    AddSaveToChanged(String),
    AddBrowseFolderPressed,
    FolderPicked(Option<String>),
    ToggleAdvancedOptions,
    AddMaxConnectionsChanged(String),
    AddSpeedLimitChanged(String),
    AddSpeedUnitChanged(SpeedUnit),
    AddMirrorUrlsChanged(String),
    QuickAddDownload,
    SubmitNewDownload,
    ScheduleNewDownload,
}

pub fn update(
    state: &mut AddDialogModel,
    message: AddDialogueModalMessage,
) -> Task<AddDialogueModalMessage> {
    match message {
        AddDialogueModalMessage::CloseAddDialog => {
            state.is_open = false;
            state.reset();
        }
        AddDialogueModalMessage::PasteFromClipboard => {
            return iced::clipboard::read().map(AddDialogueModalMessage::ClipboardContentRead);
        }
        AddDialogueModalMessage::ClipboardContentRead(content) => {
            if let Some(text_val) = content {
                let trimmed = text_val.trim().to_string();
                if is_valid_url_or_magnet(&trimmed) {
                    state.url = trimmed;
                    state.has_error = false;
                    state.error.clear();
                }
            }
        }
        AddDialogueModalMessage::AddBrowseTorrentPressed => {
            return Task::perform(
                async {
                    let file = rfd::AsyncFileDialog::new()
                        .set_title("Select .torrent File")
                        .add_filter("Torrent Files", &["torrent"])
                        .pick_file()
                        .await;
                    file.map(|f| f.path().to_string_lossy().to_string())
                },
                AddDialogueModalMessage::TorrentFilePicked,
            );
        }
        AddDialogueModalMessage::TorrentFilePicked(path_opt) => {
            if let Some(path) = path_opt {
                state.url = path;
                state.has_error = false;
                state.error.clear();
                return update(state, AddDialogueModalMessage::FetchFileInfoPressed);
            }
        }
        AddDialogueModalMessage::AddUrlChanged(url) => {
            state.url = url;
            state.has_error = false;
            state.error.clear();
        }
        AddDialogueModalMessage::FetchFileInfoPressed => {
            let url = state.url.trim().to_string();
            let is_torrent = crate::core::utils::paths::is_torrent_target(&url);
            if url.is_empty() {
                state.has_error = true;
                state.error =
                    "Please enter a valid download URL or select a .torrent file".to_string();
            } else if !url.starts_with("http://")
                && !url.starts_with("https://")
                && !url.starts_with("magnet:")
                && !is_torrent
            {
                state.has_error = true;
                state.error =
                    "URL must start with http://, https://, magnet:, or be a local .torrent file"
                        .to_string();
            } else {
                state.step = AddDialogStep::FetchingInfo;
                state.has_error = false;
                state.error.clear();

                if let Some(engine) = state.engine.clone() {
                    let task = async move { engine.probe_metadata(&url).await };
                    return Task::perform(task, AddDialogueModalMessage::FileMetaDataFetched);
                }
            }
        }
        AddDialogueModalMessage::FileMetaDataFetched(meta_res) => match meta_res {
            Ok(data) => {
                match &data {
                    crate::services::engine::ProbeResult::Http(http_meta) => {
                        let extracted_name =
                            extract_filename(&state.url, http_meta.content_disposition.as_deref());
                        state.filename = extracted_name;
                    }
                    crate::services::engine::ProbeResult::Torrent(info) => {
                        state.filename = info.name.clone();
                        state.selected_files = info.files.iter().map(|f| f.id).collect();
                    }
                }
                state.probe_result = Some(data);
                state.step = AddDialogStep::DownloadDetails;
                state.has_error = false;
                state.error.clear();
            }
            Err(err) => {
                state.step = AddDialogStep::UrlInput;
                state.has_error = true;
                state.error = err;
            }
        },
        AddDialogueModalMessage::AddFilenameChanged(filename) => {
            state.filename = filename;
        }
        AddDialogueModalMessage::AutoRenameFilename => {
            state.filename = paths::generate_unique_filename(&state.save_to, &state.filename);
        }
        AddDialogueModalMessage::AddSaveToChanged(save_to) => {
            state.save_to = save_to;
        }
        AddDialogueModalMessage::AddBrowseFolderPressed => {
            let current_save_to = state.save_to.clone();
            let task = async move {
                let mut dialog = rfd::AsyncFileDialog::new().set_title("Select Download Folder");
                if std::path::Path::new(&current_save_to).exists() {
                    dialog = dialog.set_directory(&current_save_to);
                }
                dialog
                    .pick_folder()
                    .await
                    .map(|folder| folder.path().to_string_lossy().to_string())
            };
            return Task::perform(task, AddDialogueModalMessage::FolderPicked);
        }
        AddDialogueModalMessage::FolderPicked(Some(path)) => {
            state.save_to = path;
        }
        AddDialogueModalMessage::FolderPicked(None) => {}
        AddDialogueModalMessage::ToggleAdvancedOptions => {
            state.is_advanced_expanded = !state.is_advanced_expanded;
        }
        AddDialogueModalMessage::AddMaxConnectionsChanged(val) => {
            state.max_connections = val;
        }
        AddDialogueModalMessage::AddSpeedLimitChanged(val) => {
            state.speed_limit = val.chars().filter(|c| c.is_ascii_digit()).collect();
        }
        AddDialogueModalMessage::AddSpeedUnitChanged(val) => {
            state.speed_unit = val;
        }
        AddDialogueModalMessage::AddMirrorUrlsChanged(val) => {
            state.mirror_urls_text = val;
        }
        AddDialogueModalMessage::ToggleFileSelection(id) => {
            if state.selected_files.contains(&id) {
                state.selected_files.remove(&id);
            } else {
                state.selected_files.insert(id);
            }
        }
        AddDialogueModalMessage::QuickAddDownload
        | AddDialogueModalMessage::SubmitNewDownload
        | AddDialogueModalMessage::ScheduleNewDownload => {
            // Final submission is handled at app level
        }
    }
    Task::none()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode_bytes(input: &str) -> Vec<u8> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h1), Some(h2)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                decoded.push((h1 << 4) | h2);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    decoded
}

/// Decodes percent-encoded strings (e.g. `%20` -> ` `, `%E2%9C%93` -> `✓`).
pub fn percent_decode(input: &str) -> String {
    let bytes = percent_decode_bytes(input);
    String::from_utf8(bytes.clone())
        .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned())
}

/// Unquotes an HTTP header parameter value, unescaping `\"` and `\\` if enclosed in double quotes.
fn unquote_header_value(val: &str) -> String {
    let trimmed = val.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        let inner = &trimmed[1..trimmed.len() - 1];
        let mut unescaped = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(next_c) = chars.next() {
                    unescaped.push(next_c);
                } else {
                    unescaped.push('\\');
                }
            } else {
                unescaped.push(c);
            }
        }
        unescaped
    } else {
        trimmed
            .trim_matches(|c| c == '\'' || c == '"' || c == ';')
            .to_string()
    }
}

/// Parses an RFC 6266 / RFC 5987 Content-Disposition header.
/// - Splits parameters by `;` outside quoted strings.
/// - Prioritizes RFC 5987 `filename*` over `filename` per RFC 6266 §4.3.
/// - Supports UTF-8 and ISO-8859-1 charsets with percent-decoding.
pub fn parse_content_disposition(cd: &str) -> Option<String> {
    let mut regular_filename: Option<String> = None;
    let mut ext_filename: Option<String> = None;

    // Tokenize parameters by splitting on ';' while respecting quoted strings
    let mut params = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut escape = false;

    for ch in cd.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' && in_quotes {
            escape = true;
            current.push(ch);
            continue;
        }
        if ch == '"' {
            in_quotes = !in_quotes;
            current.push(ch);
        } else if ch == ';' && !in_quotes {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                params.push(trimmed);
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        params.push(trimmed);
    }

    for param in params {
        if let Some((key, val)) = param.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let val = val.trim();

            if key == "filename*" {
                // RFC 5987 format: [charset]'[language]'encoded_value
                // e.g. UTF-8''Win11_25H2_English_x64_v2.iso
                let raw_val = val.trim_matches('"');
                let parts: Vec<&str> = raw_val.splitn(3, '\'').collect();
                if parts.len() == 3 {
                    let charset = parts[0];
                    let encoded = parts[2];
                    let decoded_bytes = percent_decode_bytes(encoded);
                    let name = if charset.eq_ignore_ascii_case("iso-8859-1")
                        || charset.eq_ignore_ascii_case("latin1")
                    {
                        decoded_bytes
                            .into_iter()
                            .map(|b| b as char)
                            .collect::<String>()
                    } else {
                        String::from_utf8(decoded_bytes.clone()).unwrap_or_else(|_| {
                            String::from_utf8_lossy(&decoded_bytes).into_owned()
                        })
                    };
                    let trimmed_name = name.trim();
                    if !trimmed_name.is_empty() {
                        ext_filename = Some(trimmed_name.to_string());
                    }
                } else {
                    let decoded = percent_decode(raw_val);
                    let trimmed_name = decoded.trim();
                    if !trimmed_name.is_empty() {
                        ext_filename = Some(trimmed_name.to_string());
                    }
                }
            } else if key == "filename" {
                let unquoted = unquote_header_value(val);
                let trimmed_name = unquoted.trim();
                if !trimmed_name.is_empty() {
                    regular_filename = Some(trimmed_name.to_string());
                }
            }
        }
    }

    // RFC 6266 §4.3: filename* takes precedence over filename
    ext_filename.or(regular_filename)
}

pub fn extract_filename(url_str: &str, content_disposition: Option<&str>) -> String {
    if let Some(cd) = content_disposition {
        if let Some(parsed) = parse_content_disposition(cd) {
            let sanitized = sanitize_filename(&parsed);
            if sanitized != "download.file" && !sanitized.is_empty() {
                return sanitized;
            }
        }
    }

    if let Ok(url) = reqwest::Url::parse(url_str) {
        if url.scheme() == "magnet" {
            for (k, v) in url.query_pairs() {
                if k == "dn" && !v.trim().is_empty() {
                    return sanitize_filename(v.trim());
                }
            }
        }
        for (k, v) in url.query_pairs() {
            let k_lower = k.to_lowercase();
            if (k_lower == "filename" || k_lower == "file" || k_lower == "name")
                && !v.trim().is_empty()
            {
                return sanitize_filename(v.trim());
            }
        }
        if let Some(mut segments) = url.path_segments() {
            if let Some(last) = segments.next_back() {
                let decoded = percent_decode(last.trim());
                let sanitized = sanitize_filename(&decoded);
                if sanitized != "download.file" && !sanitized.is_empty() {
                    return sanitized;
                }
            }
        }
    } else {
        let path = url_str.split('?').next().unwrap_or(url_str);
        if let Some(last) = path.split('/').next_back() {
            let decoded = percent_decode(last.trim());
            let sanitized = sanitize_filename(&decoded);
            if sanitized != "download.file" && !sanitized.is_empty() {
                return sanitized;
            }
        }
    }

    if url_str.starts_with("magnet:") {
        "Torrent Download".to_string()
    } else {
        "download.file".to_string()
    }
}

/// Checks if a string looks like a valid downloadable URL, magnet link, or .torrent path.
pub fn is_valid_url_or_magnet(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("magnet:?")
        || lower.ends_with(".torrent")
        || (std::path::Path::new(s).is_file() && lower.ends_with(".torrent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename_http_path() {
        assert_eq!(
            extract_filename("https://example.com/files/document.pdf", None),
            "document.pdf"
        );
    }

    #[test]
    fn test_extract_filename_magnet_dn() {
        assert_eq!(
            extract_filename(
                "magnet:?xt=urn:btih:c12fe1c06bba254a70f64330b7a80f92ae54cb8b&dn=Big+Buck+Bunny.mp4",
                None
            ),
            "Big Buck Bunny.mp4"
        );
    }

    #[test]
    fn test_extract_filename_magnet_no_dn() {
        assert_eq!(
            extract_filename(
                "magnet:?xt=urn:btih:c12fe1c06bba254a70f64330b7a80f92ae54cb8b",
                None
            ),
            "Torrent Download"
        );
    }

    #[test]
    fn test_extract_filename_query_param() {
        assert_eq!(
            extract_filename("https://example.com/api?file=report.xlsx", None),
            "report.xlsx"
        );
    }

    #[test]
    fn test_extract_filename_content_disposition() {
        assert_eq!(
            extract_filename(
                "https://example.com/download",
                Some("attachment; filename=\"custom_name.zip\"")
            ),
            "custom_name.zip"
        );
    }

    #[test]
    fn test_extract_filename_microsoft_win11_dual_param() {
        // Reproduces the exact Microsoft Azure CDN header
        let cd_quoted = "attachment; filename=\"Win11_25H2_English_x64_v2.iso\"; filename*=UTF-8''Win11_25H2_English_x64_v2.iso";
        assert_eq!(
            extract_filename(
                "https://software.download.prss.microsoft.com/dbazure/Win11.iso",
                Some(cd_quoted)
            ),
            "Win11_25H2_English_x64_v2.iso"
        );

        let cd_unquoted = "attachment; filename=Win11_25H2_English_x64_v2.iso; filename*=UTF-8''Win11_25H2_English_x64_v2.iso";
        assert_eq!(
            extract_filename(
                "https://software.download.prss.microsoft.com/dbazure/Win11.iso",
                Some(cd_unquoted)
            ),
            "Win11_25H2_English_x64_v2.iso"
        );
    }

    #[test]
    fn test_extract_filename_rfc5987_utf8_percent_encoded() {
        let cd = "attachment; filename*=UTF-8''%E2%9C%93%20Windows%20Update.iso";
        assert_eq!(
            extract_filename("https://example.com/get", Some(cd)),
            "✓ Windows Update.iso"
        );
    }

    #[test]
    fn test_extract_filename_rfc5987_iso8859_1() {
        let cd = "attachment; filename*=iso-8859-1'en'caf%E9.txt";
        assert_eq!(
            extract_filename("https://example.com/file", Some(cd)),
            "café.txt"
        );
    }

    #[test]
    fn test_extract_filename_quoted_with_semicolon() {
        let cd = "attachment; filename=\"release; v2.0.zip\"";
        assert_eq!(
            extract_filename("https://example.com/dl", Some(cd)),
            "release; v2.0.zip"
        );
    }

    #[test]
    fn test_extract_filename_extra_parameters() {
        let cd = "attachment; size=987654321; filename=\"document.pdf\"; modification-date=\"Wed, 12 Feb 1997 16:29:51 -0500\"";
        assert_eq!(
            extract_filename("https://example.com/doc", Some(cd)),
            "document.pdf"
        );
    }

    #[test]
    fn test_extract_filename_sanitizes_windows_illegal_chars() {
        let cd = "attachment; filename=\"file:with*invalid?chars<foo>bar|.zip\"";
        assert_eq!(
            extract_filename("https://example.com/dl", Some(cd)),
            "file_with_invalid_chars_foo_bar_.zip"
        );
    }

    #[test]
    fn test_extract_filename_url_percent_decoding() {
        assert_eq!(
            extract_filename("https://example.com/files/My%20Cool%20Project.zip", None),
            "My Cool Project.zip"
        );
    }

    #[test]
    fn test_is_valid_url_or_magnet() {
        assert!(is_valid_url_or_magnet("http://example.com/file.zip"));
        assert!(is_valid_url_or_magnet("https://example.com/image.png"));
        assert!(is_valid_url_or_magnet("ftp://server.org/archive.tar.gz"));
        assert!(is_valid_url_or_magnet(
            "magnet:?xt=urn:btih:c12fe1c06bba254a9dc9f519b335380dc1d7d1ee&dn=archlinux.iso"
        ));
        assert!(is_valid_url_or_magnet("C:\\Downloads\\ubuntu.torrent"));
        assert!(is_valid_url_or_magnet("/tmp/test.torrent"));

        assert!(!is_valid_url_or_magnet("hello world"));
        assert!(!is_valid_url_or_magnet("random text"));
        assert!(!is_valid_url_or_magnet("C:\\Documents\\resume.docx"));
        assert!(!is_valid_url_or_magnet(""));
    }
}
