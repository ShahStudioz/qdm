use crate::icons::{self, icon};
use crate::services::download::{DownloadFileMetaData, DownloadService};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Task};

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
    pub downloader: DownloadService,
    pub has_error: bool,
    pub error: String,
    pub download_file_metadata: Option<DownloadFileMetaData>,
}

impl Default for AddDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            step: AddDialogStep::UrlInput,
            url: String::new(),
            filename: String::new(),
            save_to: "C:\\Users\\Downloads".to_string(),
            is_advanced_expanded: false,
            max_connections: "8".to_string(),
            speed_limit: String::new(),
            downloader: DownloadService::default(),
            has_error: false,
            error: String::new(),
            download_file_metadata: None,
        }
    }
}

impl AddDialogModel {
    pub fn reset(&mut self) {
        self.step = AddDialogStep::UrlInput;
        self.url.clear();
        self.filename.clear();
        self.has_error = false;
        self.error.clear();
        self.download_file_metadata = None;
        self.is_advanced_expanded = false;
    }
}

pub fn view(state: &AddDialogModel) -> Element<'_, AddDialogueModalMessage> {
    // 1. Header
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

    // Error Message Container
    let error_view: Element<AddDialogueModalMessage> = if state.has_error && !state.error.is_empty() {
        container(
            row![
                icon(icons::ICON_WARN).size(14).color(colors::ERROR),
                text(&state.error).color(colors::ERROR).size(12),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
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

    // Main Content based on step
    let form_body: Element<AddDialogueModalMessage> = match state.step {
        AddDialogStep::UrlInput | AddDialogStep::FetchingInfo => {
            // STEP 1: URL Input with Paste Icon inside on the left
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

            let url_widget = text_input("https://example.com/file.zip", &state.url)
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

            let url_group = column![url_label, url_box].spacing(6);

            column![error_view, url_group]
                .spacing(14)
                .padding([20, 20])
                .into()
        }
        AddDialogStep::DownloadDetails => {
            // STEP 2: Full Download Details
            let metadata_summary: Element<AddDialogueModalMessage> =
                if let Some(ref meta) = state.download_file_metadata {
                    let size_str = meta
                        .content_length
                        .map(format_bytes)
                        .unwrap_or_else(|| "Unknown size".to_string());

                    let resume_str = if meta.supports_resume {
                        "Resume: Supported"
                    } else {
                        "Resume: Not supported"
                    };

                    let type_str = meta
                        .content_type
                        .as_deref()
                        .unwrap_or("Unknown type");

                    container(
                        row![
                            text(format!("Size: {}", size_str))
                                .size(12)
                                .font(styles::BOLD_FONT)
                                .color(colors::TEXT_PRIMARY),
                            text("|").size(12).color(colors::TEXT_MUTED),
                            text(resume_str)
                                .size(12)
                                .color(if meta.supports_resume {
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
                } else {
                    Space::with_height(0).into()
                };

            // DOWNLOAD URL (read-only/editable display)
            let url_label = form_label("DOWNLOAD URL");
            let url_input = text_input("", &state.url)
                .on_input(AddDialogueModalMessage::AddUrlChanged)
                .padding([8, 10])
                .width(Length::Fill)
                .style(styles::dark_input_style);
            let url_group = column![url_label, url_input].spacing(4);

            // FILENAME
            let filename_label = form_label("FILENAME");
            let wand_icon = icon(icons::ICON_WAND).size(14).color(colors::PRIMARY);
            let filename_widget = text_input("file.zip", &state.filename)
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

            // SAVE TO
            let save_to_label = form_label("SAVE TO");
            let save_to_input = text_input("C:\\Users\\Downloads", &state.save_to)
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

            // Collapsible Advanced Options
            let chevron_char = if state.is_advanced_expanded {
                icons::ICON_CHEVRON_UP
            } else {
                icons::ICON_CHEVRON_DOWN
            };

            let advanced_header_btn = button(
                row![
                    text("Advanced Options")
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
                let max_conn_label = form_label("MAX CONNECTIONS");
                let max_conn_input = text_input("8", &state.max_connections)
                    .on_input(AddDialogueModalMessage::AddMaxConnectionsChanged)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(styles::dark_input_style);

                let max_conn_col = column![max_conn_label, max_conn_input]
                    .spacing(6)
                    .width(Length::FillPortion(1));

                let speed_limit_label = form_label("SPEED LIMIT");
                let speed_limit_input = text_input("Unlimited", &state.speed_limit)
                    .on_input(AddDialogueModalMessage::AddSpeedLimitChanged)
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(styles::dark_input_style);

                let speed_limit_col = column![speed_limit_label, speed_limit_input]
                    .spacing(6)
                    .width(Length::FillPortion(1));

                row![max_conn_col, speed_limit_col].spacing(16).into()
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
                save_to_group,
                advanced_group
            ]
            .spacing(14)
            .padding([16, 20])
            .into()
        }
    };

    // 3. Footer Buttons
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

    let primary_btn: Element<AddDialogueModalMessage> = match state.step {
        AddDialogStep::UrlInput => button(
            text("Ok")
                .size(14)
                .font(styles::BOLD_FONT)
                .color(colors::BACKGROUND),
        )
        .padding([10, 24])
        .style(styles::primary_button_style)
        .on_press(AddDialogueModalMessage::FetchFileInfoPressed)
        .into(),

        AddDialogStep::FetchingInfo => button(
            row![
                text("Fetching...").size(14).color(colors::TEXT_MUTED),
            ]
            .align_y(Alignment::Center),
        )
        .padding([10, 20])
        .style(styles::ghost_button_style)
        .into(),

        AddDialogStep::DownloadDetails => button(
            text("Start Download")
                .size(14)
                .font(styles::BOLD_FONT)
                .color(colors::BACKGROUND),
        )
        .padding([10, 20])
        .style(styles::primary_button_style)
        .on_press(AddDialogueModalMessage::SubmitNewDownload)
        .into(),
    };

    let footer_row = row![Space::with_width(Length::Fill), cancel_btn, primary_btn]
        .spacing(12)
        .padding([16, 20])
        .align_y(Alignment::Center);

    // Modal Card Layout
    let modal_card = container(column![
        header_row,
        header_divider,
        form_body,
        footer_divider,
        footer_row,
    ])
    .width(520)
    .style(styles::card_style);

    // Semi-transparent Overlay Backdrop
    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.0, 0.0, 0.0, 0.65,
            ))),
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

#[derive(Debug, Clone)]
pub enum AddDialogueModalMessage {
    CloseAddDialog,
    PasteFromClipboard,
    ClipboardContentRead(Option<String>),
    AddUrlChanged(String),
    FetchFileInfoPressed,
    FileMetaDataFetched(Result<DownloadFileMetaData, String>),
    AddFilenameChanged(String),
    AddSaveToChanged(String),
    AddBrowseFolderPressed,
    ToggleAdvancedOptions,
    AddMaxConnectionsChanged(String),
    AddSpeedLimitChanged(String),
    SubmitNewDownload,
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
                if !trimmed.is_empty() {
                    state.url = trimmed;
                    state.has_error = false;
                    state.error.clear();
                }
            }
        }
        AddDialogueModalMessage::AddUrlChanged(url) => {
            state.url = url;
            state.has_error = false;
            state.error.clear();
        }
        AddDialogueModalMessage::FetchFileInfoPressed => {
            let url = state.url.trim().to_string();
            if url.is_empty() {
                state.has_error = true;
                state.error = "Please enter a valid download URL".to_string();
            } else if !url.starts_with("http://") && !url.starts_with("https://") {
                state.has_error = true;
                state.error = "URL must start with http:// or https://".to_string();
            } else {
                state.step = AddDialogStep::FetchingInfo;
                state.has_error = false;
                state.error.clear();

                let downloader = state.downloader.clone();
                let task = async move { downloader.get_file_meta_data(&url).await };
                return Task::perform(task, AddDialogueModalMessage::FileMetaDataFetched);
            }
        }
        AddDialogueModalMessage::FileMetaDataFetched(meta_res) => match meta_res {
            Ok(data) => {
                let extracted_name = extract_filename(&state.url, data.content_disposition.as_deref());
                state.filename = extracted_name;
                state.download_file_metadata = Some(data);
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
        AddDialogueModalMessage::AddSaveToChanged(save_to) => {
            state.save_to = save_to;
        }
        AddDialogueModalMessage::AddBrowseFolderPressed => {
            println!("[QDM] Add Dialog browse folder clicked");
        }
        AddDialogueModalMessage::ToggleAdvancedOptions => {
            state.is_advanced_expanded = !state.is_advanced_expanded;
        }
        AddDialogueModalMessage::AddMaxConnectionsChanged(val) => {
            state.max_connections = val;
        }
        AddDialogueModalMessage::AddSpeedLimitChanged(val) => {
            state.speed_limit = val;
        }
        AddDialogueModalMessage::SubmitNewDownload => {
            // Final submission is handled at app level
        }
    }
    Task::none()
}

pub fn extract_filename(url_str: &str, content_disposition: Option<&str>) -> String {
    if let Some(cd) = content_disposition {
        if let Some(pos) = cd.find("filename=") {
            let filename_part = &cd[pos + 9..];
            let trimmed = filename_part.trim_matches(|c| c == '"' || c == '\'' || c == ';');
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    if let Ok(url) = reqwest::Url::parse(url_str) {
        if let Some(segments) = url.path_segments() {
            if let Some(last) = segments.last() {
                if !last.is_empty() {
                    return last.to_string();
                }
            }
        }
    } else {
        let path = url_str.split('?').next().unwrap_or(url_str);
        if let Some(last) = path.split('/').last() {
            if !last.is_empty() {
                return last.to_string();
            }
        }
    }

    "download.file".to_string()
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
