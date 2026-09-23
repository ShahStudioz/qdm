//! # Remote File Metadata Probing Module
//!
//! Queries remote HTTP endpoints with resilient multi-tier fallback (Range -> HEAD -> Stream-GET)
//! and automatic retry backoff to reliably inspect file size, resumability, ETag, Last-Modified,
//! and suggested filenames.

use std::time::Duration;
use reqwest::header::{
    ACCEPT_RANGES, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, ETAG,
    LAST_MODIFIED, RANGE,
};
use reqwest::{Client, StatusCode};

/// Metadata extracted from remote HTTP server headers.
#[derive(Debug, Clone, Default)]
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

    /// Fetches file metadata using a resilient multi-tier fallback with retries:
    /// 1. `Range: bytes=0-0` GET (Ideal for range-supporting servers)
    /// 2. `HEAD` request (For servers rejecting range probing)
    /// 3. Streaming `GET` (Headers-only stream drop for servers blocking HEAD)
    pub async fn probe(&self, url: &str) -> Result<FileMetadata, String> {
        let max_retries = 3;

        // Tier 1: Try Range bytes=0-0
        for attempt in 0..max_retries {
            match self.probe_range(url).await {
                Ok(Some(meta)) => return Ok(meta),
                Ok(None) => {
                    // Server responded with non-range error (e.g. 400/405/416/501), fallback to Tier 2
                    break;
                }
                Err(err) => {
                    if attempt + 1 < max_retries {
                        tokio::time::sleep(Duration::from_millis(300 * (1 << attempt))).await;
                    } else {
                        println!("[QDM Metadata Probe] Tier 1 Range probe failed: {}. Falling back to HEAD...", err);
                    }
                }
            }
        }

        // Tier 2: Try HEAD request
        for attempt in 0..max_retries {
            match self.probe_head(url).await {
                Ok(Some(meta)) => return Ok(meta),
                Ok(None) => {
                    // Server responded with 405/403 to HEAD, fallback to Tier 3
                    break;
                }
                Err(err) => {
                    if attempt + 1 < max_retries {
                        tokio::time::sleep(Duration::from_millis(300 * (1 << attempt))).await;
                    } else {
                        println!("[QDM Metadata Probe] Tier 2 HEAD probe failed: {}. Falling back to Stream-GET...", err);
                    }
                }
            }
        }

        // Tier 3: Try standard GET request (read headers and drop stream)
        for attempt in 0..max_retries {
            match self.probe_stream_get(url).await {
                Ok(meta) => return Ok(meta),
                Err(err) => {
                    if attempt + 1 < max_retries {
                        tokio::time::sleep(Duration::from_millis(300 * (1 << attempt))).await;
                    } else {
                        return Err(format!("All metadata probe tiers failed for {}: {}", url, err));
                    }
                }
            }
        }

        Err(format!("Metadata probe failed after multiple retry attempts for {}", url))
    }

    async fn probe_range(&self, url: &str) -> Result<Option<FileMetadata>, String> {
        let resp = self
            .client
            .get(url)
            .header(RANGE, "bytes=0-0")
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("Range request error: {}", e))?;

        let status = resp.status();
        if status == StatusCode::PARTIAL_CONTENT || status == StatusCode::OK {
            let headers = resp.headers();
            let content_type = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(String::from);
            let last_modified = headers.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(String::from);
            let etag = headers.get(ETAG).and_then(|v| v.to_str().ok()).map(String::from);
            let content_disposition = headers.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).map(String::from);

            let supports_resume = status == StatusCode::PARTIAL_CONTENT
                || headers.get(ACCEPT_RANGES).and_then(|v| v.to_str().ok()).map(|v| v.eq_ignore_ascii_case("bytes")).unwrap_or(false);

            let mut content_length = resp.content_length();
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

            Ok(Some(FileMetadata {
                content_length,
                content_type,
                last_modified,
                etag,
                content_disposition,
                supports_resume,
            }))
        } else if status == StatusCode::BAD_REQUEST
            || status == StatusCode::RANGE_NOT_SATISFIABLE
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status == StatusCode::FORBIDDEN
            || status == StatusCode::NOT_IMPLEMENTED
        {
            Ok(None)
        } else {
            Err(format!("Server returned HTTP {}", status))
        }
    }

    async fn probe_head(&self, url: &str) -> Result<Option<FileMetadata>, String> {
        let resp = self
            .client
            .head(url)
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("HEAD request error: {}", e))?;

        let status = resp.status();
        if status.is_success() {
            let headers = resp.headers();
            let content_type = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(String::from);
            let last_modified = headers.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(String::from);
            let etag = headers.get(ETAG).and_then(|v| v.to_str().ok()).map(String::from);
            let content_disposition = headers.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).map(String::from);

            let supports_resume = headers
                .get(ACCEPT_RANGES)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("bytes"))
                .unwrap_or(false);

            let mut content_length = resp.content_length();
            if let Some(len_val) = headers.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()) {
                if let Ok(len) = len_val.trim().parse::<u64>() {
                    content_length = Some(len);
                }
            }

            Ok(Some(FileMetadata {
                content_length,
                content_type,
                last_modified,
                etag,
                content_disposition,
                supports_resume,
            }))
        } else if status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::FORBIDDEN {
            Ok(None)
        } else {
            Err(format!("Server returned HTTP {}", status))
        }
    }

    async fn probe_stream_get(&self, url: &str) -> Result<FileMetadata, String> {
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("Stream GET error: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!("Server returned HTTP {}", status));
        }

        let headers = resp.headers();
        let content_type = headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).map(String::from);
        let last_modified = headers.get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(String::from);
        let etag = headers.get(ETAG).and_then(|v| v.to_str().ok()).map(String::from);
        let content_disposition = headers.get(CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()).map(String::from);

        let supports_resume = headers
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.eq_ignore_ascii_case("bytes"))
            .unwrap_or(false);

        let mut content_length = resp.content_length();
        if let Some(len_val) = headers.get(CONTENT_LENGTH).and_then(|v| v.to_str().ok()) {
            if let Ok(len) = len_val.trim().parse::<u64>() {
                content_length = Some(len);
            }
        }

        // Dropping `resp` closes the HTTP connection without downloading the response payload
        drop(resp);

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
