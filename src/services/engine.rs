use crate::models::download::{DownloadItem, DownloadType};
use crate::services::torrent::engine::TorrentEngine;
use crate::services::http::metadata::FileMetadata;
use crate::services::http::EngineUiEvent;

use tokio::sync::broadcast;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppEngine {
    pub http: crate::services::http::DownloadEngine,
    pub torrent: Arc<tokio::sync::RwLock<Option<TorrentEngine>>>,
    pub ui_event_tx: broadcast::Sender<EngineUiEvent>,
}

#[derive(Debug, Clone)]
pub enum ProbeResult {
    Http(FileMetadata),
    Torrent(TorrentInfo),
}

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub name: String,
    pub files: Vec<TorrentFileInfo>,
    pub is_folder: bool,
}

#[derive(Debug, Clone)]
pub struct TorrentFileInfo {
    pub id: usize,
    pub path: String,
    pub size: u64,
}

impl AppEngine {
    pub fn new(http: crate::services::http::DownloadEngine) -> Self {
        let (ui_event_tx, _) = broadcast::channel(512);

        // Forward HTTP events to unified ui_event_tx
        let mut http_rx = http.subscribe();
        let tx_clone = ui_event_tx.clone();
        tokio::spawn(async move {
            while let Ok(evt) = http_rx.recv().await {
                let _ = tx_clone.send(evt);
            }
        });

        Self {
            http,
            torrent: Arc::new(tokio::sync::RwLock::new(None)),
            ui_event_tx,
        }
    }
    
    pub async fn init_torrent(&self, dir: std::path::PathBuf) {
        let mut lock = self.torrent.write().await;
        if lock.is_none() {
            if let Ok(engine) = TorrentEngine::new(dir, self.ui_event_tx.clone()).await {
                *lock = Some(engine);
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineUiEvent> {
        self.ui_event_tx.subscribe()
    }

    pub async fn start_or_resume(&self, item: DownloadItem, play_media: bool) {
        match item.download_type {
            DownloadType::Http(_) => self.http.start_or_resume(item).await,
            DownloadType::Torrent(_) => {
                if let Some(engine) = self.torrent.read().await.as_ref() {
                    let _ = engine.start_or_resume(&item, play_media).await;
                }
            }
        }
    }

    pub async fn pause(&self, id: usize) {
        self.http.pause(id).await;
        if let Some(engine) = self.torrent.read().await.as_ref() {
            engine.pause(id).await;
        }
    }

    pub async fn cancel(&self, id: usize) {
        self.http.cancel(id).await;
        if let Some(engine) = self.torrent.read().await.as_ref() {
            engine.cancel(id).await;
        }
    }

    pub async fn probe_metadata(&self, url: &str) -> Result<ProbeResult, String> {
        if url.starts_with("magnet:") {
            let engine_opt = self.torrent.read().await;
            if let Some(engine) = engine_opt.as_ref() {
                let resp = engine.probe_metadata(url).await?;
                if let librqbit::AddTorrentResponse::ListOnly(list) = resp {
                    let name = list.info.name().as_ref().map(|s| s.to_string()).unwrap_or_else(|| "Unknown Torrent".to_string());
                    let mut files = Vec::new();
                    for (idx, f) in list.info.iter_file_details().enumerate() {
                        files.push(TorrentFileInfo {
                            id: idx,
                            path: f.filename.to_string(),
                            size: f.len,
                        });
                    }
                    let is_folder = files.len() > 1;
                    
                    return Ok(ProbeResult::Torrent(TorrentInfo { name, files, is_folder }));
                }
                Err("Failed to resolve magnet link".to_string())
            } else {
                Err("Torrent engine not initialized".to_string())
            }
        } else {
            let meta = self.http.probe_metadata(url).await?;
            Ok(ProbeResult::Http(meta))
        }
    }
}

impl std::fmt::Debug for AppEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppEngine").finish_non_exhaustive()
    }
}
