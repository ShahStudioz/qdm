
#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    
    #[test]
    fn test_torrent_folder_output_path() {
        let save_path = "test_downloads".to_string();
        let filename = "TestFolder".to_string();
        let id = 123;
        
        let mut staging_dir = PathBuf::from(&save_path);
        staging_dir.push(format!(".qdmdownload_{}", id));
        
        let final_dir = PathBuf::from(&save_path);
        
        let is_folder = true;
        let play_media = false; // staging ON
        
        let (source, target) = if play_media {
            let mut src = staging_dir.clone();
            if is_folder { src.push(&filename); }
            let mut tgt = final_dir.clone();
            if is_folder { tgt.push(&filename); }
            (src, tgt)
        } else {
            let mut src = final_dir.clone();
            if is_folder { src.push(&filename); }
            let mut tgt = staging_dir.clone();
            if is_folder { tgt.push(&filename); }
            (src, tgt)
        };
        
        assert_eq!(source, PathBuf::from("test_downloads/TestFolder"));
        assert_eq!(target, PathBuf::from("test_downloads/.qdmdownload_123/TestFolder"));
    }

    #[test]
    fn test_torrent_single_file_output_path() {
        let save_path = "test_downloads".to_string();
        let filename = "test.mkv".to_string();
        let id = 124;
        
        let mut staging_dir = PathBuf::from(&save_path);
        staging_dir.push(format!(".qdmdownload_{}", id));
        
        let final_dir = PathBuf::from(&save_path);
        
        let is_folder = false;
        let play_media = true; // staging OFF
        
        let (source, target) = if play_media {
            let mut src = staging_dir.clone();
            if is_folder { src.push(&filename); }
            let mut tgt = final_dir.clone();
            if is_folder { tgt.push(&filename); }
            (src, tgt)
        } else {
            let mut src = final_dir.clone();
            if is_folder { src.push(&filename); }
            let mut tgt = staging_dir.clone();
            if is_folder { tgt.push(&filename); }
            (src, tgt)
        };
        
        assert_eq!(source, PathBuf::from("test_downloads/.qdmdownload_124"));
        assert_eq!(target, PathBuf::from("test_downloads"));
    }
}

