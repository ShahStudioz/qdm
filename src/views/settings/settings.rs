use crate::models::schedule::{OnCompleteAction, ScheduleConfig};
use crate::theme::{colors, styles};
use crate::views::settings::tabs;
use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    General,
    Downloads,
    Torrents,
    Scheduler,
    Updates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ScheduleSubTab {
    #[default]
    Settings,
    ScheduledDownloads,
}

fn default_true() -> bool {
    true
}

fn default_simultaneous_downloads() -> usize {
    3
}

fn default_max_connections() -> usize {
    8
}

fn default_max_threads() -> usize {
    4
}

fn default_max_retries() -> u32 {
    3
}

fn default_max_peers() -> usize {
    50
}

fn default_listen_port() -> String {
    "6881".to_string()
}

fn default_update_api_url() -> String {
    "https://qdm.shahstudioz.store/api/v1/version-check".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum SpeedUnit {
    #[default]
    KBps,
    MBps,
    GBps,
}

impl SpeedUnit {
    pub const ALL: &'static [SpeedUnit] = &[SpeedUnit::KBps, SpeedUnit::MBps, SpeedUnit::GBps];

    pub fn as_str(&self) -> &'static str {
        match self {
            SpeedUnit::KBps => "KB/s",
            SpeedUnit::MBps => "MB/s",
            SpeedUnit::GBps => "GB/s",
        }
    }

    pub fn to_bps(self, val: u64) -> u64 {
        match self {
            SpeedUnit::KBps => val * 1024,
            SpeedUnit::MBps => val * 1024 * 1024,
            SpeedUnit::GBps => val * 1024 * 1024 * 1024,
        }
    }
}

impl std::fmt::Display for SpeedUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileConflictAction {
    AutoRename,
    Overwrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteAction {
    RemoveFromList,
    DeleteFromDisk,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsModel {
    #[serde(default)]
    pub active_tab: SettingsTab,

    /// Tracks whether initial first-run setup tasks (e.g. registering OS startup) are pending.
    #[serde(default = "default_true")]
    pub is_fresh_install: bool,

    // General Tab
    #[serde(default = "default_true")]
    pub launch_at_startup: bool,
    #[serde(skip)]
    pub launch_at_startup_anim: f32,
    #[serde(skip)]
    pub launch_at_startup_loading: bool,

    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,
    #[serde(skip)]
    pub minimize_to_tray_anim: f32,

    #[serde(default = "default_true")]
    pub show_notifications: bool,
    #[serde(skip)]
    pub show_notifications_anim: f32,

    // Downloads Tab
    #[serde(default = "crate::core::utils::paths::get_default_download_dir")]
    pub download_folder: String,

    #[serde(default)]
    pub file_conflict_action: Option<FileConflictAction>,

    #[serde(default)]
    pub delete_action: Option<DeleteAction>,

    #[serde(default = "default_simultaneous_downloads")]
    pub simultaneous_downloads: usize,

    #[serde(default = "default_max_connections")]
    pub max_connections: usize,

    #[serde(default = "default_max_threads")]
    pub max_threads: usize,

    #[serde(default = "default_true")]
    pub auto_retry_downloads: bool,
    #[serde(skip)]
    pub auto_retry_downloads_anim: f32,

    #[serde(default = "default_max_retries")]
    pub max_auto_retries: u32,

    #[serde(default)]
    pub speed_limit_value: String,

    #[serde(default)]
    pub speed_limit_unit: SpeedUnit,

    // Torrents Tab
    #[serde(default = "default_true")]
    pub torrent_enable_dht: bool,
    #[serde(skip)]
    pub torrent_enable_dht_anim: f32,

    #[serde(default = "default_true")]
    pub torrent_enable_upnp: bool,
    #[serde(skip)]
    pub torrent_enable_upnp_anim: f32,

    #[serde(default = "default_max_peers")]
    pub torrent_max_peers: usize,

    #[serde(default = "default_true")]
    pub torrent_play_media_while_downloading: bool,
    #[serde(skip)]
    pub torrent_play_media_while_downloading_anim: f32,

    #[serde(default = "default_listen_port")]
    pub torrent_listen_port: String,

    #[serde(default)]
    pub torrent_seed_ratio_limit: String,

    #[serde(default)]
    pub torrent_upload_limit_value: String,

    #[serde(default)]
    pub torrent_upload_limit_unit: SpeedUnit,

    // Scheduler Tab
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(skip)]
    pub schedule_subtab: ScheduleSubTab,

    // Updates Tab
    #[serde(default = "default_update_api_url")]
    pub update_api_url: String,
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    #[serde(skip)]
    pub auto_check_updates_anim: f32,
}

impl Default for SettingsModel {
    fn default() -> Self {
        Self {
            active_tab: SettingsTab::General,
            is_fresh_install: true,
            launch_at_startup: true,
            launch_at_startup_anim: 1.0,
            launch_at_startup_loading: false,

            minimize_to_tray: true,
            minimize_to_tray_anim: 1.0,

            show_notifications: true,
            show_notifications_anim: 1.0,

            download_folder: crate::core::utils::paths::get_default_download_dir(),
            file_conflict_action: None,
            delete_action: None,
            simultaneous_downloads: 3,
            max_connections: 8,
            max_threads: 4,
            auto_retry_downloads: true,
            auto_retry_downloads_anim: 1.0,
            max_auto_retries: 3,
            speed_limit_value: String::new(),
            speed_limit_unit: SpeedUnit::KBps,

            torrent_enable_dht: true,
            torrent_enable_dht_anim: 1.0,
            torrent_enable_upnp: true,
            torrent_enable_upnp_anim: 1.0,
            torrent_max_peers: 50,
            torrent_play_media_while_downloading: true,
            torrent_play_media_while_downloading_anim: 1.0,
            torrent_listen_port: "6881".to_string(),
            torrent_seed_ratio_limit: String::new(),
            torrent_upload_limit_value: String::new(),
            torrent_upload_limit_unit: SpeedUnit::KBps,

            schedule: ScheduleConfig::default(),
            schedule_subtab: ScheduleSubTab::Settings,

            update_api_url: default_update_api_url(),
            auto_check_updates: true,
            auto_check_updates_anim: 1.0,
        }
    }
}

impl SettingsModel {
    pub fn global_speed_limit_bps(&self) -> Option<u64> {
        let trimmed = self.speed_limit_value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(num) = trimmed.parse::<u64>() {
            if num > 0 {
                return Some(self.speed_limit_unit.to_bps(num));
            }
        }
        None
    }

