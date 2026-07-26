#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Archive,
    Code,
    Media,
    Document,
    Other,
}

#[derive(Debug, Clone)]
pub struct DownloadItem {
    pub id: usize,
    pub filename: String,
    pub url: String,
    pub size_downloaded: String,
    pub size_total: String,
    pub state: DownloadState,
    pub file_type: FileType,
}
