//! # Auto-Update Service
//!
//! Handles querying the QDM update API endpoint, parsing release metadata,
//! managing update file persistence in `~/.qdm/updates/`, verifying cryptographic
//! checksums, and orchestrating application relaunch upon installation.

use crate::core::utils::{paths, platform};
use crate::core::version::{APP_USER_AGENT, APP_VERSION};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Server response schema from `GET /api/v1/version-check`.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct VersionCheckResponse {
    pub status: String,
    pub message: Option<String>,
    #[serde(default)]
    pub update_available: Option<bool>,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
    pub release_title: Option<String>,
    pub released_at: Option<String>,
    pub changelog: Option<String>,
    pub release_notes_url: Option<String>,
    pub matched_platform: Option<String>,
    pub matched_arch: Option<String>,
    pub download_url: Option<String>,
    pub download: Option<DownloadAssetPayload>,
    #[serde(default)]
    pub downloads: Vec<DownloadAssetPayload>,
}

/// Download asset payload returned by the version-check endpoint.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DownloadAssetPayload {
    pub id: Option<usize>,
    pub platform: Option<String>,
    pub platform_label: Option<String>,
    pub architecture: Option<String>,
    pub file_format: Option<String>,
    pub download_url: Option<String>,
    pub direct_url: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub formatted_size: Option<String>,
    pub checksum_sha256: Option<String>,
}

/// Normalized release update information used by QDM client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub title: String,
    pub changelog: String,
    pub released_at: Option<String>,
    pub download_url: String,
    pub direct_url: Option<String>,
    pub file_name: String,
    pub file_format: String,
    pub file_size_bytes: Option<u64>,
    pub formatted_size: Option<String>,
    pub checksum_sha256: Option<String>,
}

/// Persistent cache model saved in `~/.qdm/update_cache.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedUpdate {
    pub info: UpdateInfo,
    pub file_path: PathBuf,
    pub saved_at: u64,
}

/// Current state of the auto-update subsystem.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        checked_at: u64,
        latest_version: String,
    },
    UpdateAvailable {
        info: UpdateInfo,
        checked_at: u64,
    },
    Downloading {
        info: UpdateInfo,
        download_id: usize,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        speed_bps: u64,
        eta_secs: Option<u64>,
    },
    ReadyToInstall {
        info: UpdateInfo,
        file_path: PathBuf,
        file_size: u64,
    },
    Error {
        message: String,
    },
}

impl Default for UpdateStatus {
    fn default() -> Self {
        UpdateStatus::Idle
    }
}

#[allow(dead_code)]
impl UpdateStatus {
    pub fn is_update_available(&self) -> bool {
        matches!(self, UpdateStatus::UpdateAvailable { .. })
    }

    pub fn is_downloading(&self) -> bool {
        matches!(self, UpdateStatus::Downloading { .. })
    }

    pub fn is_ready_to_install(&self) -> bool {
        matches!(self, UpdateStatus::ReadyToInstall { .. })
    }

    pub fn info(&self) -> Option<&UpdateInfo> {
        match self {
            UpdateStatus::UpdateAvailable { info, .. } => Some(info),
            UpdateStatus::Downloading { info, .. } => Some(info),
            UpdateStatus::ReadyToInstall { info, .. } => Some(info),
            _ => None,
        }
    }
}

/// Queries the remote version-check API endpoint for available updates.
pub async fn check_for_updates(
    api_url: &str,
    current_version: &str,
) -> Result<Option<UpdateInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(APP_USER_AGENT)
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let os = platform::current_platform();
    let arch = platform::current_arch();

    let full_url = if api_url.contains('?') {
        format!(
            "{}&os={}&arch={}&current_version={}",
            api_url, os, arch, current_version
        )
    } else {
        format!(
            "{}?os={}&arch={}&current_version={}",
            api_url, os, arch, current_version
        )
    };

    let response = client
        .get(&full_url)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Update server returned HTTP status {}",
            response.status()
        ));
    }

    let payload: VersionCheckResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse version check response: {}", e))?;

    if payload.update_available != Some(true) {
        return Ok(None);
    }

    let latest_ver = payload
        .latest_version
        .unwrap_or_else(|| "unknown".to_string());

    // Resolve matching download asset
    let asset = payload.download.or_else(|| {
        payload
            .downloads
            .into_iter()
            .find(|d| d.platform.as_deref() == Some(os))
    });

    let (download_url, direct_url, file_format, file_size, formatted_size, checksum) = match asset {
        Some(a) => {
            let url = a.download_url.or(payload.download_url).unwrap_or_default();
            let direct = a.direct_url;
            let format = a.file_format.unwrap_or_else(|| derive_format_from_os(os));
            (
                url,
                direct,
                format,
                a.file_size_bytes,
                a.formatted_size,
                a.checksum_sha256,
            )
        }
        None => {
            let url = payload.download_url.unwrap_or_default();
            let format = derive_format_from_os(os);
            (url, None, format, None, None, None)
        }
    };

    if download_url.is_empty() {
        return Ok(None);
    }

    let file_name = format!("qdm-update-{}-{}.{}", latest_ver, arch, file_format);

    Ok(Some(UpdateInfo {
        version: latest_ver,
        title: payload
            .release_title
            .unwrap_or_else(|| "QDM Update".to_string()),
        changelog: payload
            .changelog
            .unwrap_or_else(|| "No release notes provided.".to_string()),
        released_at: payload.released_at,
        download_url,
        direct_url,
        file_name,
        file_format,
        file_size_bytes: file_size,
        formatted_size,
        checksum_sha256: checksum,
    }))
}

