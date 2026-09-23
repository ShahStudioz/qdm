//! # Platform Utilities Module
//!
//! Handles OS-specific system integrations such as Windows startup registry entries.

#[allow(dead_code)]
pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {}", e))?;
        let exe_str = exe_path.to_str()
            .ok_or_else(|| "Invalid UTF-8 in executable path".to_string())?;

        if enabled {
            let status = Command::new("reg")
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "QuickDownloadManager",
                    "/t",
                    "REG_SZ",
                    "/d",
                    exe_str,
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Failed to execute reg command: {}", e))?;

            if !status.success() {
                return Err(format!("reg add failed with code {:?}", status.code()));
            }
        } else {
            let _ = Command::new("reg")
                .args([
                    "delete",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "QuickDownloadManager",
                    "/f",
                ])
                .status();
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Ok(())
    }
}

/// Returns the normalized OS platform name matching QDM_Web API conventions ("windows", "macos", "linux").
#[allow(dead_code)]
pub fn current_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown"
    }
}

/// Returns the normalized CPU architecture matching QDM_Web API conventions ("x64", "arm64", "x86").
#[allow(dead_code)]
pub fn current_arch() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x64"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(target_arch = "x86")]
    {
        "x86"
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    {
        "unknown"
    }
}

/// Launches the downloaded installer or package in a detached background process so QDM can exit.
#[allow(dead_code)]
pub fn launch_installer(installer_path: &std::path::Path) -> Result<(), String> {
    use std::process::Command;

    if !installer_path.exists() {
        return Err(format!("Installer file does not exist at {:?}", installer_path));
    }

    #[cfg(target_os = "windows")]
    {
        // On Windows, launch the installer as a separate detached process using cmd start.
        Command::new("cmd")
            .args(["/C", "start", "", installer_path.to_str().unwrap_or("")])
            .spawn()
            .map_err(|e| format!("Failed to spawn installer: {}", e))?;

        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let path_str = installer_path.to_str().unwrap_or("");
        if path_str.ends_with(".AppImage") {
            let _ = Command::new("chmod").args(["+x", path_str]).status();
            Command::new(installer_path)
                .spawn()
                .map_err(|e| format!("Failed to launch AppImage: {}", e))?;
        } else {
            Command::new("xdg-open")
                .arg(installer_path)
                .spawn()
                .map_err(|e| format!("Failed to open installer: {}", e))?;
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(installer_path)
            .spawn()
            .map_err(|e| format!("Failed to open installer: {}", e))?;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Unsupported operating system for auto-install".to_string())
    }
}