    pub fn sync_animations(&mut self) {
        self.launch_at_startup_anim = if self.launch_at_startup { 1.0 } else { 0.0 };
        self.minimize_to_tray_anim = if self.minimize_to_tray { 1.0 } else { 0.0 };
        self.show_notifications_anim = if self.show_notifications { 1.0 } else { 0.0 };
        self.auto_retry_downloads_anim = if self.auto_retry_downloads { 1.0 } else { 0.0 };
        self.torrent_enable_dht_anim = if self.torrent_enable_dht { 1.0 } else { 0.0 };
        self.torrent_enable_upnp_anim = if self.torrent_enable_upnp { 1.0 } else { 0.0 };
        self.torrent_play_media_while_downloading_anim =
            if self.torrent_play_media_while_downloading {
                1.0
            } else {
                0.0
            };
        self.schedule.enabled_anim = if self.schedule.enabled { 1.0 } else { 0.0 };
        self.schedule.stop_enabled_anim = if self.schedule.stop_enabled { 1.0 } else { 0.0 };
        self.schedule.prioritize_scheduled_anim = if self.schedule.prioritize_scheduled {
            1.0
        } else {
            0.0
        };
        self.auto_check_updates_anim = if self.auto_check_updates { 1.0 } else { 0.0 };
    }