/// Checks if an existing downloaded update is stored in `~/.qdm/updates/` and valid.
pub fn load_cached_update() -> Option<(UpdateInfo, PathBuf)> {
    let cache_file = paths::get_update_cache_path();
    if !cache_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&cache_file).ok()?;
    let cached: CachedUpdate = serde_json::from_str(&content).ok()?;

    // Ensure the cached version is strictly newer than current app version
    if !is_version_greater(&cached.info.version, APP_VERSION) {
        let _ = std::fs::remove_file(&cache_file);
        if cached.file_path.exists() {
            let _ = std::fs::remove_file(&cached.file_path);
        }
        return None;
    }

    // Verify the downloaded file actually exists on disk and is non-empty
    if !cached.file_path.exists() {
        let _ = std::fs::remove_file(&cache_file);
        return None;
    }

    if let Ok(meta) = std::fs::metadata(&cached.file_path) {
        if meta.len() == 0 {
            let _ = std::fs::remove_file(&cache_file);
            let _ = std::fs::remove_file(&cached.file_path);
            return None;
        }

        // If expected checksum is present, verify integrity
        if let Some(expected_sha) = &cached.info.checksum_sha256 {
            if !verify_file_sha256(&cached.file_path, expected_sha) {
                println!("[QDM Updater] Cached file failed checksum, removing invalid file.");
                let _ = std::fs::remove_file(&cache_file);
                let _ = std::fs::remove_file(&cached.file_path);
                return None;
            }
        }

        return Some((cached.info, cached.file_path));
    }

    None
}

/// Persists the downloaded update metadata to `~/.qdm/update_cache.json`.
pub fn save_cached_update(info: &UpdateInfo, file_path: &Path) -> Result<(), String> {
    let cache_file = paths::get_update_cache_path();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let cached = CachedUpdate {
        info: info.clone(),
        file_path: file_path.to_path_buf(),
        saved_at: now,
    };

    let json = serde_json::to_string_pretty(&cached)
        .map_err(|e| format!("Failed to serialize update cache: {}", e))?;

    std::fs::write(&cache_file, json)
        .map_err(|e| format!("Failed to write update cache: {}", e))?;

    Ok(())
}

/// Clears cached update files and metadata from disk.
#[allow(dead_code)]
pub fn clear_cached_update() {
    let cache_file = paths::get_update_cache_path();
    if cache_file.exists() {
        let _ = std::fs::remove_file(cache_file);
    }
}

/// Computes SHA-256 hash of a file on disk and compares it case-insensitively with expected hash.
pub fn verify_file_sha256(file_path: &Path, expected_sha256: &str) -> bool {
    let file = match File::open(file_path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(_) => return false,
        }
    }

    let calculated = format!("{:x}", hasher.finalize());
    calculated.eq_ignore_ascii_case(expected_sha256.trim())
}

/// Simple semver comparison: returns true if `ver_a` is strictly greater than `ver_b`.
pub fn is_version_greater(ver_a: &str, ver_b: &str) -> bool {
    let clean_a = ver_a.trim_start_matches(|c| c == 'v' || c == 'V');
    let clean_b = ver_b.trim_start_matches(|c| c == 'v' || c == 'V');

    let parse_parts = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|p| {
                let digits: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse::<u64>().unwrap_or(0)
            })
            .collect()
    };

    let parts_a = parse_parts(clean_a);
    let parts_b = parse_parts(clean_b);

    for i in 0..parts_a.len().max(parts_b.len()) {
        let a = parts_a.get(i).copied().unwrap_or(0);
        let b = parts_b.get(i).copied().unwrap_or(0);
        if a > b {
            return true;
        }
        if a < b {
            return false;
        }
    }

    false
}

fn derive_format_from_os(os: &str) -> String {
    match os {
        "windows" => "exe".to_string(),
        "macos" => "dmg".to_string(),
        "linux" => "AppImage".to_string(),
        _ => "bin".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_greater() {
        assert!(is_version_greater("v1.1.0", "v1.0.0"));
        assert!(is_version_greater("1.0.1", "1.0.0"));
        assert!(is_version_greater("2.0.0", "1.99.99"));
        assert!(!is_version_greater("1.0.0", "1.0.0"));
        assert!(!is_version_greater("v1.0.0", "1.0.0"));
        assert!(!is_version_greater("0.9.0", "1.0.0"));
    }

    #[test]
    fn test_sha256_verification() {
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("qdm_test_sha.bin");
        std::fs::write(&test_file, b"QDM_AUTO_UPDATE_TEST_CONTENT").unwrap();

        // SHA256 of "QDM_AUTO_UPDATE_TEST_CONTENT"
        let mut hasher = Sha256::new();
        hasher.update(b"QDM_AUTO_UPDATE_TEST_CONTENT");
        let expected = format!("{:x}", hasher.finalize());

        assert!(verify_file_sha256(&test_file, &expected));
        assert!(!verify_file_sha256(
            &test_file,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ));

        let _ = std::fs::remove_file(test_file);
    }
}
