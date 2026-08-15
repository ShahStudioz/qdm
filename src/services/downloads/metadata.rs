//! # Remote File Metadata Probing Module
//!
//! Queries remote HTTP endpoints via HEAD or initial Range requests (`bytes=0-0`)
//! to inspect file size, resumability, ETag, Last-Modified, and suggested filenames.

use reqwest::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG,
    LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};

/// Metadata extracted from remote HTTP server headers.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct FileMetadata {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub last_modified: Option<String>,
    pub etag: Option<String>,
    pub content_disposition: Option<String>,
    pub supports_resume: bool,
}

/// Service for probing remote URL capabilities before initiating downloads.
#[derive(Debug, Clone)]
pub struct MetadataService {
    client: Client,
}

impl MetadataService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Fetches file metadata using a non-destructive `Range: bytes=0-0` request.
    ///
    /// This is superior to standard `HEAD` requests because many CDNs and servers
    /// return inaccurate headers or reject HEAD requests, whereas `bytes=0-0`
    /// tests both server reachability AND Range header support in a single round-trip.
    pub async fn probe(&self, url: &str) -> Result<FileMetadata, String> {
        let resp = self
            .client
            .get(url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(|e| format!("Metadata probe failed for {}: {}", url, e))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!("Server returned HTTP {} during metadata probe", status));
        }

        let headers = resp.headers();

        let content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let last_modified = headers
            .get(LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let etag = headers
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let content_disposition = headers
            .get(CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let supports_resume = status == StatusCode::PARTIAL_CONTENT
            || headers
                .get(ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("bytes"))
                .unwrap_or(false);

        let mut content_length = resp.content_length();

        // If HTTP 206 Partial Content was returned, parse total size from Content-Range: bytes 0-0/TOTAL
        if status == StatusCode::PARTIAL_CONTENT {
            if let Some(content_range) = headers.get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
                if let Some(total_str) = content_range.split('/').last() {
                    if let Ok(total) = total_str.trim().parse::<u64>() {
                        content_length = Some(total);
                    }
                }
            }
        } else if let Some(len_val) = headers.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()) {
            if let Ok(len) = len_val.trim().parse::<u64>() {
                content_length = Some(len);
            }
        }

        Ok(FileMetadata {
            content_length,
            content_type,
            last_modified,
            etag,
            content_disposition,
            supports_resume,
        })
    }
}
