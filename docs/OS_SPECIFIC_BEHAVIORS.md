# OS-Specific Behaviors & Architectural Solutions in QDM

This guide documents the operating system integrations, quirks, and engineering solutions developed for **Quick Download Manager (QDM)**. It serves as an authoritative architectural reference for QDM maintainers and a blueprint for future cross-platform desktop applications built in Rust and [Iced](https://iced.rs/).

---

## 1. Architectural Overview

Building a modern, frameless, cross-platform desktop application requires navigating deep divergences across Windows, macOS, and Linux desktop environments:

| Feature / Subsystem | Windows (10/11) | macOS (Cocoa) | Linux (GNOME Wayland / KDE Plasma) |
| :--- | :--- | :--- | :--- |
| **Iced Runtime** | `iced::application` | `iced::application` | `iced::daemon` (multi-window lifecycle) |
| **Window Frame** | Frameless + Win32 Subclassing | Frameless + Native Window Shadows | Frameless + Compositor Shadows |
| **Window Controls** | Right-aligned (44×32px buttons) | Left-aligned (14px traffic lights) | Right-aligned (44×32px buttons) |
| **Close to Tray** | `Mode::Hidden` | `Mode::Hidden` | Window Destruction (`iced::window::close`) |
| **Restore from Tray** | `Mode::Windowed` + Focus | `Mode::Windowed` + Focus | Window Recreation (`iced::window::open`) |
| **Tray Tech Stack** | Win32 Shell NotifyIcon | Cocoa `NSStatusItem` | Ayatana / AppIndicator via DBus SNI |
| **Dock / Taskbar Link** | Automatic by HWND | Automatic by `.app` bundle | `StartupWMClass` + `PlatformSpecific` |
| **Autostart** | Registry `HKCU\...\Run` | LaunchAgent `.plist` | XDG Autostart `~/.config/autostart/*.desktop` |
| **Power Actions** | `rundll32` / `shutdown.exe` | `pmset` / AppleScript `osascript` | `systemctl` / `shutdown` |
| **Packaging Target** | NSIS Installer (`.exe`) | App Bundle (`.dmg`) | AppImage (universal) + Debian (`.deb`) |

---

## 2. macOS Platform Behaviors

### 2.1 Custom Frameless Title Bar & Traffic Lights
- **Human Interface Guidelines (HIG)**: macOS users expect window controls ("traffic lights") on the far **top-left** of the window in the order **Close** (Red `#FF5F56`), **Minimize** (Yellow `#FFBD2E`), and **Maximize/Restore** (Green `#27C93F`).
- **Layout Inversion**: On Windows and Linux, the application logo and title appear on the left, with controls on the right. On macOS, QDM inverts the title bar row:
  ```text
  macOS:         [Traffic Lights] ---- [Draggable Area] ---- [App Logo + Version]
  Windows/Linux: [App Logo + Version] ---- [Draggable Area] ---- [Min / Max / Close]
  ```
- **Button Styling**: macOS buttons are circular (14×14px with 8px inner glyph icons) and spaced 8px apart. Hover and press states use precise brightness shifts rather than square backdrops.

### 2.2 Autostart via LaunchAgent Plist
- Rather than relying on deprecated Login Items APIs, QDM registers for startup by generating an interactive property list (`.plist`) inside `~/Library/LaunchAgents/shahstudioz.qdm.app.plist`:
  ```xml
  <?xml version="1.0" encoding="UTF-8"?>
  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  <plist version="1.0">
  <dict>
      <key>Label</key>
      <string>shahstudioz.qdm.app</string>
      <key>ProgramArguments</key>
      <array>
          <string>/path/to/qdm</string>
          <string>--minimized</string>
      </array>
      <key>RunAtLoad</key>
      <true/>
      <key>ProcessType</key>
      <string>Interactive</string>
  </dict>
  </plist>
  ```
- To disable startup, QDM deletes this plist file.

### 2.3 Native System Commands
- **File / Folder Reveal**: Uses `open <path>`.
- **Sleep**: Executes `pmset sleepnow`.
- **Graceful Shutdown**: Dispatches AppleScript via `osascript -e 'tell app "System Events" to shut down'` to let the OS prompt the user for unsaved documents.

### 2.4 macOS Packaging & Signing
- **Iconset Generation**: macOS requires a multi-resolution `.icns` file containing 16x16 up to 1024x1024 icons. The CI workflow builds this on macOS runners using `iconutil -c icns AppIcon.iconset -o assets/icons/icon.icns`.
- **Ad-hoc Codesigning**: To prevent macOS Gatekeeper from killing the app bundle immediately, the DMG is mounted in CI, and the `.app` bundle is signed with `codesign --force --deep --sign "-" --options runtime`.

---

## 3. Linux Platform Behaviors & Solutions (Deep Dive)

### 3.1 The System Tray & GNOME Wayland Dilemma

#### The Problem
In modern Linux desktop environments running **Wayland** (notably GNOME Mutter on Ubuntu, Fedora, Debian):
1. **Window Isolation Security**: Wayland compositors prohibit background applications from taking focus or restoring minimized/hidden windows.
2. When an app minimizes to tray using `iced::window::change_mode(id, Mode::Hidden)` or `window::minimize(id, true)`:
   - Clicking the system tray icon triggers a DBus `Activate` call via StatusNotifierItem.
   - Calling `change_mode(Mode::Windowed)` or `gain_focus()` on the existing hidden window is **silently ignored or rejected by GNOME Wayland**. The window never reappears.

#### The Solution: `iced::daemon` + Window Destruction & Recreation
Instead of `iced::application`, QDM runs as an **`iced::daemon`** on Linux:
- `iced::application` automatically exits the process when all windows are closed.
- `iced::daemon` keeps the event loop, background Tokio tasks, and tray icon running **indefinitely with zero windows open**.
- **Close to Tray**:
  ```rust
  // Linux Daemon Mode: Destroy the window completely
  app.window_id = None;
  iced::window::close(id)
  ```
  Destroying the window completely removes it from the GNOME/KDE dock and taskbar.
- **Restore from Tray**:
  ```rust
  // Linux Daemon Mode: Open a brand-new window
  let (id, open_task) = iced::window::open(create_main_window_settings());
  app.window_id = Some(id);
  open_task.map(Message::NewWindowOpened)
  ```
  Because opening a window is a new surface allocation request, Wayland compositors always allow it to map, render, and acquire keyboard/mouse focus immediately!

### 3.2 Dock Icon Setup & Desktop Grouping (`StartupWMClass`)

#### The Problem
On Linux, launching an application often results in:
- A generic gear/cog icon showing in the dock instead of the app logo.
- Multiple separate dock icons appearing instead of grouping with the pinned launcher.

#### The Solution
1. **`PlatformSpecific` Application ID**:
   When opening windows in Iced, configure the platform-specific application ID:
   ```rust
   iced::window::Settings {
       platform_specific: iced::window::settings::PlatformSpecific {
           application_id: "qdm".to_string(),
           ..Default::default()
       },
       ..Default::default()
   }
   ```
2. **Desktop File `StartupWMClass`**:
   In `packaging/qdm.desktop`:
   ```ini
   [Desktop Entry]
   Type=Application
   Name=Quick Download Manager
   Exec=qdm %U
   Icon=qdm
   StartupNotify=true
   StartupWMClass=qdm
   MimeType=application/x-bittorrent;x-scheme-handler/magnet;
   ```
   The X11 `WM_CLASS` and Wayland `app_id` match `qdm`, grouping the running window with the dock launcher.
3. **Database & Icon Caches (`postinst` / `postrm`)**:
   Debian packages must update system caches upon installation and removal:
   - `update-desktop-database -q /usr/share/applications`
   - `gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor`
   - `update-mime-database /usr/share/mime`

### 3.3 Dynamic Library Panic Safety (`libappindicator`)
- On Linux, `tray-icon` links dynamically to `libayatana-appindicator3.so` or `libappindicator3.so`.
- If a user runs QDM on an Arch or minimal Linux installation where this library is missing, `libappindicator-sys` triggers an unhandled Rust panic during `dlopen`.
- **Solution**: QDM wraps tray initialization inside `std::panic::catch_unwind`:
  ```rust
  match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      Self::try_build(logo_rgba, width, height)
  })) {
      Ok(result) => result,
      Err(_) => {
          eprintln!("[QDM Tray] System tray initialization failed (missing native library). Running in window-only mode.");
          None
      }
  }
  ```
  If the library is absent, QDM logs a warning and boots into normal window-only mode without crashing.

### 3.4 Universal Linux Packaging (AppImage + `linuxdeploy`)

#### Why Not Pure Static Linking?
GTK and GLib cannot be purely statically linked into Rust binaries because GTK dynamically opens:
- Display modules (Wayland / X11 backends)
- Theme rendering engines
- GSettings schema files
- GDK Pixbuf image loaders

#### The Solution: Bundling with `linuxdeploy` & `linuxdeploy-plugin-gtk`
To eliminate `apt install` or `pacman -S` requirements for users, QDM's `Cargo.toml` configures `cargo-packager`'s AppImage builder:

```toml
[package.metadata.packager.appimage]
libs = [
    "libayatana-appindicator3.so*",
    "libayatana-ido3-0.4.so*",
    "libdbusmenu-glib.so*",
    "libdbusmenu-gtk3.so*",
    "libxdo.so*",
    "libssl.so*",
    "libcrypto.so*",
]
linuxdeploy-plugins = { "gtk" = "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh" }
```

In GitHub Actions release workflows, we pass `DEPLOY_GTK_VERSION=3`:
1. `linuxdeploy` scans the compiled Rust binary and pulls all matching dynamic `.so` dependencies into the AppImage's `usr/lib` folder.
2. `linuxdeploy-plugin-gtk` copies GTK 3 assets, `glib-2.0/schemas/gschemas.compiled`, and MIME definitions into the AppImage.
3. The resulting `.AppImage` is 100% self-contained and runs on Ubuntu, Debian, Fedora, Arch, openSUSE, and Alpine without additional runtime installations.

### 3.5 AppImage Host Isolation & Virtual Machine Graphics Safeguards

#### 1. Host GIO / GVFS Symbol Mismatch (`g_task_set_static_name`)
- **The Issue**: When an AppImage bundles GTK and GLib, `AppRun` prepends the bundled libraries to `LD_LIBRARY_PATH`. On modern desktop distributions (such as Ubuntu 24.04), GIO attempts to load host desktop integration modules from `/usr/lib/x86_64-linux-gnu/gio/modules/libgvfsdbus.so`. If the AppImage was built on an older runner with an older GLib, host modules fail to link due to missing symbols (e.g. `undefined symbol: g_task_set_static_name`).
- **The Solution**: In `src/main.rs`, QDM detects when running under an AppImage (`APPIMAGE` or `APPDIR` environment variables) and sets `GIO_MODULE_DIR=""` if not explicitly overridden, isolating the bundled GLib from incompatible host extensions.

#### 2. Virtual Machine & Fallback Graphics Backends (`wgpu` / EGL)
- **The Issue**: In virtualized Linux guests (VMware, VirtualBox, QEMU), hardware Vulkan drivers are absent. When falling back to OpenGL/GLES, `wgpu-hal`'s EGL loader requires an active EGL 1.5 platform context (`khronos_egl::EGL1_5`). If graphics driver versions or DRI loaders mismatch between the bundled libraries and the host Mesa driver (`vmwgfx_dri.so`), platform negotiation drops through to `egl.get_display(EGL_DEFAULT_DISPLAY).unwrap()`, which panics on Linux.
- **The Solution**:
  1. Default `WGPU_BACKEND` to `"vulkan,gl"` and `WGPU_ALLOW_INSECURE="1"`.
  2. Build GitHub release AppImages on modern LTS environments (`ubuntu-24.04`) so bundled Mesa/X11 libraries match contemporary distros.
  3. If running inside a VM with broken 3D driver acceleration, setting `LIBGL_ALWAYS_SOFTWARE=1` forces Mesa to use `llvmpipe` (CPU software rasterization), which provides full EGL 1.5 compliance without GPU driver crashes.

---

## 4. Windows Platform Behaviors

### 4.1 Frameless Window with Native DWM Resizing
- When `decorations: false` is configured in Iced, Windows strips standard resize borders and snap gestures.
- QDM uses Win32 subclassing via `windows-sys` (`SetWindowSubclass`):
  1. **Restore `WS_THICKFRAME`**: Signals to Windows that the window is resizable.
  2. **`WM_NCCALCSIZE`**:
     ```rust
     if wparam != 0 && IsZoomed(hwnd) == 0 {
         return 0; // Client area fills the entire window rectangle
     }
     ```
  3. **`WM_NCHITTEST`**: Checks mouse coordinates against a 7px border and 14px corners, returning `HTLEFT`, `HTRIGHT`, `HTTOP`, `HTBOTTOM`, `HTTOPLEFT`, etc. This restores native hardware cursor resizing indicators.
  4. **Flicker Free Resizing**:
     - `WM_ERASEBKGND`: Returns 1 (prevents Windows from repainting white backgrounds).
     - `WM_WINDOWPOSCHANGING`: Adds `SWP_NOCOPYBITS` to prevent stale framebuffer tearing.

### 4.2 Windows 11 DWM Rounded Corners
- **Background Bleed Prevention**: Extends the DWM frame sheet into the client area via `DwmExtendFrameIntoClientArea` with `-1` margins. This ensures transparent rounded corners don't have white square boxes behind them.
- **Dynamic Corner Preference**:
  Using `DwmSetWindowAttribute` with `DWMWA_WINDOW_CORNER_PREFERENCE (33)`:
  - When windowed: Applies `DWMWCP_ROUND (2)` for Windows 11 rounded corners.
  - When maximized: Applies `DWMWCP_DONOTROUND (1)` so the window snaps flat against display borders without clipping corner pixels.

### 4.3 Windows Registry Autostart
- Registered under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
- Command value: `"<exe_path>" --minimized`.
- Execution flag: `CREATE_NO_WINDOW (0x08000000)` ensures `reg.exe` runs silently without flashing a CMD window.

### 4.4 Windows Single-Instance IPC
- Uses Win32 Named Pipes: `\\.\pipe\qdm-single-instance-pipe`.
- The first instance creates the pipe server and listens in a background Tokio task.
- Secondary instances connect to the pipe, transmit CLI arguments (such as torrent URLs or `--add-url`), request window focus, and exit immediately.

---

## 5. Architectural Checklist for Future Projects

When creating or maintaining a cross-platform Rust/Iced desktop application:

1. **Never leak `#[cfg(target_os = ...)]` into UI view code**:
   Consolidate all operating system commands, file reveals, and power controls behind a single deep adapter module (such as `core::platform` or `core::utils::platform`).
2. **For Linux system trays, always use `iced::daemon`**:
   Do not rely on `Mode::Hidden` or `minimize()` to restore windows on Wayland. Close the window on minimize, and use `iced::window::open()` on tray restore.
3. **Set `application_id` and `StartupWMClass`**:
   Always match the `application_id` in `PlatformSpecific` with the `.desktop` file's `StartupWMClass` and `Icon` name.
4. **Protect tray libraries with `catch_unwind`**:
   Never let missing `libayatana-appindicator3` or `libgtk-3` libraries panic your application on launch.
5. **Use `linuxdeploy-plugin-gtk` for AppImages**:
   Always bundle GTK schemas and runtime assets when targeting universal Linux distributions.
