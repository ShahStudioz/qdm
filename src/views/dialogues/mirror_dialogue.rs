use crate::icons::{self, icon};
use crate::models::download::{format_bytes, DownloadUrl};
use crate::theme::{colors, styles};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};
use iced::{Alignment, Element, Length, Task};

#[derive(Debug, Clone)]
pub struct MirrorDialogModel {
    pub is_open: bool,
    pub download_id: usize,
    pub filename: String,
    pub primary_url: DownloadUrl,
    pub mirror_urls: Vec<DownloadUrl>,
    pub new_mirror_url: String,
    pub error_message: String,
    pub testing_index: Option<usize>, // None = not testing, Some(idx) = testing mirror idx, Some(usize::MAX) = testing primary
}

impl Default for MirrorDialogModel {
    fn default() -> Self {
        Self {
            is_open: false,
            download_id: 0,
            filename: String::new(),
            primary_url: DownloadUrl::new(""),
            mirror_urls: Vec::new(),
            new_mirror_url: String::new(),
            error_message: String::new(),
            testing_index: None,
        }
    }
}

impl MirrorDialogModel {
    pub fn open(
        &mut self,
        download_id: usize,
        filename: String,
        primary_url: DownloadUrl,
        mirror_urls: Vec<DownloadUrl>,
    ) {
        self.is_open = true;
        self.download_id = download_id;
        self.filename = filename;
        self.primary_url = primary_url;
        self.mirror_urls = mirror_urls;
        self.new_mirror_url.clear();
        self.error_message.clear();
        self.testing_index = None;
    }

    pub fn close(&mut self) {
        self.is_open = false;
        self.new_mirror_url.clear();
        self.error_message.clear();
        self.testing_index = None;
    }
}

#[derive(Debug, Clone)]
pub enum MirrorDialogueMessage {
    CloseMirrorDialog,
    NewMirrorUrlChanged(String),
    AddMirrorPressed,
    DeleteMirrorPressed(usize),
    ToggleMirrorActive(usize),
    TestMirrorPressed(usize),
    TestPrimaryPressed,
    MirrorTested(usize, Result<u16, String>),
    PrimaryTested(Result<u16, String>),
    SaveAndClose,
}

