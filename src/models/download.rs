#![allow(dead_code)]

use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_connections() -> u32 {
    8
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadUrl {
    pub url: String,
    #[serde(default)]
    pub status_code: Option<u16>,
    #[serde(default)]
    pub downloaded_bytes: u64,
    #[serde(default)]
    pub speed_bps: u64,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub last_checked_at: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

impl DownloadUrl {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            status_code: None,
            downloaded_bytes: 0,
            speed_bps: 0,
            is_active: true,
            last_checked_at: None,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadState {
    FetchingMetadata,
    Queued,
    Scheduled,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    /// Torrent hash verification in progress after resume.
    Checking {
        /// Verification progress as a percentage (0–100).
        progress_pct: u8,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Completed,
    Paused {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    WaitingForNetwork {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Failed {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        error: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileType {
    Archive,
    Code,
    Media,
    Document,
    Other,
}

impl FileType {
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();

        // Archive / disk image
        if lower.ends_with(".zip")
            || lower.ends_with(".tar")
            || lower.ends_with(".gz")
            || lower.ends_with(".bz2")
            || lower.ends_with(".xz")
            || lower.ends_with(".zst")
            || lower.ends_with(".7z")
            || lower.ends_with(".rar")
            || lower.ends_with(".iso")
            || lower.ends_with(".img")
            || lower.ends_with(".dmg")
            || lower.ends_with(".cab")
            || lower.ends_with(".deb")
            || lower.ends_with(".rpm")
        {
            FileType::Archive
        // Video / audio / image
        } else if lower.ends_with(".mp4")
            || lower.ends_with(".mkv")
            || lower.ends_with(".avi")
            || lower.ends_with(".mov")
            || lower.ends_with(".wmv")
            || lower.ends_with(".webm")
            || lower.ends_with(".flv")
            || lower.ends_with(".m4v")
            || lower.ends_with(".3gp")
            || lower.ends_with(".mp3")
            || lower.ends_with(".flac")
            || lower.ends_with(".ogg")
            || lower.ends_with(".wav")
            || lower.ends_with(".aac")
            || lower.ends_with(".m4a")
            || lower.ends_with(".opus")
            || lower.ends_with(".wma")
            || lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
            || lower.ends_with(".gif")
            || lower.ends_with(".webp")
            || lower.ends_with(".bmp")
            || lower.ends_with(".svg")
            || lower.ends_with(".ico")
        {
            FileType::Media
        // Code / executables / scripts
        } else if lower.ends_with(".rs")
            || lower.ends_with(".py")
            || lower.ends_with(".js")
            || lower.ends_with(".ts")
            || lower.ends_with(".html")
            || lower.ends_with(".css")
            || lower.ends_with(".sql")
            || lower.ends_with(".msi")
            || lower.ends_with(".exe")
            || lower.ends_with(".apk")
            || lower.ends_with(".appimage")
            || lower.ends_with(".sh")
            || lower.ends_with(".bat")
            || lower.ends_with(".ps1")
            || lower.ends_with(".c")
            || lower.ends_with(".cpp")
            || lower.ends_with(".h")
            || lower.ends_with(".java")
            || lower.ends_with(".go")
            || lower.ends_with(".json")
            || lower.ends_with(".xml")
            || lower.ends_with(".yaml")
            || lower.ends_with(".yml")
            || lower.ends_with(".toml")
            || lower.ends_with(".wasm")
            || lower.ends_with(".php")
        {
            FileType::Code
        // Documents / spreadsheets / ebooks
        } else if lower.ends_with(".pdf")
            || lower.ends_with(".doc")
            || lower.ends_with(".docx")
            || lower.ends_with(".xls")
            || lower.ends_with(".xlsx")
            || lower.ends_with(".ppt")
            || lower.ends_with(".pptx")
            || lower.ends_with(".odt")
            || lower.ends_with(".csv")
            || lower.ends_with(".rtf")
            || lower.ends_with(".epub")
            || lower.ends_with(".txt")
            || lower.ends_with(".md")
        {
            FileType::Document
        } else {
            FileType::Other
        }
    }
}

/// Represents an exact contiguous byte range allocated to a chunk worker.
/// Tracks progress at byte-level accuracy rather than lossy percentages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkState {
    pub id: usize,
    #[serde(default)]
    pub url: String,
    pub start_byte: u64,
    pub end_byte: u64,
    pub current_offset: u64,
    #[serde(default)]
    pub is_completed: bool,
}

impl ChunkState {
    pub fn new(id: usize, url: impl Into<String>, start_byte: u64, end_byte: u64) -> Self {
        Self {
            id,
            url: url.into(),
            start_byte,
            end_byte,
            current_offset: start_byte,
            is_completed: false,
        }
    }

    pub fn downloaded_bytes(&self) -> u64 {
        if self.current_offset >= self.start_byte {
            let downloaded = self.current_offset - self.start_byte;
            if self.end_byte == u64::MAX {
                downloaded
            } else {
                downloaded.min(self.total_chunk_bytes())
            }
        } else {
            0
        }
    }

    pub fn total_chunk_bytes(&self) -> u64 {
        if self.end_byte >= self.start_byte {
            if self.end_byte == u64::MAX {
                u64::MAX - self.start_byte
            } else {
                (self.end_byte - self.start_byte).saturating_add(1)
            }
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadType {
    Http(HttpMetadata),
    Torrent(TorrentMetadata),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpMetadata {
    pub primary_url: DownloadUrl,
    #[serde(default)]
    pub mirror_urls: Vec<DownloadUrl>,
    #[serde(default)]
    pub resumable: bool,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub chunks: Vec<ChunkState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TorrentMetadata {
    pub magnet_uri: String,
    #[serde(default)]
    pub info_hash: Option<String>,
    #[serde(default)]
    pub selected_files: Option<Vec<usize>>,
    #[serde(default)]
    pub is_folder: bool,
    #[serde(default)]
    pub peers_connected: u32,
    #[serde(default)]
    pub seeds_connected: u32,
    #[serde(default)]
    pub upload_speed_bps: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: usize,
    pub filename: String,
    #[serde(rename = "type")]
    pub download_type: DownloadType,
    #[serde(default)]
    pub save_path: String,
    #[serde(default)]
    pub downloaded_bytes: u64,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    pub state: DownloadState,
    pub file_type: FileType,
    #[serde(default)]
    pub is_scheduled: bool,
    #[serde(default = "default_connections")]
    pub max_connections: u32,
    #[serde(default)]
    pub speed_limit_bps: Option<u64>,
    #[serde(default)]
    pub sha256_hash: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
    #[serde(default)]
    pub completed_at: Option<u64>,
}

impl DownloadItem {
    pub fn progress(&self) -> f32 {
        match &self.state {
            DownloadState::Completed => 100.0,
            DownloadState::FetchingMetadata | DownloadState::Queued | DownloadState::Scheduled => {
                0.0
            }
            DownloadState::Downloading {
                downloaded_bytes,
                total_bytes,
                ..
            }
            | DownloadState::Paused {
                downloaded_bytes,
                total_bytes,
            }
            | DownloadState::WaitingForNetwork {
                downloaded_bytes,
                total_bytes,
            }
            | DownloadState::Failed {
                downloaded_bytes,
                total_bytes,
                ..
            }
            | DownloadState::Checking {
                downloaded_bytes,
                total_bytes,
                ..
            } => {
                if let Some(total) = total_bytes {
                    if *total > 0 {
                        ((*downloaded_bytes as f64 / *total as f64) * 100.0) as f32
                    } else {
                        0.0
                    }
                } else if self.downloaded_bytes > 0 {
                    if let Some(total) = self.total_bytes {
                        if total > 0 {
                            ((self.downloaded_bytes as f64 / total as f64) * 100.0) as f32
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            }
        }
    }

    pub fn formatted_size_progress(&self) -> String {
        let downloaded = format_bytes(self.downloaded_bytes);
        match self.total_bytes {
            Some(total) => format!("{} / {}", downloaded, format_bytes(total)),
            None => format!("{} / Unknown", downloaded),
        }
    }

    pub fn formatted_total_size(&self) -> String {
        match self.total_bytes {
            Some(total) => format_bytes(total),
            None => "Unknown size".to_string(),
        }
    }

    pub fn formatted_downloaded_size(&self) -> String {
        format_bytes(self.downloaded_bytes)
    }

    pub fn formatted_created_date(&self) -> String {
        if self.created_at == 0 {
            return "Just now".to_string();
        }
        if let Some(dt) = chrono::DateTime::from_timestamp(self.created_at as i64, 0) {
            let local: chrono::DateTime<chrono::Local> = chrono::DateTime::from(dt);
            local.format("%b %d, %Y %I:%M %p").to_string()
        } else {
            "Unknown".to_string()
        }
    }

    pub fn http_meta(&self) -> Option<&HttpMetadata> {
        match &self.download_type {
            DownloadType::Http(meta) => Some(meta),
            _ => None,
        }
    }

    pub fn http_meta_mut(&mut self) -> Option<&mut HttpMetadata> {
        match &mut self.download_type {
            DownloadType::Http(meta) => Some(meta),
            _ => None,
        }
    }

    pub fn torrent_meta(&self) -> Option<&TorrentMetadata> {
        match &self.download_type {
            DownloadType::Torrent(meta) => Some(meta),
            _ => None,
        }
    }

    pub fn torrent_meta_mut(&mut self) -> Option<&mut TorrentMetadata> {
        match &mut self.download_type {
            DownloadType::Torrent(meta) => Some(meta),
            _ => None,
        }
    }

    pub fn get_url(&self) -> &str {
        match &self.download_type {
            DownloadType::Http(http) => &http.primary_url.url,
            DownloadType::Torrent(torrent) => &torrent.magnet_uri,
        }
    }

    pub fn active_mirrors_count(&self) -> usize {
        match &self.download_type {
            DownloadType::Http(http) => http.mirror_urls.iter().filter(|m| m.is_active).count(),
            DownloadType::Torrent(_) => 0,
        }
    }

    pub fn total_mirrors_count(&self) -> usize {
        match &self.download_type {
            DownloadType::Http(http) => http.mirror_urls.len(),
            DownloadType::Torrent(_) => 0,
        }
    }

    pub fn is_folder(&self) -> bool {
        match &self.download_type {
            DownloadType::Torrent(torrent) => {
                torrent.is_folder || torrent.selected_files.as_ref().map(|f| f.len() > 1).unwrap_or(false)
            }
            DownloadType::Http(_) => false,
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_speed(bytes_per_sec: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec))
}

/// Formats transfer speed (bytes per second) into (number_str, unit_str).
///
/// Formats the numeric portion with 1 decimal place (e.g., "12.3", "0.0", "999.9")
/// to ensure consistent spacing for 3 digits and 1 decimal digit (XXX.X) plus unit.
pub fn format_speed_parts(bytes_per_sec: u64) -> (String, &'static str) {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes_per_sec >= TB {
        (format!("{:.1}", bytes_per_sec as f64 / TB as f64), "TB/s")
    } else if bytes_per_sec >= GB {
        (format!("{:.1}", bytes_per_sec as f64 / GB as f64), "GB/s")
    } else if bytes_per_sec >= MB {
        (format!("{:.1}", bytes_per_sec as f64 / MB as f64), "MB/s")
    } else if bytes_per_sec >= KB {
        (format!("{:.1}", bytes_per_sec as f64 / KB as f64), "KB/s")
    } else if bytes_per_sec > 0 {
        (format!("{:.1}", bytes_per_sec as f64), "B/s")
    } else {
        ("0.0".to_string(), "B/s")
    }
}

pub fn format_eta(eta_secs: u64) -> String {
    if eta_secs == 0 {
        "0s".to_string()
    } else if eta_secs < 60 {
        format!("{}s", eta_secs)
    } else if eta_secs < 3600 {
        let mins = eta_secs / 60;
        let secs = eta_secs % 60;
        format!("{}m {:02}s", mins, secs)
    } else {
        let hours = eta_secs / 3600;
        let mins = (eta_secs % 3600) / 60;
        format!("{}h {:02}m", hours, mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_speed_parts() {
        assert_eq!(format_speed_parts(0), ("0.0".to_string(), "B/s"));
        assert_eq!(format_speed_parts(500), ("500.0".to_string(), "B/s"));
        assert_eq!(format_speed_parts(1024), ("1.0".to_string(), "KB/s"));
        assert_eq!(format_speed_parts(1536), ("1.5".to_string(), "KB/s"));
        assert_eq!(format_speed_parts(100 * 1024), ("100.0".to_string(), "KB/s"));
        assert_eq!(format_speed_parts(10 * 1024 * 1024), ("10.0".to_string(), "MB/s"));
        assert_eq!(format_speed_parts(157286400), ("150.0".to_string(), "MB/s"));
        assert_eq!(format_speed_parts(1610612736), ("1.5".to_string(), "GB/s"));
    }

    #[test]
    fn test_is_folder() {
        let http_item = DownloadItem {
            id: 1,
            filename: "file.mp4".to_string(),
            download_type: DownloadType::Http(HttpMetadata {
                primary_url: DownloadUrl::new("http://example.com/file.mp4"),
                mirror_urls: vec![],
                resumable: true,
                etag: None,
                last_modified: None,
                chunks: vec![],
            }),
            save_path: "".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            state: DownloadState::Completed,
            file_type: FileType::Media,
            is_scheduled: false,
            max_connections: 8,
            speed_limit_bps: None,
            sha256_hash: None,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
        };
        assert!(!http_item.is_folder());

        let mut torrent_item = DownloadItem {
            id: 2,
            filename: "Series (Season 1)".to_string(),
            download_type: DownloadType::Torrent(TorrentMetadata {
                magnet_uri: "magnet:?...".to_string(),
                info_hash: None,
                selected_files: Some(vec![0, 1, 2]),
                is_folder: true,
                peers_connected: 0,
                seeds_connected: 0,
                upload_speed_bps: 0,
            }),
            save_path: "".to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            state: DownloadState::Completed,
            file_type: FileType::Media,
            is_scheduled: false,
            max_connections: 8,
            speed_limit_bps: None,
            sha256_hash: None,
            created_at: 0,
            updated_at: 0,
            completed_at: None,
        };
        assert!(torrent_item.is_folder());

        if let DownloadType::Torrent(ref mut t) = torrent_item.download_type {
            t.is_folder = false;
            t.selected_files = Some(vec![0]);
        }
        assert!(!torrent_item.is_folder());
    }
}

