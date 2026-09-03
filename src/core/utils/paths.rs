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

/// Checks if either `filename` or `filename.qdmdownload` exists in the target directory.
pub fn file_exists_or_downloading(dir: &str, filename: &str) -> bool {
    let dir_path = std::path::Path::new(dir);
    let target = dir_path.join(filename);
    let temp_target = dir_path.join(format!("{}.qdmdownload", filename));
    target.exists() || temp_target.exists()
}

/// Given a directory path and a candidate filename, returns the filename if neither it
/// nor its `.qdmdownload` variant exist, or generates a non-conflicting filename like `filename (1).ext`.
pub fn generate_unique_filename(dir: &str, filename: &str) -> String {
    if !file_exists_or_downloading(dir, filename) {
        return filename.to_string();
    }

    let file_path = std::path::Path::new(filename);
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let ext = file_path
        .extension()
        .and_then(|s| s.to_str());

    let mut counter = 1u32;
    loop {
        let candidate_name = match ext {
            Some(extension) => format!("{} ({}).{}", stem, counter, extension),
            None => format!("{} ({})", stem, counter),
        };

        if !file_exists_or_downloading(dir, &candidate_name) {
            return candidate_name;
        }
        counter += 1;
    }
}

/// Checks if a directory with the given name exists in the target directory.
/// Used for torrent folder conflict detection.
pub fn folder_exists(dir: &str, folder_name: &str) -> bool {
    let dir_path = std::path::Path::new(dir);
    let target = dir_path.join(folder_name);
    target.exists() && target.is_dir()
}

/// Generates a unique folder name if one already exists. 
/// Appends ` (1)`, ` (2)`, etc. until a non-conflicting name is found.
pub fn generate_unique_folder_name(dir: &str, folder_name: &str) -> String {
    if !folder_exists(dir, folder_name) {
        return folder_name.to_string();
    }

    let mut counter = 1u32;
    loop {
        let candidate_name = format!("{} ({})", folder_name, counter);
        if !folder_exists(dir, &candidate_name) {
            return candidate_name;
        }
        counter += 1;
    }
}
