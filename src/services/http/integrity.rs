//! # File Integrity and Validation Module
//!
//! Provides post-download validation routines to guarantee downloaded artifacts are not
//! corrupted, truncated, or tampered with.
//!
//! Includes:
//! 1. Full-file SHA-256 cryptographic checksum calculation and validation.
//! 2. Structural verification of ZIP archives by validating the End of Central Directory (EOCD) header.

use crate::models::download::FileType;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Computes the SHA-256 hex digest of the target file asynchronously in a blocking thread pool.
pub async fn compute_sha256<P: AsRef<Path>>(path: P) -> Result<String, String> {
    let path_buf = path.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let mut file = File::open(&path_buf)
            .map_err(|e| format!("Failed to open file for SHA-256 verification: {}", e))?;

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024]; // 64KB read chunks for high I/O throughput

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| format!("Read error during checksum computation: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let hash_bytes = hasher.finalize();
        Ok(format!("{:x}", hash_bytes))
    })
    .await
    .map_err(|e| format!("Join error during SHA-256 computation: {}", e))?
}

/// Verifies that the file at `path` matches `expected_hex` SHA-256 digest (case-insensitive).
pub async fn verify_sha256<P: AsRef<Path>>(path: P, expected_hex: &str) -> Result<bool, String> {
    let computed = compute_sha256(path).await?;
    Ok(computed.eq_ignore_ascii_case(expected_hex.trim()))
}

/// Structural validator for ZIP archives.
///
/// Every valid standard ZIP archive ends with an **End of Central Directory Record (EOCD)**.
/// The magic signature for EOCD is `0x06054B50` (`[0x50, 0x4B, 0x05, 0x06]` in little-endian order).
/// If a download was truncated or aborted prematurely, the EOCD record will be missing or misplaced.
pub async fn verify_zip_eocd<P: AsRef<Path>>(path: P) -> Result<bool, String> {
    let path_buf: PathBuf = path.as_ref().to_path_buf();

    tokio::task::spawn_blocking(move || {
        let mut file = File::open(&path_buf)
            .map_err(|e| format!("Failed to open file for ZIP validation: {}", e))?;

        let file_len = file
            .metadata()
            .map_err(|e| format!("Failed to read file metadata: {}", e))?
            .len();

        // Minimum valid ZIP file size with empty archive and EOCD is 22 bytes
        if file_len < 22 {
            return Ok(false);
        }

        // An EOCD record can have an optional trailing comment of up to 65,535 bytes.
        // We inspect up to the last 65KB + 22 bytes of the file.
        let max_eocd_search = 65_535 + 22;
        let search_len = std::cmp::min(file_len, max_eocd_search) as usize;

        file.seek(SeekFrom::End(-(search_len as i64)))
            .map_err(|e| format!("Seek failed in ZIP validator: {}", e))?;

        let mut buffer = vec![0u8; search_len];
        file.read_exact(&mut buffer)
            .map_err(|e| format!("Read failed in ZIP validator: {}", e))?;

        // Search backward for the EOCD magic signature: 0x50, 0x4B, 0x05, 0x06 ("PK\x05\x06")
        for i in (0..=search_len.saturating_sub(4)).rev() {
            if buffer[i..i + 4] == [0x50, 0x4B, 0x05, 0x06] {
                return Ok(true);
            }
        }

        Ok(false)
    })
    .await
    .map_err(|e| format!("Join error in ZIP EOCD validator: {}", e))?
}

/// Dispatches automatic structural checks based on detected file type.
pub async fn verify_structure<P: AsRef<Path>>(
    path: P,
    file_type: FileType,
) -> Result<bool, String> {
    let path_ref = path.as_ref();
    let ext = path_ref
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match file_type {
        FileType::Archive if ext == "zip" => verify_zip_eocd(path_ref).await,
        _ => {
            // For general file types without format checks, ensure file exists and is non-empty
            if let Ok(meta) = std::fs::metadata(path_ref) {
                Ok(meta.len() > 0)
            } else {
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_sha256_verification() {
        let temp_dir = std::env::temp_dir().join(format!(
            "qdm_test_sha_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let file_path = temp_dir.join("test_hash.txt");

        // SHA-256 of "hello world" (11 bytes without newline) is b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let data = b"hello world";
        {
            let mut file = File::create(&file_path).unwrap();
            file.write_all(data).unwrap();
            file.flush().unwrap();
        }

        let expected_hash = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        let is_valid = verify_sha256(&file_path, expected_hash).await.unwrap();
        assert!(is_valid, "SHA-256 should match expected hash");

        let is_invalid = verify_sha256(
            &file_path,
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .await
        .unwrap();
        assert!(!is_invalid, "SHA-256 should not match bogus hash");

        let _ = std::fs::remove_file(&file_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_zip_eocd_verification() {
        let temp_dir = std::env::temp_dir().join(format!(
            "qdm_test_zip_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let valid_zip = temp_dir.join("valid.zip");
        let invalid_zip = temp_dir.join("corrupt.zip");

        // Minimal valid empty ZIP file (22 bytes End of Central Directory)
        // Magic bytes: 50 4b 05 06 followed by 18 zeros
        let mut eocd = vec![0x50, 0x4B, 0x05, 0x06];
        eocd.extend_from_slice(&[0u8; 18]);

        let mut f1 = File::create(&valid_zip).unwrap();
        f1.write_all(&eocd).unwrap();

        let mut f2 = File::create(&invalid_zip).unwrap();
        f2.write_all(b"this is a corrupt random file that is not a zip")
            .unwrap();

        assert!(
            verify_zip_eocd(&valid_zip).await.unwrap(),
            "Valid ZIP EOCD should pass"
        );
        assert!(
            !verify_zip_eocd(&invalid_zip).await.unwrap(),
            "Corrupt ZIP without EOCD should fail"
        );

        let _ = std::fs::remove_file(&valid_zip);
        let _ = std::fs::remove_file(&invalid_zip);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