pub fn update(
    state: &mut MirrorDialogModel,
    message: MirrorDialogueMessage,
) -> Task<MirrorDialogueMessage> {
    match message {
        MirrorDialogueMessage::CloseMirrorDialog => {
            state.close();
        }
        MirrorDialogueMessage::NewMirrorUrlChanged(url) => {
            state.new_mirror_url = url;
            state.error_message.clear();
        }
        MirrorDialogueMessage::AddMirrorPressed => {
            let trimmed = state.new_mirror_url.trim().to_string();
            if trimmed.is_empty() {
                state.error_message = "Please enter a mirror URL".to_string();
            } else if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                state.error_message = "URL must start with http:// or https://".to_string();
            } else if trimmed == state.primary_url.url
                || state.mirror_urls.iter().any(|m| m.url == trimmed)
            {
                state.error_message = "This URL is already in the mirror list".to_string();
            } else {
                state.mirror_urls.push(DownloadUrl::new(trimmed));
                state.new_mirror_url.clear();
                state.error_message.clear();
            }
        }
        MirrorDialogueMessage::DeleteMirrorPressed(idx) => {
            if idx < state.mirror_urls.len() {
                state.mirror_urls.remove(idx);
            }
        }
        MirrorDialogueMessage::ToggleMirrorActive(idx) => {
            if let Some(mirror) = state.mirror_urls.get_mut(idx) {
                mirror.is_active = !mirror.is_active;
            }
        }
        MirrorDialogueMessage::TestMirrorPressed(idx) => {
            if let Some(mirror) = state.mirror_urls.get(idx) {
                let url = mirror.url.clone();
                state.testing_index = Some(idx);
                return Task::perform(
                    async move {
                        let client = reqwest::Client::builder()
                            .timeout(std::time::Duration::from_secs(5))
                            .build()
                            .map_err(|e| e.to_string())?;
                        let res = client.get(&url).header("Range", "bytes=0-0").send().await;
                        match res {
                            Ok(resp) => Ok(resp.status().as_u16()),
                            Err(e) => Err(e.to_string()),
                        }
                    },
                    move |res| MirrorDialogueMessage::MirrorTested(idx, res),
                );
            }
        }
        MirrorDialogueMessage::TestPrimaryPressed => {
            let url = state.primary_url.url.clone();
            state.testing_index = Some(usize::MAX);
            return Task::perform(
                async move {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(5))
                        .build()
                        .map_err(|e| e.to_string())?;
                    let res = client.get(&url).header("Range", "bytes=0-0").send().await;
                    match res {
                        Ok(resp) => Ok(resp.status().as_u16()),
                        Err(e) => Err(e.to_string()),
                    }
                },
                MirrorDialogueMessage::PrimaryTested,
            );
        }
        MirrorDialogueMessage::MirrorTested(idx, res) => {
            state.testing_index = None;
            if let Some(mirror) = state.mirror_urls.get_mut(idx) {
                mirror.last_checked_at = Some(now_timestamp());
                match res {
                    Ok(code) => {
                        mirror.status_code = Some(code);
                        mirror.error = None;
                    }
                    Err(err) => {
                        mirror.status_code = None;
                        mirror.error = Some(err);
                    }
                }
            }
        }
        MirrorDialogueMessage::PrimaryTested(res) => {
            state.testing_index = None;
            state.primary_url.last_checked_at = Some(now_timestamp());
            match res {
                Ok(code) => {
                    state.primary_url.status_code = Some(code);
                    state.primary_url.error = None;
                }
                Err(err) => {
                    state.primary_url.status_code = None;
                    state.primary_url.error = Some(err);
                }
            }
        }
        MirrorDialogueMessage::SaveAndClose => {
            state.close();
        }
    }
    Task::none()
}

