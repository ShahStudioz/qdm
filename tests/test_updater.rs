use qdm::core::utils::paths;
use qdm::services::updater::{check_for_updates, verify_file_sha256};

#[tokio::test]
async fn test_version_check_live_endpoint() {
    let api_url = "https://qdm.shahstudioz.store/api/v1/version-check";
    // Check with older version "1.0.2"
    let result = check_for_updates(api_url, "1.0.2").await;
    assert!(result.is_ok(), "API check failed: {:?}", result.err());

    let maybe_update = result.unwrap();
    assert!(
        maybe_update.is_some(),
        "Expected update to be available for 1.0.2"
    );

    let info = maybe_update.unwrap();
    assert_eq!(info.version, "v1.0.3");
    assert!(info.checksum_sha256.is_some());
    assert!(!info.download_url.is_empty());
    assert!(info.file_name.starts_with("qdm-update-v1.0.3-"));

    // Check with equal or newer version
    let up_to_date = check_for_updates(api_url, "1.0.3").await;
    assert!(up_to_date.is_ok());
    assert!(
        up_to_date.unwrap().is_none(),
        "Expected no update for current 1.0.3"
    );
}

#[tokio::test]
async fn test_update_download_target_path_contract() {
    let updates_dir = paths::get_updates_dir();
    let file_name = "qdm-update-test-v1.0.3-x64.exe";
    let target_file = updates_dir.join(file_name);

    // Ensure the save_path given to the download engine is the DIRECTORY, not the file
    let save_path = updates_dir.to_string_lossy().to_string();
    assert_eq!(std::path::Path::new(&save_path), updates_dir.as_path());

    // When the download engine runs with (save_path, filename):
    let engine_computed_target = std::path::Path::new(&save_path).join(file_name);
    assert_eq!(engine_computed_target, target_file);

    // Verify file creation and hash check
    let dummy_payload = b"QDM_TEST_PAYLOAD_FOR_UPDATE_VERIFICATION";
    std::fs::write(&target_file, dummy_payload).unwrap();
    assert!(target_file.is_file());

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(dummy_payload);
    let expected_hash = format!("{:x}", hasher.finalize());

    assert!(verify_file_sha256(&target_file, &expected_hash));

    // Cleanup
    let _ = std::fs::remove_file(&target_file);
}

#[tokio::test]
async fn test_real_update_download_and_verify() {
    let api_url = "https://qdm.shahstudioz.store/api/v1/version-check";
    let info = check_for_updates(api_url, "1.0.2")
        .await
        .expect("API check failed")
        .expect("No update returned");

    let updates_dir = paths::get_updates_dir();
    let target_file = updates_dir.join(&info.file_name);
    if target_file.exists() {
        if target_file.is_dir() {
            let _ = std::fs::remove_dir_all(&target_file);
        } else {
            let _ = std::fs::remove_file(&target_file);
        }
    }

    let save_path = updates_dir.to_string_lossy().to_string();
    let download_id = 99999;
    let primary_url = qdm::models::download::DownloadUrl::new(info.download_url.clone());
    let http_meta = qdm::models::download::HttpMetadata {
        primary_url,
        mirror_urls: Vec::new(),
        resumable: true,
        etag: None,
        last_modified: None,
        chunks: Vec::new(),
    };

    let item = qdm::models::download::DownloadItem {
        id: download_id,
        filename: info.file_name.clone(),
        download_type: qdm::models::download::DownloadType::Update(http_meta),
        save_path,
        downloaded_bytes: 0,
        total_bytes: info.file_size_bytes,
        state: qdm::models::download::DownloadState::Downloading {
            downloaded_bytes: 0,
            total_bytes: info.file_size_bytes,
            speed_bps: 0,
            eta_secs: None,
        },
        file_type: qdm::models::download::FileType::from_filename(&info.file_name),
        is_scheduled: false,
        max_connections: 4,
        speed_limit_bps: None,
        sha256_hash: info.checksum_sha256.clone(),
        created_at: 0,
        updated_at: 0,
        completed_at: None,
    };

    let engine = qdm::services::http::engine::DownloadEngine::new();
    let mut rx = engine.subscribe();
    engine.start_or_resume(item).await;

    let timeout = tokio::time::sleep(std::time::Duration::from_secs(45));
    tokio::pin!(timeout);

    let mut download_succeeded = false;
    while !download_succeeded {
        tokio::select! {
            _ = &mut timeout => {
                panic!("Update download timed out after 45 seconds");
            }
            event = rx.recv() => {
                match event {
                    Ok(qdm::services::http::engine::EngineUiEvent::DownloadCompleted { id, sha256 })
                        if id == download_id =>
                    {
                        println!("Update download completed! sha256 = {:?}", sha256);
                        download_succeeded = true;
                    }
                    Ok(qdm::services::http::engine::EngineUiEvent::DownloadFailed { id, error })
                        if id == download_id =>
                    {
                        panic!("Update download failed: {}", error);
                    }
                    _ => {}
                }
            }
        }
    }

    assert!(download_succeeded);
    assert!(
        target_file.is_file(),
        "Target file must be a regular file on disk"
    );
    assert!(target_file.metadata().unwrap().len() > 0);

    // Verify file checksum with updater verify_file_sha256
    let expected_sha = info.checksum_sha256.expect("Expected sha256");
    assert!(
        verify_file_sha256(&target_file, &expected_sha),
        "Downloaded update file must pass SHA-256 verification"
    );

    // Clean up
    let _ = std::fs::remove_file(&target_file);
}
