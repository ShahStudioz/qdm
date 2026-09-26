#![allow(dead_code)]

use crate::services::http::metadata::MetadataService;
use reqwest::Client;

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
    metadata_service: MetadataService,
}

impl Default for DownloadService {
    fn default() -> Self {
        Self {
            metadata_service: MetadataService::new(Client::new()),
        }
    }
}

impl DownloadService {
    pub async fn get_file_meta_data(&self, url: &str) -> Result<DownloadFileMetaData, String> {
        let meta = self.metadata_service.probe(url).await?;
        Ok(DownloadFileMetaData {
            content_length: meta.content_length,
            content_type: meta.content_type,
            last_modified: meta.last_modified,
            content_disposition: meta.content_disposition,
            supports_resume: meta.supports_resume,
        })
    }
}