    pub fn is_animating(&self) -> bool {
        let target_startup = if self.launch_at_startup { 1.0 } else { 0.0 };
        let target_tray = if self.minimize_to_tray { 1.0 } else { 0.0 };
        let target_notify = if self.show_notifications { 1.0 } else { 0.0 };
        let target_retry = if self.auto_retry_downloads { 1.0 } else { 0.0 };
        let target_torrent_dht = if self.torrent_enable_dht { 1.0 } else { 0.0 };
        let target_torrent_upnp = if self.torrent_enable_upnp { 1.0 } else { 0.0 };
        let target_torrent_media = if self.torrent_play_media_while_downloading {
            1.0
        } else {
            0.0
        };
        let target_sched = if self.schedule.enabled { 1.0 } else { 0.0 };
        let target_sched_stop = if self.schedule.stop_enabled { 1.0 } else { 0.0 };
        let target_sched_priority = if self.schedule.prioritize_scheduled {
            1.0
        } else {
            0.0
        };
        let target_updates = if self.auto_check_updates { 1.0 } else { 0.0 };

        (self.launch_at_startup_anim - target_startup).abs() > 0.005
            || (self.minimize_to_tray_anim - target_tray).abs() > 0.005
            || (self.show_notifications_anim - target_notify).abs() > 0.005
            || (self.auto_retry_downloads_anim - target_retry).abs() > 0.005
            || (self.torrent_enable_dht_anim - target_torrent_dht).abs() > 0.005
            || (self.torrent_enable_upnp_anim - target_torrent_upnp).abs() > 0.005
            || (self.torrent_play_media_while_downloading_anim - target_torrent_media).abs() > 0.005
            || (self.schedule.enabled_anim - target_sched).abs() > 0.005
            || (self.schedule.stop_enabled_anim - target_sched_stop).abs() > 0.005
            || (self.schedule.prioritize_scheduled_anim - target_sched_priority).abs() > 0.005
            || (self.auto_check_updates_anim - target_updates).abs() > 0.005
    }

    pub fn tick_animation(&mut self) {
        let target_startup = if self.launch_at_startup { 1.0 } else { 0.0 };
        let target_tray = if self.minimize_to_tray { 1.0 } else { 0.0 };
        let target_notify = if self.show_notifications { 1.0 } else { 0.0 };
        let target_retry = if self.auto_retry_downloads { 1.0 } else { 0.0 };
        let target_torrent_dht = if self.torrent_enable_dht { 1.0 } else { 0.0 };
        let target_torrent_upnp = if self.torrent_enable_upnp { 1.0 } else { 0.0 };
        let target_torrent_media = if self.torrent_play_media_while_downloading {
            1.0
        } else {
            0.0
        };
        let target_sched = if self.schedule.enabled { 1.0 } else { 0.0 };
        let target_sched_stop = if self.schedule.stop_enabled { 1.0 } else { 0.0 };
        let target_sched_priority = if self.schedule.prioritize_scheduled {
            1.0
        } else {
            0.0
        };
        let target_updates = if self.auto_check_updates { 1.0 } else { 0.0 };

        self.launch_at_startup_anim += (target_startup - self.launch_at_startup_anim) * 0.35;
        self.minimize_to_tray_anim += (target_tray - self.minimize_to_tray_anim) * 0.35;
        self.show_notifications_anim += (target_notify - self.show_notifications_anim) * 0.35;
        self.auto_retry_downloads_anim += (target_retry - self.auto_retry_downloads_anim) * 0.35;
        self.torrent_enable_dht_anim += (target_torrent_dht - self.torrent_enable_dht_anim) * 0.35;
        self.torrent_enable_upnp_anim +=
            (target_torrent_upnp - self.torrent_enable_upnp_anim) * 0.35;
        self.torrent_play_media_while_downloading_anim +=
            (target_torrent_media - self.torrent_play_media_while_downloading_anim) * 0.35;
        self.schedule.enabled_anim += (target_sched - self.schedule.enabled_anim) * 0.35;
        self.schedule.stop_enabled_anim +=
            (target_sched_stop - self.schedule.stop_enabled_anim) * 0.35;
        self.schedule.prioritize_scheduled_anim +=
            (target_sched_priority - self.schedule.prioritize_scheduled_anim) * 0.35;
        self.auto_check_updates_anim += (target_updates - self.auto_check_updates_anim) * 0.35;

        if (self.launch_at_startup_anim - target_startup).abs() < 0.005 {
            self.launch_at_startup_anim = target_startup;
        }
        if (self.minimize_to_tray_anim - target_tray).abs() < 0.005 {
            self.minimize_to_tray_anim = target_tray;
        }
        if (self.show_notifications_anim - target_notify).abs() < 0.005 {
            self.show_notifications_anim = target_notify;
        }
        if (self.auto_retry_downloads_anim - target_retry).abs() < 0.005 {
            self.auto_retry_downloads_anim = target_retry;
        }
        if (self.torrent_enable_dht_anim - target_torrent_dht).abs() < 0.005 {
            self.torrent_enable_dht_anim = target_torrent_dht;
        }
        if (self.torrent_enable_upnp_anim - target_torrent_upnp).abs() < 0.005 {
            self.torrent_enable_upnp_anim = target_torrent_upnp;
        }
        if (self.torrent_play_media_while_downloading_anim - target_torrent_media).abs() < 0.005 {
            self.torrent_play_media_while_downloading_anim = target_torrent_media;
        }
        if (self.schedule.enabled_anim - target_sched).abs() < 0.005 {
            self.schedule.enabled_anim = target_sched;
        }
        if (self.schedule.stop_enabled_anim - target_sched_stop).abs() < 0.005 {
            self.schedule.stop_enabled_anim = target_sched_stop;
        }
        if (self.schedule.prioritize_scheduled_anim - target_sched_priority).abs() < 0.005 {
            self.schedule.prioritize_scheduled_anim = target_sched_priority;
        }
        if (self.auto_check_updates_anim - target_updates).abs() < 0.005 {
            self.auto_check_updates_anim = target_updates;
        }
    }
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    TabSelected(SettingsTab),

