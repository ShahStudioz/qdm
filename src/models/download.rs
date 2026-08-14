use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DownloadState {
    Downloading {
        progress: f32, // 0.0 to 100.0
        speed: String,
        eta: String,
    },
    Completed,
    Paused {
        progress: f32,
    },
    Failed {
        progress: f32,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: usize,
    pub filename: String,
    pub url: String,
    #[serde(default)]
    pub save_path: String,
    pub size_downloaded: String,
    pub size_total: String,
    pub state: DownloadState,
    pub file_type: FileType,
    #[serde(default)]
    pub created_at: u64,
}
