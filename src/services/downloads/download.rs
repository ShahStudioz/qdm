use reqwest::{
    header::{CONTENT_DISPOSITION, CONTENT_RANGE, CONTENT_TYPE, LAST_MODIFIED, RANGE},
    Client, StatusCode,
};

#[derive(Debug, Clone)]
pub struct DownloadFileMetaData {
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
    pub last_modified: Option<String>,
    pub content_disposition: Option<String>,
    pub supports_resume: bool,
}

#[derive(Debug, Clone)]
pub struct DownloadService {
    client: Client,
}

impl Default for DownloadService {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl DownloadService {
    pub async fn get_file_meta_data(&self, url: &str) -> Result<DownloadFileMetaData, String> {
        let resp = self
            .client
            .get(url)
            .header(RANGE, "bytes=0-0")
            .send()
            .await
            .map_err(|x| format!("Error while fetching file metadata: {}", x))?;

        if !resp.status().is_success() {
            return Err("Unsuccessful HTTP status while fetching metadata".to_string());
        }

        let headers = resp.headers();

        let content_type = headers
            .get(CONTENT_TYPE)
            .and_then(|val| val.to_str().ok())
            .map(|s| s.to_string());

        let last_modified = headers
            .get(LAST_MODIFIED)
            .and_then(|val| val.to_str().ok())
            .map(|s| s.to_string());

        let content_disposition = headers
            .get(CONTENT_DISPOSITION)
            .and_then(|val| val.to_str().ok())
            .map(|s| s.to_string());

        let supports_resume = resp.status() == StatusCode::PARTIAL_CONTENT;

        let mut content_length = resp.content_length();
        if supports_resume {
            if let Some(content_range) = headers.get(CONTENT_RANGE).and_then(|v| v.to_str().ok()) {
                if let Some(total_size_str) = content_range.split('/').last() {
                    if let Ok(total_size) = total_size_str.parse::<u64>() {
                        content_length = Some(total_size);
                    }
                }
            }
        }

        Ok(DownloadFileMetaData {
            content_length,
            content_type,
            last_modified,
            content_disposition,
            supports_resume,
        })
    }
}