    // General Tab
    ToggleStartup(bool),
    StartupRegistrationFinished(bool, Result<(), String>),
    ToggleTray(bool),
    ToggleNotifications(bool),

    // Downloads Tab
    FolderChanged(String),
    BrowseFolderPressed,
    BrowseFolderResult(Option<String>),
    FileConflictActionChanged(String),
    DeleteActionChanged(String),
    SimultaneousDownloadsDec,
    SimultaneousDownloadsInc,
    MaxConnectionsDec,
    MaxConnectionsInc,
    MaxThreadsDec,
    MaxThreadsInc,
    ToggleAutoRetry(bool),
    MaxAutoRetriesDec,
    MaxAutoRetriesInc,
    SpeedLimitValueChanged(String),
    SpeedLimitUnitChanged(SpeedUnit),

    // Torrents Tab
    ToggleTorrentDht(bool),
    ToggleTorrentUpnp(bool),
    ToggleTorrentPlayMedia(bool),
    TorrentMaxPeersDec,
    TorrentMaxPeersInc,
    TorrentListenPortChanged(String),
    TorrentSeedRatioLimitChanged(String),
    TorrentUploadLimitValueChanged(String),
    TorrentUploadLimitUnitChanged(SpeedUnit),

    // Scheduler Tab
    ScheduleSubTabSelected(ScheduleSubTab),
    ToggleScheduleEnabled(bool),
    ToggleScheduleStopEnabled(bool),
    TogglePrioritizeScheduled(bool),
    ScheduleStartTimeChanged(String),
    ScheduleStopTimeChanged(String),
    ScheduleToggleDay(usize),
    ScheduleOnCompleteChanged(OnCompleteAction),
    MoveScheduledItemUp(usize),
    MoveScheduledItemDown(usize),
    RemoveFromSchedule(usize),

    // Updates Tab
    ToggleUpdates(bool),
    CheckUpdatesPressed,
    StartUpdateDownload,
    CancelUpdateDownload,
    InstallUpdatePressed,

