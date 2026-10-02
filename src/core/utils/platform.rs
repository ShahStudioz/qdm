//! # Platform Utilities Module
//!
//! Provides unified cross-platform abstractions for operating system integrations,
//! including startup registration, shell execution, power management, and
//! system information queries.
//!
//! ## Supported Operating Systems
//! - **Windows**: Registry Run key, Win32 ShellExecuteW, Rundll32 power calls
//! - **macOS**: LaunchAgents plist, AppleScript / pmset, native `open`
//! - **Linux**: XDG Autostart desktop files, Systemd / Shutdown, native `xdg-open`

use std::path::Path;

/// Configures whether QDM launches automatically when the user logs in to their OS.
///
/// - **Windows**: Adds or deletes an entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
///   with `--minimized`.
/// - **macOS**: Creates or removes a `~/Library/LaunchAgents/shahstudioz.qdm.app.plist` file.
/// - **Linux**: Creates or removes `~/.config/autostart/qdm.desktop`.
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

/// Opens a file or directory using the platform's native file manager / shell
/// without spawning a visible console window.
pub fn open_path_native(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide_path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let wide_op: Vec<u16> = std::ffi::OsStr::new("open")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                0,
                wide_op.as_ptr(),
                wide_path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
    }
}

/// Puts the host computer into sleep/suspend mode.
///
/// - Windows: `rundll32.exe powrprof.dll,SetSuspendState 0,1,0`
/// - macOS: `pmset sleepnow`
/// - Linux: `systemctl suspend`
pub fn sleep_computer() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32.exe")
            .args(["powrprof.dll,SetSuspendState", "0,1,0"])
            .spawn()
            .map_err(|e| format!("Failed to initiate sleep: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("pmset")
            .args(["sleepnow"])
            .spawn()
            .map_err(|e| format!("Failed to initiate sleep: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("systemctl")
            .args(["suspend"])
            .spawn()
            .map_err(|e| format!("Failed to initiate sleep: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Sleep is not supported on this platform".to_string())
    }
}

/// Shuts down the host computer with a grace period / warning notification.
///
/// - Windows: `shutdown /s /t 60` with a cancellation notice.
/// - macOS: AppleScript tell "System Events" to shut down.
/// - Linux: `shutdown -h +1`
pub fn shutdown_computer() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("shutdown")
            .args([
                "/s",
                "/t",
                "60",
                "/c",
                "QDM: All scheduled downloads complete. Computer will shut down in 60 seconds. Run 'shutdown /a' in CMD to cancel.",
            ])
            .spawn()
            .map_err(|e| format!("Failed to schedule shutdown: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("osascript")
            .args(["-e", "tell app \"System Events\" to shut down"])
            .spawn()
            .map_err(|e| format!("Failed to schedule shutdown: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("shutdown")
            .args(["-h", "+1"])
            .spawn()
            .map_err(|e| format!("Failed to schedule shutdown: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Shutdown is not supported on this platform".to_string())
    }
}

/// Launches a downloaded installer or package in a detached background process so QDM can exit cleanly.
pub fn launch_installer(installer_path: &Path) -> Result<(), String> {
    use std::process::Command;

    if !installer_path.exists() {
        return Err(format!(
            "Installer file does not exist at {:?}",
            installer_path
        ));
    }

    #[cfg(target_os = "windows")]
    {
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
