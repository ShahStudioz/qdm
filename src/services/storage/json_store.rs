use crate::core::utils::paths;
use crate::models::download::DownloadItem;
use crate::views::settings::settings::SettingsModel;
use std::time::{SystemTime, UNIX_EPOCH};

/// Loads all download items from `~/qdm/downloads.json`.
/// Returns an empty vector if the file does not exist yet.
pub fn load_downloads() -> Result<Vec<DownloadItem>, String> {
    let path = paths::get_downloads_json_path();
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read downloads file at {:?}: {}", path, e))?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse downloads JSON: {}", e))
}

/// Saves the list of download items to `~/qdm/downloads.json` with pretty formatting.
pub fn save_downloads(downloads: &[DownloadItem]) -> Result<(), String> {
    let path = paths::get_downloads_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json_str = serde_json::to_string_pretty(downloads)
        .map_err(|e| format!("Failed to serialize downloads: {}", e))?;

    std::fs::write(&path, json_str)
        .map_err(|e| format!("Failed to write downloads to {:?}: {}", path, e))?;

    Ok(())
}

/// Inserts a new download item, assigns a unique auto-incremented ID, and saves to JSON.
pub fn insert_download(mut item: DownloadItem) -> Result<DownloadItem, String> {
    let mut downloads = load_downloads().unwrap_or_default();
    let next_id = downloads.iter().map(|d| d.id).max().unwrap_or(0) + 1;
    item.id = next_id;
    if item.created_at == 0 {
        item.created_at = now_timestamp();
    }

    downloads.insert(0, item.clone());
    save_downloads(&downloads)?;

    Ok(item)
}

/// Loads the application settings from `~/qdm/settings.json`.
/// Creates and saves default settings if the file does not exist yet.
pub fn load_settings() -> Result<SettingsModel, String> {
    let path = paths::get_settings_json_path();
    if !path.exists() {
        let defaults = SettingsModel::default();
        let _ = save_settings(&defaults);
        return Ok(defaults);
    }

    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings file at {:?}: {}", path, e))?;

    if content.trim().is_empty() {
        let defaults = SettingsModel::default();
        let _ = save_settings(&defaults);
        return Ok(defaults);
    }

    let mut settings: SettingsModel = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse settings JSON: {}", e))?;

    settings.sync_animations();
    Ok(settings)
}

/// Saves the application settings to `~/qdm/settings.json`.
pub fn save_settings(settings: &SettingsModel) -> Result<(), String> {
    let path = paths::get_settings_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let json_str = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    std::fs::write(&path, json_str)
        .map_err(|e| format!("Failed to write settings to {:?}: {}", path, e))?;

    Ok(())
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::{DownloadState, DownloadUrl, FileType};

    #[test]
    fn test_download_item_serde() {
        let item = DownloadItem {
            id: 1,
            filename: "ubuntu.iso".to_string(),
            primary_url: DownloadUrl::new("https://example.com/ubuntu.iso"),
            mirror_urls: vec![DownloadUrl::new("https://mirror.example.com/ubuntu.iso")],
            save_path: "/home/user/Downloads".to_string(),
            downloaded_bytes: 524_288_000,
            total_bytes: Some(2_684_354_560),
            state: DownloadState::Downloading {
                downloaded_bytes: 524_288_000,
                total_bytes: Some(2_684_354_560),
                speed_bps: 5_242_880,
                eta_secs: Some(300),
            },
            file_type: FileType::Archive,
            resumable: true,
            max_connections: 8,
            speed_limit_bps: None,
            etag: Some("\"test-etag-12345\"".to_string()),
            last_modified: Some("Wed, 21 Oct 2025 07:28:00 GMT".to_string()),
            sha256_hash: None,
            chunks: Vec::new(),
            created_at: 1700000000,
            updated_at: 1700000010,
            completed_at: None,
        };

        let json = serde_json::to_string(&item).expect("Serialize item");
        let parsed: DownloadItem = serde_json::from_str(&json).expect("Deserialize item");

        assert_eq!(item.id, parsed.id);
        assert_eq!(item.filename, parsed.filename);
        assert_eq!(item.primary_url.url, parsed.primary_url.url);
        assert_eq!(item.mirror_urls.len(), 1);
        assert_eq!(item.downloaded_bytes, parsed.downloaded_bytes);
        assert_eq!(item.total_bytes, parsed.total_bytes);
        assert_eq!(item.save_path, parsed.save_path);
        assert_eq!(item.state, parsed.state);
        assert_eq!(item.file_type, parsed.file_type);
    }

    #[test]
    fn test_settings_model_serde() {
        let settings = SettingsModel::default();
        let json = serde_json::to_string(&settings).expect("Serialize settings");
        let parsed: SettingsModel = serde_json::from_str(&json).expect("Deserialize settings");

        assert_eq!(settings.download_folder, parsed.download_folder);
        assert_eq!(settings.simultaneous_downloads, parsed.simultaneous_downloads);
        assert_eq!(settings.max_connections, parsed.max_connections);
        assert_eq!(settings.max_threads, parsed.max_threads);
        assert_eq!(settings.retry_count, parsed.retry_count);
        assert_eq!(settings.timeout_seconds, parsed.timeout_seconds);
        assert_eq!(settings.user_agent, parsed.user_agent);
        assert_eq!(settings.show_notifications, parsed.show_notifications);
    }
}