    // Global
    ResetDefaultsPressed,
}

pub fn update(model: &mut SettingsModel, message: SettingsMessage) {
    match message {
        SettingsMessage::TabSelected(tab) => model.active_tab = tab,

        SettingsMessage::ToggleStartup(val) => {
            model.launch_at_startup = val;
            model.launch_at_startup_loading = true;
        }
        SettingsMessage::StartupRegistrationFinished(val, res) => {
            model.launch_at_startup_loading = false;
            if let Err(err) = res {
                eprintln!("[QDM Startup Registration Error] {}", err);
                model.launch_at_startup = !val;
            } else {
                model.launch_at_startup = val;
            }
        }
        SettingsMessage::ToggleTray(val) => model.minimize_to_tray = val,
        SettingsMessage::ToggleNotifications(val) => model.show_notifications = val,

        SettingsMessage::FolderChanged(folder) => model.download_folder = folder,
        SettingsMessage::BrowseFolderPressed => {
            println!("[QDM] Browse download folder requested");
        }
        SettingsMessage::BrowseFolderResult(res) => {
            if let Some(folder) = res {
                model.download_folder = folder;
            }
        }
        SettingsMessage::FileConflictActionChanged(val) => {
            model.file_conflict_action = match val.as_str() {
                "Auto-rename file" => Some(FileConflictAction::AutoRename),
                "Overwrite existing file" => Some(FileConflictAction::Overwrite),
                _ => None,
            };
        }
        SettingsMessage::DeleteActionChanged(val) => {
            model.delete_action = match val.as_str() {
                "Remove from list only" => Some(DeleteAction::RemoveFromList),
                "Delete file from disk" => Some(DeleteAction::DeleteFromDisk),
                _ => None,
            };
        }
        SettingsMessage::SimultaneousDownloadsDec => {
            if model.simultaneous_downloads > 1 {
                model.simultaneous_downloads -= 1;
            }
        }
        SettingsMessage::SimultaneousDownloadsInc => {
            if model.simultaneous_downloads < 16 {
                model.simultaneous_downloads += 1;
            }
        }
        SettingsMessage::MaxConnectionsDec => {
            if model.max_connections > 1 {
                model.max_connections -= 1;
            }
        }
        SettingsMessage::MaxConnectionsInc => {
            if model.max_connections < 32 {
                model.max_connections += 1;
            }
        }
        SettingsMessage::MaxThreadsDec => {
            if model.max_threads > 1 {
                model.max_threads -= 1;
            }
        }
        SettingsMessage::MaxThreadsInc => {
            if model.max_threads < 16 {
                model.max_threads += 1;
            }
        }
        SettingsMessage::ToggleAutoRetry(val) => model.auto_retry_downloads = val,
        SettingsMessage::MaxAutoRetriesDec => {
            if model.max_auto_retries > 1 {
                model.max_auto_retries -= 1;
            }
        }
        SettingsMessage::MaxAutoRetriesInc => {
            if model.max_auto_retries < 10 {
                model.max_auto_retries += 1;
            }
        }
        SettingsMessage::SpeedLimitValueChanged(val) => {
            // Keep only digits in speed limit input
            model.speed_limit_value = val.chars().filter(|c| c.is_ascii_digit()).collect();
        }
        SettingsMessage::SpeedLimitUnitChanged(unit) => model.speed_limit_unit = unit,

        // Torrents Tab Handlers
        SettingsMessage::ToggleTorrentDht(val) => model.torrent_enable_dht = val,
        SettingsMessage::ToggleTorrentUpnp(val) => model.torrent_enable_upnp = val,
        SettingsMessage::ToggleTorrentPlayMedia(val) => {
            model.torrent_play_media_while_downloading = val
        }
        SettingsMessage::TorrentMaxPeersDec => {
            if model.torrent_max_peers > 5 {
                model.torrent_max_peers -= 5;
            }
        }
        SettingsMessage::TorrentMaxPeersInc => {
            if model.torrent_max_peers < 200 {
                model.torrent_max_peers += 5;
            }
        }
        SettingsMessage::TorrentListenPortChanged(val) => {
            model.torrent_listen_port = val.chars().filter(|c| c.is_ascii_digit()).collect();
        }
        SettingsMessage::TorrentSeedRatioLimitChanged(val) => {
            model.torrent_seed_ratio_limit = val
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
        }
        SettingsMessage::TorrentUploadLimitValueChanged(val) => {
            model.torrent_upload_limit_value = val.chars().filter(|c| c.is_ascii_digit()).collect();
        }
        SettingsMessage::TorrentUploadLimitUnitChanged(unit) => {
            model.torrent_upload_limit_unit = unit
        }

        // Scheduler Tab Handlers
        SettingsMessage::ScheduleSubTabSelected(subtab) => model.schedule_subtab = subtab,
        SettingsMessage::ToggleScheduleEnabled(val) => model.schedule.enabled = val,
        SettingsMessage::ToggleScheduleStopEnabled(val) => model.schedule.stop_enabled = val,
        SettingsMessage::TogglePrioritizeScheduled(val) => {
            model.schedule.prioritize_scheduled = val
        }
        SettingsMessage::ScheduleStartTimeChanged(val) => model.schedule.start_time = val,
        SettingsMessage::ScheduleStopTimeChanged(val) => model.schedule.stop_time = val,
        SettingsMessage::ScheduleToggleDay(idx) => model.schedule.toggle_day(idx),
        SettingsMessage::ScheduleOnCompleteChanged(action) => {
            model.schedule.on_complete_action = action
        }
        SettingsMessage::MoveScheduledItemUp(_)
        | SettingsMessage::MoveScheduledItemDown(_)
        | SettingsMessage::RemoveFromSchedule(_) => {
            // These list modification actions are handled at App level
        }

        SettingsMessage::ToggleUpdates(val) => model.auto_check_updates = val,
        SettingsMessage::CheckUpdatesPressed
        | SettingsMessage::StartUpdateDownload
        | SettingsMessage::CancelUpdateDownload
        | SettingsMessage::InstallUpdatePressed => {
            // Handled at App level
        }
        SettingsMessage::ResetDefaultsPressed => {
            *model = SettingsModel {
                is_fresh_install: false,
                launch_at_startup_loading: true,
                ..SettingsModel::default()
            };
        }
    }

    // Auto-save changes immediately to JSON storage
    let _ = crate::services::shared::storage::json_store::save_settings(model);
}

pub fn settings_view<'a>(
    model: &'a SettingsModel,
    downloads: &'a [crate::models::download::DownloadItem],
    update_status: &'a crate::services::updater::UpdateStatus,
) -> Element<'a, SettingsMessage> {
    let tabs_row = row![
        tab_item("General", SettingsTab::General, model.active_tab),
        tab_item("Downloads", SettingsTab::Downloads, model.active_tab),
        tab_item("Torrents", SettingsTab::Torrents, model.active_tab),
        tab_item("Scheduler", SettingsTab::Scheduler, model.active_tab),
        tab_item("Updates", SettingsTab::Updates, model.active_tab),
    ]
    .spacing(24)
    .align_y(Alignment::Center);

    let tab_bar = container(tabs_row).padding([12, 24]).width(Length::Fill);

    let tab_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let tab_content: Element<SettingsMessage> = match model.active_tab {
        SettingsTab::General => tabs::general::view(model),
        SettingsTab::Downloads => tabs::downloads::view(model),
        SettingsTab::Torrents => tabs::torrents::view(model),
        SettingsTab::Scheduler => tabs::scheduler::view(model, downloads),
        SettingsTab::Updates => tabs::updates::view(model, update_status),
    };

    let footer_divider = container(Space::with_height(1))
        .width(Length::Fill)
        .height(1)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::BORDER)),
            ..Default::default()
        });

    let reset_btn = button(text("Reset to Defaults").size(13).color(colors::TEXT_MUTED))
        .padding([8, 14])
        .style(styles::ghost_button_style)
        .on_press(SettingsMessage::ResetDefaultsPressed);

    let footer_row = row![
        reset_btn,
        Space::with_width(Length::Fill),
        text("Settings are saved automatically")
            .size(12)
            .color(colors::TEXT_MUTED),
    ]
    .padding([14, 24])
    .align_y(Alignment::Center);

    let card_content = column![
        tab_bar,
        tab_divider,
        scrollable(tab_content).height(Length::Fill),
        footer_divider,
        footer_row,
    ];

    let settings_card = container(card_content)
        .width(Length::Fill)
        .max_width(820.0)
        .height(Length::Fill)
        .max_height(640.0)
        .style(styles::card_style);

    container(settings_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .padding([24, 32])
        .into()
}

