// Library crate root - exposes only what the speed_debug binary needs.
// Excludes views/app/theme/services::storage to avoid pulling iced UI deps.

pub mod models;

pub mod services {
    pub mod downloads {
        pub mod diagnostics;
        pub mod download;
        pub mod engine;
        pub mod integrity;
        pub mod metadata;
        pub mod task;
        pub mod worker;
        pub mod writer;
    }
}

pub mod core {
    pub mod utils;
}
