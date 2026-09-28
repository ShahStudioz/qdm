//! # Platform Utilities Module
//!
//! Handles OS-specific system integrations such as Windows startup registry entries.

#[allow(dead_code)]
pub fn set_launch_at_startup(enabled: bool) -> Result<(), String> {
    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to get executable path: {}", e))?;
    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| "Invalid UTF-8 in executable path".to_string())?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let launch_cmd = format!("\"{}\" --minimized", exe_str);

        if enabled {
            let status = Command::new("reg")
                .creation_flags(CREATE_NO_WINDOW)
                .args([
                    "add",
                    r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                    "/v",
                    "QuickDownloadManager",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &launch_cmd,
                    "/f",
                ])
                .status()
                .map_err(|e| format!("Failed to execute reg command: {}", e))?;

            if !status.success() {
                return Err(format!("reg add failed with code {:?}", status.code()));
            }
        } else {
            let _ = Command::new("reg")
                .creation_flags(CREATE_NO_WINDOW)
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

    #[cfg(target_os = "macos")]
    {
        let home =
            dirs::home_dir().ok_or_else(|| "Failed to resolve home directory".to_string())?;
        let launch_agents_dir = home.join("Library").join("LaunchAgents");
        let plist_path = launch_agents_dir.join("shahstudioz.qdm.app.plist");

        if enabled {
            std::fs::create_dir_all(&launch_agents_dir)
                .map_err(|e| format!("Failed to create LaunchAgents directory: {}", e))?;

            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>shahstudioz.qdm.app</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--minimized</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
                exe_str
            );

            std::fs::write(&plist_path, plist_content)
                .map_err(|e| format!("Failed to write LaunchAgent plist: {}", e))?;
        } else if plist_path.exists() {
            let _ = std::fs::remove_file(&plist_path);
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        let config_dir =
            dirs::config_dir().ok_or_else(|| "Failed to resolve config directory".to_string())?;
        let autostart_dir = config_dir.join("autostart");
        let desktop_path = autostart_dir.join("qdm.desktop");

        if enabled {
            std::fs::create_dir_all(&autostart_dir)
                .map_err(|e| format!("Failed to create autostart directory: {}", e))?;

            let desktop_content = format!(
                "[Desktop Entry]\nType=Application\nName=Quick Download Manager\nComment=Modern Open-Source Downloader in Rust & Iced\nExec=\"{}\" --minimized\nTerminal=false\nStartupNotify=false\nX-GNOME-Autostart-enabled=true\n",
                exe_str
            );

            std::fs::write(&desktop_path, desktop_content)
                .map_err(|e| format!("Failed to write autostart desktop entry: {}", e))?;
        } else if desktop_path.exists() {
            let _ = std::fs::remove_file(&desktop_path);
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (enabled, exe_str);
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
        return Err(format!(
            "Installer file does not exist at {:?}",
            installer_path
        ));
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
