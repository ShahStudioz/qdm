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