pub fn custom_switch<'a>(
    is_on: bool,
    anim_progress: f32,
    on_toggle: impl Fn(bool) -> SettingsMessage + 'a,
) -> Element<'a, SettingsMessage> {
    custom_switch_with_loading(is_on, anim_progress, false, on_toggle)
}

pub fn custom_switch_with_loading<'a>(
    _is_on: bool,
    anim_progress: f32,
    is_loading: bool,
    on_toggle: impl Fn(bool) -> SettingsMessage + 'a,
) -> Element<'a, SettingsMessage> {
    let track_bg = if anim_progress > 0.5 {
        colors::PRIMARY
    } else {
        colors::SURFACE_HIGH
    };

    let thumb_offset = (2.0 + anim_progress * 20.0) as u16;

    let thumb = container(Space::with_width(18))
        .width(18)
        .height(18)
        .style(|_| container::Style {
            background: Some(iced::Background::Color(colors::TEXT_PRIMARY)),
            border: iced::Border {
                radius: 9.0.into(),
                ..Default::default()
            },
            shadow: iced::Shadow {
                color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: iced::Vector::new(0.0, 1.0),
                blur_radius: 2.0,
            },
            ..Default::default()
        });

    let track = container(row![Space::with_width(thumb_offset), thumb,].align_y(Alignment::Center))
        .width(44)
        .height(24)
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(track_bg)),
            border: iced::Border {
                color: colors::BORDER,
                width: 1.0,
                radius: 12.0.into(),
            },
            ..Default::default()
        });

    let target_state = anim_progress <= 0.5;

    let mut switch_btn = button(track).style(styles::icon_button_style).padding(0);
    if !is_loading {
        switch_btn = switch_btn.on_press(on_toggle(target_state));
    }

    if is_loading {
        row![
            crate::icons::icon(crate::icons::ICON_SPINNER)
                .size(12)
                .color(colors::PRIMARY),
            text("Applying...").size(12).color(colors::TEXT_MUTED),
            Space::with_width(4),
            switch_btn,
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    } else {
        switch_btn.into()
    }
}

