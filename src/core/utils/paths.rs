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

/// Returns the update downloads directory at `~/.qdm/updates`, creating it if it doesn't exist.
pub fn get_updates_dir() -> PathBuf {
    let dir = get_qdm_dir().join("updates");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    dir
}

/// Returns the path to the update cache metadata file (`~/.qdm/update_cache.json`).
pub fn get_update_cache_path() -> PathBuf {
    get_qdm_dir().join("update_cache.json")
}

/// Returns the default user download directory, ensuring it exists on disk, falling back to `~/.qdm/downloads`.
pub fn get_default_download_dir() -> String {
    let dir = dirs::download_dir().unwrap_or_else(|| get_qdm_dir().join("downloads"));
    let _ = std::fs::create_dir_all(&dir);
    dir.to_string_lossy().to_string()
}

/// Returns true if the string is a magnet link, a .torrent URL, or a .torrent file path.
pub fn is_torrent_target(target: &str) -> bool {
    let t = target.trim();
    if t.starts_with("magnet:") {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    let path_part = lower.split('?').next().unwrap_or(&lower);
    if path_part.ends_with(".torrent") {
        return true;
    }
    let p = std::path::Path::new(t);
    p.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
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
    let ext = file_path.extension().and_then(|s| s.to_str());

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

/// Sanitizes a candidate filename to ensure it is safe for the filesystem (particularly Windows).
/// - Strips path traversal and directory separators (`/`, `\`), keeping only the leaf filename.
/// - Replaces illegal Windows characters (`<`, `>`, `:`, `"`, `/`, `\`, `|`, `?`, `*`) and control chars with `_`.
/// - Trims leading/trailing whitespace and trailing dots/spaces.
/// - Prefixes Windows reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`) with `_`.
/// - Enforces a maximum length of 255 characters while preserving the extension.
/// - Falls back to `"download.file"` if the result is empty.
pub fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "download.file".to_string();
    }

    // Strip any path traversal or leading directory components
    let leaf = trimmed
        .replace('\\', "/")
        .split('/')
        .rfind(|s| !s.is_empty() && *s != "." && *s != "..")
        .unwrap_or(trimmed)
        .to_string();

    // Replace illegal Windows filesystem characters and control chars with '_'
    // Illegal on Windows: < > : " / \ | ? * and ASCII control characters 0x00..=0x1F, 0x7F
    let mut sanitized: String = leaf
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();

    // Strip trailing dots and spaces (Windows forbids files ending with a dot or space)
    let stripped = sanitized.trim_end_matches(['.', ' ']);
    if stripped.is_empty() {
        sanitized = "download.file".to_string();
    } else {
        sanitized = stripped.to_string();
    }

    // Check against Windows reserved device names (CON, PRN, AUX, NUL, COM1..9, LPT1..9)
    let stem = sanitized.split('.').next().unwrap_or(&sanitized);
    let stem_upper = stem.to_ascii_uppercase();
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem_upper.as_str()) {
        sanitized = format!("_{}", sanitized);
    }

    // Enforce maximum single-component length (255 chars) while preserving extension
    if sanitized.len() > 255 {
        if let Some(dot_idx) = sanitized.rfind('.') {
            let ext = &sanitized[dot_idx..];
            if ext.len() < 30 {
                let keep_stem_len = 255 - ext.len();
                sanitized = format!("{}{}", &sanitized[..keep_stem_len], ext);
            } else {
                sanitized.truncate(255);
            }
        } else {
            sanitized.truncate(255);
        }
    }

    if sanitized.trim().is_empty() {
        "download.file".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_normal() {
        assert_eq!(sanitize_filename("document.pdf"), "document.pdf");
        assert_eq!(
            sanitize_filename("Big Buck Bunny.mp4"),
            "Big Buck Bunny.mp4"
        );
        assert_eq!(
            sanitize_filename("archive_v1.0.tar.gz"),
            "archive_v1.0.tar.gz"
        );
    }

    #[test]
    fn test_sanitize_filename_illegal_chars() {
        assert_eq!(
            sanitize_filename("video:part*1?file<name>|.mp4"),
            "video_part_1_file_name__.mp4"
        );
        assert_eq!(
            sanitize_filename("\"quoted_file\".zip"),
            "_quoted_file_.zip"
        );
    }

    #[test]
    fn test_sanitize_filename_trailing_dots_and_spaces() {
        assert_eq!(
            sanitize_filename("my_document.pdf...   "),
            "my_document.pdf"
        );
        assert_eq!(sanitize_filename("...."), "download.file");
        assert_eq!(sanitize_filename("    "), "download.file");
        assert_eq!(sanitize_filename(""), "download.file");
    }

    #[test]
    fn test_sanitize_filename_path_traversal() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(
            sanitize_filename("C:\\Users\\Administrator\\secret.key"),
            "secret.key"
        );
    }

    #[test]
    fn test_sanitize_filename_reserved_device_names() {
        assert_eq!(sanitize_filename("con.txt"), "_con.txt");
        assert_eq!(sanitize_filename("CON.iso"), "_CON.iso");
        assert_eq!(sanitize_filename("aux"), "_aux");
        assert_eq!(sanitize_filename("NUL.tar.gz"), "_NUL.tar.gz");
        assert_eq!(sanitize_filename("com1.dat"), "_com1.dat");
        assert_eq!(sanitize_filename("lpt3.log"), "_lpt3.log");
    }
}
