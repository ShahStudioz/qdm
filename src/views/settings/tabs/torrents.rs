use crate::theme::{colors, styles};
use crate::views::settings::settings::{
    custom_switch, setting_row, stepper_widget, SettingsMessage, SettingsModel, SpeedUnit,
};
use iced::widget::{column, container, pick_list, row, text_input};
use iced::{Alignment, Element, Length};

pub fn view<'a>(model: &SettingsModel) -> Element<'a, SettingsMessage> {
    // 1. Enable DHT
    let item_dht = setting_row(
        "Enable DHT (Distributed Hash Table)",
        "Allows finding peers without relying solely on centralized trackers",
        custom_switch(
            model.torrent_enable_dht,
            model.torrent_enable_dht_anim,
            SettingsMessage::ToggleTorrentDht,
        ),
    );

    // 2. Enable UPnP
    let item_upnp = setting_row(
        "Enable UPnP Port Mapping",
        "Automatically forward incoming port on UPnP-compatible routers",
        custom_switch(
            model.torrent_enable_upnp,
            model.torrent_enable_upnp_anim,
            SettingsMessage::ToggleTorrentUpnp,
        ),
    );

    // 3. Listen Port
    let port_input = text_input("6881", &model.torrent_listen_port)
        .on_input(SettingsMessage::TorrentListenPortChanged)
        .padding([6, 12])
        .width(100)
        .style(styles::transparent_text_input_style);

    let port_box = container(port_input)
        .padding([0, 4])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let item_port = setting_row(
        "Incoming Listen Port",
        "TCP/UDP port used for incoming peer connections",
        port_box.into(),
    );

    // 4. Max Peers
    let max_peers_widget = stepper_widget(
        &model.torrent_max_peers.to_string(),
        SettingsMessage::TorrentMaxPeersDec,
        SettingsMessage::TorrentMaxPeersInc,
    );

    let item_max_peers = setting_row(
        "Max peers per torrent",
        "Maximum number of connected peers per active torrent download",
        max_peers_widget,
    );

    // 5. Seed Ratio Limit
    let ratio_input = text_input("Unlimited", &model.torrent_seed_ratio_limit)
        .on_input(SettingsMessage::TorrentSeedRatioLimitChanged)
        .padding([6, 10])
        .width(120)
        .style(styles::transparent_text_input_style);

    let ratio_box = container(ratio_input)
        .padding([0, 4])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let item_seed_ratio = setting_row(
        "Seed Ratio Limit",
        "Stop seeding torrents automatically when this ratio is reached (e.g., 1.5)",
        ratio_box.into(),
    );

    // 6. Upload Speed Limit
    let ul_input = text_input("Unlimited", &model.torrent_upload_limit_value)
        .on_input(SettingsMessage::TorrentUploadLimitValueChanged)
        .padding([6, 10])
        .width(120)
        .style(styles::transparent_text_input_style);

    let ul_box = container(ul_input)
        .padding([0, 4])
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::SURFACE_HIGH)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Default::default()
        });

    let ul_unit_dropdown = pick_list(
        SpeedUnit::ALL,
        Some(model.torrent_upload_limit_unit),
        SettingsMessage::TorrentUploadLimitUnitChanged,
    )
    .style(styles::pick_list_style)
    .menu_style(styles::pick_list_menu_style)
    .padding([8, 12])
    .width(90);

    let ul_control = row![ul_box, ul_unit_dropdown]
        .spacing(8)
        .align_y(Alignment::Center);

    let item_ul_limit = setting_row(
        "Torrent upload rate limit",
        "Maximum upload bandwidth for seeding torrents (empty for unlimited)",
        ul_control.into(),
    );

    // 2.5 Play Media While Downloading
    let item_play_media = setting_row(
        "Play media while it is still downloading",
        "Downloads media files without the .qdmdownload extension so they can be opened in a media player",
        custom_switch(
            model.torrent_play_media_while_downloading,
            model.torrent_play_media_while_downloading_anim,
            SettingsMessage::ToggleTorrentPlayMedia,
        ),
    );

    column![
        item_dht,
        item_upnp,
        item_play_media,
        item_port,
        item_max_peers,
        item_seed_ratio,
        item_ul_limit,
    ]
    .spacing(24)
    .padding([20, 24])
    .width(Length::Fill)
    .into()
}