pub fn view(state: &MirrorDialogModel) -> Element<'_, MirrorDialogueMessage> {
    let title_text = row![
        icon(icons::ICON_LINK).size(18).color(colors::PRIMARY),
        text("Manage Mirror Links")
            .size(18)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let close_btn = button(icon(icons::ICON_CANCEL).size(14).color(colors::TEXT_MUTED))
        .style(styles::icon_button_style)
        .on_press(MirrorDialogueMessage::CloseMirrorDialog);

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

    // File name banner
    let file_info = container(
        column![
            text(format!("File: {}", state.filename))
                .size(13)
                .font(styles::BOLD_FONT)
                .color(colors::TEXT_PRIMARY),
            text(format!("Total Mirrors: {}", state.mirror_urls.len()))
                .size(11)
                .color(colors::TEXT_MUTED),
        ]
        .spacing(2),
    )
    .padding([10, 16])
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
        border: iced::Border {
            color: colors::BORDER,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    });

    // Primary URL Section
    let primary_status_badge = render_status_badge(
        state.primary_url.status_code,
        state.primary_url.error.as_deref(),
        state.testing_index == Some(usize::MAX),
    );

    let test_primary_btn = button(
        row![
            icon(icons::ICON_RETRY).size(11).color(colors::TEXT_PRIMARY),
            text("Test").size(11).color(colors::TEXT_PRIMARY),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3, 8])
    .style(styles::ghost_button_style)
    .on_press(MirrorDialogueMessage::TestPrimaryPressed);

    let mut primary_col = column![
        row![
            text("PRIMARY SOURCE")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::PRIMARY),
            Space::with_width(Length::Fill),
            primary_status_badge,
            test_primary_btn,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(&state.primary_url.url)
            .size(12)
            .font(styles::MONO_FONT)
            .color(colors::TEXT_PRIMARY),
    ]
    .spacing(6);

    if state.primary_url.downloaded_bytes > 0 {
        primary_col = primary_col.push(
            text(format!(
                "Downloaded: {}",
                format_bytes(state.primary_url.downloaded_bytes)
            ))
            .size(11)
            .color(colors::TEXT_MUTED),
        );
    }

    let primary_box = container(primary_col)
        .padding([10, 14])
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    // Mirrors List
    let mut mirrors_list_col = column![].spacing(8).width(Length::Fill);

    if state.mirror_urls.is_empty() {
        let empty_mirrors = container(
            text("No mirror links added yet. Add backup URLs below for failover and faster chunk distribution.")
                .size(12)
                .color(colors::TEXT_MUTED),
        )
        .padding([14, 14])
        .width(Length::Fill)
        .align_x(Alignment::Center);

        mirrors_list_col = mirrors_list_col.push(empty_mirrors);
    } else {
        for (idx, mirror) in state.mirror_urls.iter().enumerate() {
            let is_testing = state.testing_index == Some(idx);
            let status_badge =
                render_status_badge(mirror.status_code, mirror.error.as_deref(), is_testing);

            let toggle_label = if mirror.is_active {
                "Active"
            } else {
                "Disabled"
            };
            let toggle_btn = button(
                row![
                    icon(if mirror.is_active {
                        icons::ICON_CHECK
                    } else {
                        icons::ICON_CANCEL
                    })
                    .size(11)
                    .color(if mirror.is_active {
                        colors::SUCCESS
                    } else {
                        colors::TEXT_MUTED
                    }),
                    text(toggle_label).size(11).color(if mirror.is_active {
                        colors::TEXT_PRIMARY
                    } else {
                        colors::TEXT_MUTED
                    }),
                ]
                .spacing(4)
                .align_y(Alignment::Center),
            )
            .padding([3, 8])
            .style(styles::ghost_button_style)
            .on_press(MirrorDialogueMessage::ToggleMirrorActive(idx));

            let test_btn = button(icon(icons::ICON_RETRY).size(12).color(colors::TEXT_PRIMARY))
                .padding([4, 8])
                .style(styles::ghost_button_style)
                .on_press(MirrorDialogueMessage::TestMirrorPressed(idx));

            let delete_btn = button(icon(icons::ICON_TRASH).size(12).color(colors::ERROR))
                .padding([4, 8])
                .style(styles::ghost_button_style)
                .on_press(MirrorDialogueMessage::DeleteMirrorPressed(idx));

            let mut mirror_detail_col = column![
                row![
                    text(format!("Mirror #{}", idx + 1))
                        .size(11)
                        .font(styles::BOLD_FONT)
                        .color(colors::TEXT_PRIMARY),
                    status_badge,
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text(&mirror.url)
                    .size(12)
                    .font(styles::MONO_FONT)
                    .color(if mirror.is_active {
                        colors::TEXT_PRIMARY
                    } else {
                        colors::TEXT_MUTED
                    }),
            ]
            .spacing(4)
            .width(Length::Fill);

            if mirror.downloaded_bytes > 0 {
                mirror_detail_col = mirror_detail_col.push(
                    text(format!(
                        "Downloaded: {}",
                        format_bytes(mirror.downloaded_bytes)
                    ))
                    .size(11)
                    .color(colors::TEXT_MUTED),
                );
            }

            let mirror_row = container(
                row![
                    mirror_detail_col,
                    row![toggle_btn, test_btn, delete_btn]
                        .spacing(4)
                        .align_y(Alignment::Center),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                border: iced::Border {
                    color: if mirror.is_active {
                        colors::BORDER
                    } else {
                        colors::SURFACE
                    },
                    width: 1.0,
                    radius: 6.0.into(),
                },
                ..Default::default()
            });

            mirrors_list_col = mirrors_list_col.push(mirror_row);
        }
    }

    let mirrors_scrollable = scrollable(mirrors_list_col)
        .height(Length::Fixed(180.0))
        .width(Length::Fill);

    // Add Mirror Form
    let url_input = text_input(
        "https://mirror1.example.com/file.zip",
        &state.new_mirror_url,
    )
    .on_input(MirrorDialogueMessage::NewMirrorUrlChanged)
    .on_submit(MirrorDialogueMessage::AddMirrorPressed)
    .padding([8, 10])
    .width(Length::Fill)
    .style(styles::dark_input_style);

    let add_btn = button(
        row![
            icon(icons::ICON_PLUS).size(12).color(colors::BACKGROUND),
            text("Add Mirror")
                .size(12)
                .font(styles::BOLD_FONT)
                .color(colors::BACKGROUND),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([8, 14])
    .style(styles::primary_button_style)
    .on_press(MirrorDialogueMessage::AddMirrorPressed);

    let add_mirror_row = row![url_input, add_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    let error_view: Element<MirrorDialogueMessage> = if !state.error_message.is_empty() {
        text(&state.error_message)
            .size(12)
            .color(colors::ERROR)
            .into()
    } else {
        Space::with_height(0).into()
    };

    let add_section = column![
        text("ADD MIRROR URL")
            .size(11)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_MUTED),
        add_mirror_row,
        error_view,
    ]
    .spacing(6);

    let modal_body = column![
        file_info,
        primary_box,
        text("MIRROR SOURCES (FALLBACK & LOAD DISTRIBUTION)")
            .size(11)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_MUTED),
        mirrors_scrollable,
        add_section,
    ]
    .spacing(12)
    .padding([16, 20]);

    let footer_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let done_btn = button(
        text("Done")
            .size(14)
            .font(styles::BOLD_FONT)
            .color(colors::BACKGROUND),
    )
    .padding([8, 24])
    .style(styles::primary_button_style)
    .on_press(MirrorDialogueMessage::SaveAndClose);

    let footer_row = row![Space::with_width(Length::Fill), done_btn]
        .padding([14, 20])
        .align_y(Alignment::Center);

    let modal_card = container(column![
        header_row,
        header_divider,
        modal_body,
        footer_divider,
        footer_row,
    ])
    .width(580)
    .style(styles::card_style);

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(styles::modal_backdrop_style)
        .into()
}

fn render_status_badge(
    status_code: Option<u16>,
    error: Option<&str>,
    is_testing: bool,
) -> Element<'static, MirrorDialogueMessage> {
    if is_testing {
        return container(
            text("Checking...")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::PRIMARY),
        )
        .padding([2, 6])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
            border: iced::Border {
                color: colors::PRIMARY,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into();
    }

    if let Some(code) = status_code {
        let (bg, border, text_color, label) = if (200..300).contains(&code) {
            (
                iced::Color::from_rgba(0.18, 0.80, 0.44, 0.15),
                colors::SUCCESS,
                colors::SUCCESS,
                format!("HTTP {}", code),
            )
        } else if code == 206 {
            (
                iced::Color::from_rgba(0.18, 0.80, 0.44, 0.15),
                colors::SUCCESS,
                colors::SUCCESS,
                "206 Partial Content".to_string(),
            )
        } else {
            (
                iced::Color::from_rgba(0.93, 0.26, 0.26, 0.15),
                colors::ERROR,
                colors::ERROR,
                format!("HTTP {}", code),
            )
        };

        container(
            text(label)
                .size(10)
                .font(styles::BOLD_FONT)
                .color(text_color),
        )
        .padding([2, 6])
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(bg)),
            border: iced::Border {
                color: border,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
    } else if error.is_some() {
        container(
            text("Error / Offline")
                .size(10)
                .font(styles::BOLD_FONT)
                .color(colors::ERROR),
        )
        .padding([2, 6])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgba(
                0.93, 0.26, 0.26, 0.15,
            ))),
            border: iced::Border {
                color: colors::ERROR,
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        })
        .into()
    } else {
        container(text("Untested").size(10).color(colors::TEXT_MUTED))
            .padding([2, 6])
            .style(|_| container::Style {
                background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
                border: iced::Border {
                    color: colors::BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn now_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
