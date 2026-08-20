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
        pub mod queue;
        pub mod task;
        pub mod throttler;
        pub mod worker;
        pub mod writer;
    }
    pub mod network;
    pub mod schedule;
}

pub mod core {
    pub mod utils;
}
