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
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    Completed,
    Paused {
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
        if lower.ends_with(".zip")
            || lower.ends_with(".tar")
            || lower.ends_with(".gz")
            || lower.ends_with(".7z")
            || lower.ends_with(".rar")
            || lower.ends_with(".iso")
        {
            FileType::Archive
        } else if lower.ends_with(".mp4")
            || lower.ends_with(".mkv")
            || lower.ends_with(".avi")
            || lower.ends_with(".mp3")
            || lower.ends_with(".wav")
            || lower.ends_with(".png")
            || lower.ends_with(".jpg")
            || lower.ends_with(".jpeg")
        {
            FileType::Media
        } else if lower.ends_with(".rs")
            || lower.ends_with(".py")
            || lower.ends_with(".js")
            || lower.ends_with(".ts")
            || lower.ends_with(".html")
            || lower.ends_with(".css")
            || lower.ends_with(".sql")
            || lower.ends_with(".msi")
            || lower.ends_with(".exe")
        {
            FileType::Code
        } else if lower.ends_with(".pdf")
            || lower.ends_with(".doc")
            || lower.ends_with(".docx")
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
pub struct DownloadItem {
    pub id: usize,
    pub filename: String,
    pub primary_url: DownloadUrl,
    #[serde(default)]
    pub mirror_urls: Vec<DownloadUrl>,
    #[serde(default)]
    pub save_path: String,
    #[serde(default)]
    pub downloaded_bytes: u64,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    pub state: DownloadState,
    pub file_type: FileType,
    #[serde(default)]
    pub resumable: bool,
    #[serde(default = "default_connections")]
    pub max_connections: u32,
    #[serde(default)]
    pub speed_limit_bps: Option<u64>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub sha256_hash: Option<String>,
    #[serde(default)]
    pub chunks: Vec<ChunkState>,
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
            DownloadState::FetchingMetadata | DownloadState::Queued => 0.0,
            DownloadState::Downloading { downloaded_bytes, total_bytes, .. }
            | DownloadState::Paused { downloaded_bytes, total_bytes }
            | DownloadState::Failed { downloaded_bytes, total_bytes, .. } => {
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

    pub fn get_url(&self) -> &str {
        &self.primary_url.url
    }

    pub fn active_mirrors_count(&self) -> usize {
        self.mirror_urls.iter().filter(|m| m.is_active).count()
    }

    pub fn total_mirrors_count(&self) -> usize {
        self.mirror_urls.len()
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
