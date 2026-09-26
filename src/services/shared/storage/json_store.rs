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

    serde_json::from_str(&content).map_err(|e| format!("Failed to parse downloads JSON: {}", e))
}

/// Saves the list of download items to `~/qdm/downloads.json` with pretty formatting.
pub fn save_downloads(downloads: &[DownloadItem]) -> Result<(), String> {
    let path = paths::get_downloads_json_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let regular_downloads: Vec<&DownloadItem> = downloads
        .iter()
        .filter(|d| !d.download_type.is_update())
        .collect();

    let json_str = serde_json::to_string_pretty(&regular_downloads)
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

    downloads.push(item.clone());
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
    use crate::models::download::{
        DownloadState, DownloadType, DownloadUrl, FileType, HttpMetadata,
    };

    fn get_test_item() -> DownloadItem {
        DownloadItem {
            id: 1,
            filename: "test.zip".to_string(),
            save_path: "/tmp".to_string(),
            downloaded_bytes: 1024,
            total_bytes: Some(2048),
            state: DownloadState::Paused {
                downloaded_bytes: 1024,
                total_bytes: Some(2048),
            },
            file_type: FileType::Archive,
            is_scheduled: false,
            max_connections: 4,
            speed_limit_bps: None,
            sha256_hash: None,
            created_at: 123456789,
            updated_at: 123456789,
            completed_at: None,
            download_type: DownloadType::Http(HttpMetadata {
                primary_url: DownloadUrl::new("http://example.com/test.zip"),
                mirror_urls: vec![],
                resumable: true,
                etag: None,
                last_modified: None,
                chunks: vec![],
            }),
        }
    }

    #[test]
    fn test_download_item_serde() {
        let item = get_test_item();

        let json = serde_json::to_string(&item).expect("Serialize item");
        let parsed: DownloadItem = serde_json::from_str(&json).expect("Deserialize item");

        assert_eq!(item.id, parsed.id);
        assert_eq!(item.filename, parsed.filename);
        let item_http = item.http_meta().unwrap();
        let parsed_http = parsed.http_meta().unwrap();
        assert_eq!(item_http.primary_url.url, parsed_http.primary_url.url);
        assert_eq!(item_http.mirror_urls.len(), 0);
        assert_eq!(item.downloaded_bytes, parsed.downloaded_bytes);
        assert_eq!(item.total_bytes, parsed.total_bytes);
        assert_eq!(item.save_path, parsed.save_path);
        assert_eq!(item.state, parsed.state);
        assert_eq!(item.file_type, parsed.file_type);
        assert_eq!(item.is_scheduled, parsed.is_scheduled);
    }

    #[test]
    fn test_settings_model_serde() {
        let settings = SettingsModel::default();
        let json = serde_json::to_string(&settings).expect("Serialize settings");
        let parsed: SettingsModel = serde_json::from_str(&json).expect("Deserialize settings");

        assert_eq!(settings.download_folder, parsed.download_folder);
        assert_eq!(
            settings.simultaneous_downloads,
            parsed.simultaneous_downloads
        );
        assert_eq!(settings.max_connections, parsed.max_connections);
        assert_eq!(settings.max_threads, parsed.max_threads);
        assert_eq!(settings.auto_retry_downloads, parsed.auto_retry_downloads);
        assert_eq!(settings.max_auto_retries, parsed.max_auto_retries);
        assert_eq!(settings.speed_limit_value, parsed.speed_limit_value);
        assert_eq!(settings.speed_limit_unit, parsed.speed_limit_unit);
        assert_eq!(settings.show_notifications, parsed.show_notifications);
        assert_eq!(settings.file_conflict_action, parsed.file_conflict_action);
        assert_eq!(settings.delete_action, parsed.delete_action);
        assert_eq!(settings.schedule, parsed.schedule);
        assert_eq!(settings.auto_check_updates, parsed.auto_check_updates);
    }

    #[test]
    fn test_download_state_scheduled_serde() {
        let state = DownloadState::Scheduled;
        let json = serde_json::to_string(&state).expect("Serialize Scheduled state");
        let parsed: DownloadState =
            serde_json::from_str(&json).expect("Deserialize Scheduled state");
        assert_eq!(state, parsed);
    }

    #[test]
    fn test_settings_model_with_custom_actions_serde() {
        use crate::views::settings::settings::{DeleteAction, FileConflictAction};
        let settings = SettingsModel {
            file_conflict_action: Some(FileConflictAction::AutoRename),
            delete_action: Some(DeleteAction::DeleteFromDisk),
            ..Default::default()
        };

        let json = serde_json::to_string(&settings).expect("Serialize settings");
        let parsed: SettingsModel = serde_json::from_str(&json).expect("Deserialize settings");

        assert_eq!(
            parsed.file_conflict_action,
            Some(FileConflictAction::AutoRename)
        );
        assert_eq!(parsed.delete_action, Some(DeleteAction::DeleteFromDisk));
    }

    #[test]
    fn test_generate_unique_filename() {
        let temp_dir = std::env::temp_dir().join("qdm_unique_name_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let temp_dir_str = temp_dir.to_string_lossy().to_string();

        let base_file = "test_archive.zip";
        let path1 = temp_dir.join(base_file);
        let _ = std::fs::write(&path1, b"first");

        let unique1 = crate::core::utils::paths::generate_unique_filename(&temp_dir_str, base_file);
        assert_eq!(unique1, "test_archive (1).zip");

        let path2 = temp_dir.join(&unique1);
        let _ = std::fs::write(&path2, b"second");

        let unique2 = crate::core::utils::paths::generate_unique_filename(&temp_dir_str, base_file);
        assert_eq!(unique2, "test_archive (2).zip");

        // Test conflict resolution when a .qdmdownload in-progress file exists
        let path3 = temp_dir.join("test_archive (2).zip.qdmdownload");
        let _ = std::fs::write(&path3, b"third in progress");

        let unique3 = crate::core::utils::paths::generate_unique_filename(&temp_dir_str, base_file);
        assert_eq!(unique3, "test_archive (3).zip");

        assert!(crate::core::utils::paths::file_exists_or_downloading(
            &temp_dir_str,
            "test_archive (2).zip"
        ));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
