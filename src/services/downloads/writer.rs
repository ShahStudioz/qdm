//! # Positional Concurrent File Writer
//!
//! Provides thread-safe, concurrent file write capabilities using operating system-level
//! positional write system calls (`seek_write` on Windows, `write_all_at` on Unix).
//!
//! ## Why Positional Writes?
//! In a multi-connection download manager, multiple worker threads/tasks stream disjoint
//! byte ranges concurrently (e.g., Worker 1 writes bytes 0..10MB, Worker 2 writes bytes 10..20MB).
//! If workers shared a standard `std::io::Seek` cursor, one worker's seek would invalidate another's,
//! causing catastrophic data corruption. Positional writes write directly to an absolute offset
//! without altering any internal file pointer.

use std::fs::File;
use std::io::{self, Result as IoResult};
use std::path::Path;
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;

/// A thread-safe file handle that supports concurrent atomic writes at arbitrary byte offsets.
#[derive(Clone, Debug)]
pub struct PositionalWriter {
    file: Arc<File>,
}

impl PositionalWriter {
    /// Opens or creates the target file at `path`, optionally pre-allocating its total size.
    ///
    /// Pre-allocating the file with `set_len` serves two critical purposes:
    /// 1. Verifies upfront that the filesystem has sufficient contiguous free disk space.
    /// 2. Drastically reduces filesystem fragmentation when multiple workers write to distant offsets.
    pub fn create_preallocated<P: AsRef<Path>>(path: P, total_size: Option<u64>) -> IoResult<Self> {
        let path_ref = path.as_ref();

        // Ensure the destination folder exists
        if let Some(parent) = path_ref.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path_ref)?;

        if let Some(size) = total_size {
            if size > 0 {
                // Pre-allocate target length
                let _ = file.set_len(size);
            }
        }

        Ok(Self {
            file: Arc::new(file),
        })
    }

    /// Writes `data` starting at the absolute `offset` in the file.
    ///
    /// Does NOT modify any shared seek pointer, making it completely safe to call
    /// concurrently from arbitrary Tokio tasks or threads.
    pub fn write_at(&self, offset: u64, data: &[u8]) -> IoResult<()> {
        #[cfg(windows)]
        {
            let mut total_written = 0;
            while total_written < data.len() {
                let current_offset = offset + total_written as u64;
                let slice = &data[total_written..];
                let bytes_written = self.file.seek_write(slice, current_offset)?;
                if bytes_written == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "Failed to write bytes: OS returned 0 bytes written",
                    ));
                }
                total_written += bytes_written;
            }
            Ok(())
        }

        #[cfg(unix)]
        {
            self.file.write_all_at(data, offset)
        }
    }

    /// Flushes operating system buffers and commits file data to physical storage.
    ///
    /// Call this before committing progress into database/JSON state to ensure
    /// crash consistency and eliminate ghost progress.
    pub fn sync_data(&self) -> IoResult<()> {
        self.file.sync_data()
    }

    /// Truncates or extends the file to an explicit length if total file size changes.
    #[allow(dead_code)]
    pub fn set_len(&self, size: u64) -> IoResult<()> {
        self.file.set_len(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[tokio::test]
    async fn test_concurrent_positional_writes() {
        let temp_dir = std::env::temp_dir().join(format!("qdm_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let test_file = temp_dir.join("concurrent_write.bin");

        let total_size = 4 * 1024 * 1024; // 4MB
        let writer = PositionalWriter::create_preallocated(&test_file, Some(total_size as u64))
            .expect("Failed to create preallocated writer");

        // Spawn 4 concurrent tasks, each filling 1MB with a distinct byte pattern
        let mut handles = Vec::new();
        for i in 0..4u8 {
            let writer_clone = writer.clone();
            let handle = tokio::spawn(async move {
                let chunk_size = 1024 * 1024; // 1MB
                let offset = (i as u64) * (chunk_size as u64);
                let byte_val = b'A' + i;
                let pattern = vec![byte_val; chunk_size];

                // Write in smaller slices within the 1MB region to stress concurrent interleaving
                let slice_size = 64 * 1024;
                for slice_idx in 0..(chunk_size / slice_size) {
                    let write_offset = offset + (slice_idx * slice_size) as u64;
                    let slice = &pattern[slice_idx * slice_size..(slice_idx + 1) * slice_size];
                    writer_clone.write_at(write_offset, slice).unwrap();
                }
            });
            handles.push(handle);
        }

        for h in handles {
            h.await.unwrap();
        }

        writer.sync_data().expect("Sync failed");

        // Verify disk contents
        let mut file = File::open(&test_file).expect("Failed to open written test file");
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).expect("Failed to read test file");

        assert_eq!(buffer.len(), total_size);

        for i in 0..4u8 {
            let chunk_size = 1024 * 1024;
            let start = (i as usize) * chunk_size;
            let end = start + chunk_size;
            let expected_byte = b'A' + i;

            assert!(
                buffer[start..end].iter().all(|&b| b == expected_byte),
                "Data corruption in chunk {} (expected byte: {})",
                i,
                expected_byte as char
            );
        }

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
        let _ = std::fs::remove_dir(&temp_dir);
    }
}
