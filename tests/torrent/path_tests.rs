use std::path::PathBuf;

// Simulated paths for testing logic
fn get_output_folder(save_path: &str, item_id: usize, filename: &str, is_folder: bool, use_staging: bool) -> PathBuf {
    let staging_base = PathBuf::from(save_path).join(format!(".qdmdownload_{}", item_id));
    let final_base = PathBuf::from(save_path);
    
    let staging_target = staging_base.join(filename);
    let final_target = final_base.join(filename);
    
    if use_staging {
        if is_folder {
            staging_target
        } else {
            staging_base
        }
    } else {
        if is_folder {
            final_target
        } else {
            final_base
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_file_torrent_with_staging() {
        let output = get_output_folder("/downloads", 123, "MyFolder", true, true);
        assert_eq!(output, PathBuf::from("/downloads/.qdmdownload_123/MyFolder"));
    }

    #[test]
    fn test_multi_file_torrent_without_staging() {
        let output = get_output_folder("/downloads", 123, "MyFolder", true, false);
        assert_eq!(output, PathBuf::from("/downloads/MyFolder"));
    }

    #[test]
    fn test_single_file_torrent_with_staging() {
        let output = get_output_folder("/downloads", 123, "file.iso", false, true);
        assert_eq!(output, PathBuf::from("/downloads/.qdmdownload_123"));
    }

    #[test]
    fn test_single_file_torrent_without_staging() {
        let output = get_output_folder("/downloads", 123, "file.iso", false, false);
        assert_eq!(output, PathBuf::from("/downloads"));
    }
}