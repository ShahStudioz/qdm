use std::path::PathBuf;

/// Returns the base application directory at `~/qdm`, creating it if it doesn't exist.
pub fn get_qdm_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let qdm_dir = home.join(".qdm");
    if !qdm_dir.exists() {
        let _ = std::fs::create_dir_all(&qdm_dir);
    }
    qdm_dir
}

/// Returns the full path to the downloads JSON file (`~/.qdm/downloads.json`).
pub fn get_downloads_json_path() -> PathBuf {
    get_qdm_dir().join("downloads.json")
}

/// Returns the full path to the settings JSON file (`~/.qdm/settings.json`).
pub fn get_settings_json_path() -> PathBuf {
    get_qdm_dir().join("settings.json")
}

/// Returns the default user download directory, falling back to `~/.qdm/downloads`.
pub fn get_default_download_dir() -> String {
    dirs::download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| {
            let path = get_qdm_dir().join("downloads");
            let _ = std::fs::create_dir_all(&path);
            path.to_string_lossy().to_string()
        })
}