pub fn stepper_widget<'a>(
    val: &str,
    on_dec: SettingsMessage,
    on_inc: SettingsMessage,
) -> Element<'a, SettingsMessage> {
    let minus_btn = button(
        text("-")
            .size(14)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY),
    )
    .padding([4, 12])
    .style(styles::ghost_button_style)
    .on_press(on_dec);

    let count_text = text(val.to_string())
        .size(13)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let count_box = container(count_text)
        .width(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center);

    let plus_btn = button(
        text("+")
            .size(14)
            .font(styles::BOLD_FONT)
            .color(colors::TEXT_PRIMARY),
    )
    .padding([4, 12])
    .style(styles::ghost_button_style)
    .on_press(on_inc);

    container(
        row![minus_btn, count_box, plus_btn]
            .spacing(4)
            .align_y(Alignment::Center),
    )
    .padding([2, 4])
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

fn tab_item<'a>(
    label: &'static str,
    tab: SettingsTab,
    active_tab: SettingsTab,
) -> Element<'a, SettingsMessage> {
    let is_active = tab == active_tab;

    let label_text = text(label)
        .size(14)
        .font(if is_active {
            styles::BOLD_FONT
        } else {
            iced::Font::DEFAULT
        })
        .color(if is_active {
            colors::PRIMARY
        } else {
            colors::TEXT_MUTED
        });

    let underline = container(Space::with_height(2))
        .width(Length::Fill)
        .height(2)
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(if is_active {
                colors::PRIMARY
            } else {
                iced::Color::TRANSPARENT
            })),
            ..Default::default()
        });

    let tab_col = column![label_text, Space::with_height(6), underline].align_x(Alignment::Center);

    button(tab_col)
        .style(styles::icon_button_style)
        .on_press(SettingsMessage::TabSelected(tab))
        .into()
}

pub fn setting_row<'a>(
    title: &'static str,
    description: &'static str,
    control: Element<'a, SettingsMessage>,
) -> Element<'a, SettingsMessage> {
    let title_text = text(title)
        .size(14)
        .font(styles::BOLD_FONT)
        .color(colors::TEXT_PRIMARY);

    let left_col = if description.is_empty() {
        column![title_text]
    } else {
        let desc_text = text(description).size(12).color(colors::TEXT_MUTED);
        column![title_text, desc_text].spacing(2)
    };

    let control_box = container(control).align_x(Alignment::End);

    row![
        left_col.width(Length::Fill),
        Space::with_width(20),
        control_box
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}
