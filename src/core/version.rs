//! Application version and build metadata.
//!
//! Centralized source of truth for the application version, ensuring
//! UI elements, update checkers, HTTP User-Agent, and diagnostics
//! stay in sync from a single place.

/// The current application semver version string (e.g. "1.0.0").
///
/// Automatically derived from `Cargo.toml` at compile-time via `env!("CARGO_PKG_VERSION")`.
/// Changing the version in `Cargo.toml` propagates to all UI displays and update systems.
#[allow(dead_code)]
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Formatted version with 'v' prefix (e.g. "v1.0.0") used in the title bar frame.
pub const APP_VERSION_TAG: &str = concat!("v", env!("CARGO_PKG_VERSION"));

/// Formatted version badge for the sidebar (e.g. "V1.0.0-RUST").
pub const APP_VERSION_SIDEBAR: &str = concat!("V", env!("CARGO_PKG_VERSION"), "-RUST");

/// Formatted version for the updates settings tab (e.g. "v1.0.0 (Latest build)").
pub const APP_VERSION_BUILD: &str = concat!("v", env!("CARGO_PKG_VERSION"), " (Latest build)");

/// User-Agent string for HTTP requests.
pub const APP_USER_AGENT: &str = concat!(
    "QDM/",
    env!("CARGO_PKG_VERSION"),
    " (Quick Download Manager; Windows NT 10.0; Win64; x64)"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants() {
        assert_eq!(APP_VERSION, "1.0.1");
        assert_eq!(APP_VERSION_TAG, "v1.0.1");
        assert_eq!(APP_VERSION_SIDEBAR, "V1.0.1-RUST");
        assert!(APP_VERSION_BUILD.starts_with("v1.0.1"));
        assert!(APP_USER_AGENT.contains("QDM/1.0.1"));
    }
}
