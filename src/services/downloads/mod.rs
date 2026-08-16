#![allow(unused_imports)]
pub mod diagnostics;
pub mod download;
pub mod engine;
pub mod integrity;
pub mod metadata;
pub mod task;
pub mod worker;
pub mod writer;

// Re-export core types for clean consumption
pub use engine::{DownloadEngine, EngineUiEvent};
pub use integrity::{compute_sha256, verify_sha256, verify_structure, verify_zip_eocd};
pub use metadata::{FileMetadata, MetadataService};
pub use writer::PositionalWriter;
