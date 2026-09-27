<p align="center">
  <img src="assets/images/logo.png" alt="QDM Logo" width="180" />
</p>

<h1 align="center">QDM — Quick Download Manager</h1>

<p align="center">
  <strong>A fast, modern, open-source download manager built with Rust & Iced.</strong>
</p>

<p align="center">
  <a href="https://github.com/ShahStudioz/qdm/releases/latest"><img alt="Latest Release" src="https://img.shields.io/github/v/release/ShahStudioz/qdm?style=flat-square&color=00b894" /></a>
  <a href="https://github.com/ShahStudioz/qdm/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ShahStudioz/qdm/ci.yml?branch=main&style=flat-square&label=CI" /></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" /></a>
  <a href="https://qdm.shahstudioz.store/"><img alt="Website" src="https://img.shields.io/badge/website-qdm.shahstudioz.store-0984e3?style=flat-square" /></a>
</p>

<p align="center">
  <a href="https://qdm.shahstudioz.store/">Website</a> •
  <a href="#features">Features</a> •
  <a href="#download">Download</a> •
  <a href="#building-from-source">Build from Source</a> •
  <a href="#contributing">Contributing</a>
</p>

<p align="center">
  <a href="https://qdm.shahstudioz.store/#download">
    <img src="https://img.shields.io/badge/%E2%AC%87%EF%B8%8F_Download_QDM-Latest_Release-00b894?style=for-the-badge&logoColor=white" alt="Download QDM" height="40" />
  </a>
</p>

---

## Features

### ⚡ High-Speed Downloads
- **Segmented multi-connection HTTP/HTTPS** — splits files into up to 32 parallel chunks for maximum throughput.
- **Mirror support** — add multiple mirror URLs for automatic failover and load balancing.
- **Resumable downloads** — respects `Range`, `ETag`, and `Last-Modified` headers for seamless resume after interruption.

### 🧲 BitTorrent & Magnet Links
- Built on [librqbit](https://github.com/ikatson/librqbit), a high-performance pure-Rust BitTorrent engine.
- Magnet URI resolution, `.torrent` file support, selective file downloads, DHT, and seeding.
- Live peer/seed counts, upload speed, and piece-level progress tracking.

### 🌐 Network Intelligence
- **Auto-pause on disconnect** — detects network loss and pauses active transfers automatically.
- **Auto-resume on reconnect** — resumes all paused downloads the moment connectivity is restored.
- **Automatic retry** — configurable retry with exponential backoff for transient failures.

### 🎛️ Download Management
- **Smart queue system** — configurable concurrent download limits with automatic promotion of queued items.
- **Bandwidth throttler** — global and per-download speed limiting with a token-bucket algorithm.
- **Download scheduler** — define active time windows for downloads (e.g., overnight only).
- **File conflict resolution** — auto-rename, overwrite, or prompt when destination files already exist.
- **Duplicate detection** — identifies duplicate downloads by URL, magnet info-hash, or destination path.

### 🖥️ Modern UI
- Custom frameless window with a dark theme built entirely with [Iced](https://github.com/iced-rs/iced).
- Real-time per-chunk progress visualization, live speed/ETA display, and visual file type icons.
- Download details dialog with per-thread, per-connection, and per-chunk breakdowns.
- System tray integration with Pause All / Resume All / Show / Quit controls.

### 🔄 Auto-Updates
- Built-in update system that checks for new releases, downloads update payloads, verifies SHA-256 integrity, and launches the installer — all from within the app.

### 🖱️ Clipboard Integration
- Opening the "Add Download" dialog automatically detects valid URLs, magnet links, or `.torrent` file paths in your clipboard and begins metadata probing immediately.

---

## Download

### 👉 [Download from the QDM Website](https://qdm.shahstudioz.store/#download)

Visit **[qdm.shahstudioz.store](https://qdm.shahstudioz.store/#download)** to download the latest installer for your platform — Windows, Linux, or macOS.

> Alternatively, you can grab installers directly from [GitHub Releases](https://github.com/ShahStudioz/qdm/releases/latest).

| Platform | Format | Link |
|----------|--------|------|
| **Windows** (x64) | `.exe` installer | [Website](https://qdm.shahstudioz.store/#download) · [GitHub](https://github.com/ShahStudioz/qdm/releases/latest) |
| **Linux** (x64) | `.deb` / `.AppImage` | [Website](https://qdm.shahstudioz.store/#download) · [GitHub](https://github.com/ShahStudioz/qdm/releases/latest) |
| **macOS** (x64) | `.dmg` | [Website](https://qdm.shahstudioz.store/#download) · [GitHub](https://github.com/ShahStudioz/qdm/releases/latest) |
| **macOS** (Apple Silicon) | `.dmg` | [Website](https://qdm.shahstudioz.store/#download) · [GitHub](https://github.com/ShahStudioz/qdm/releases/latest) |

### 🍎 macOS Users — Important Note

Since QDM is not signed with an Apple Developer certificate, macOS Gatekeeper may show **"QDM is damaged and can't be opened"** or **"QDM can't be opened because Apple cannot check it for malicious software"** when you first launch the app. This is expected for open-source software distributed outside the Mac App Store.

**To open QDM, use one of these methods:**

**Method 1 — Right-click to Open (easiest)**
1. Open Finder and navigate to the QDM app.
2. **Right-click** (or Control-click) on **QDM.app** and select **Open**.
3. Click **Open** in the confirmation dialog. You only need to do this once.

**Method 2 — Remove the quarantine attribute (Terminal)**
```bash
xattr -dr com.apple.quarantine /Applications/QDM.app
```

**Method 3 — System Settings**
1. Go to **System Settings → Privacy & Security**.
2. Scroll down to the **Security** section.
3. You should see a message about QDM being blocked — click **"Open Anyway"**.

---

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, 1.75+)
- Platform-specific system libraries (see below)

### Windows

No extra system dependencies are required — just Rust and the MSVC build tools that come with Visual Studio or the Visual Studio Build Tools installer.

```bash
cargo build --release --bin qdm
```

### Linux (Ubuntu / Debian)

Install the required system libraries first:

```bash
sudo apt-get update
sudo apt-get install -y \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  libssl-dev \
  pkg-config \
  libasound2-dev \
  libxdo-dev
```

Then build:

```bash
cargo build --release --bin qdm
```

### macOS

```bash
cargo build --release --bin qdm
```

### Running

```bash
# Debug build (faster compilation)
cargo run --bin qdm

# Release build (optimized)
cargo run --release --bin qdm
```

### Running Tests

```bash
cargo test --all-targets
```

---

## Architecture

```
src/
├── main.rs                    # Entry point, single-instance guard, Iced window setup
├── lib.rs                     # Library target exposing backend modules
├── icons.rs                   # FontAwesome glyph bindings
│
├── app/                       # Application state, message routing, lifecycle
│   ├── mod.rs                 # QdmApp struct, Message enum
│   ├── view.rs                # Main window layout composition
│   ├── subscriptions.rs       # Event streams (ticks, engine events, keyboard)
│   └── handlers/              # Message handlers (downloads, dialogs, engine, queue, …)
│
├── core/                      # Platform & utility layer
│   ├── single_instance.rs     # IPC-based single-instance enforcement
│   ├── version.rs             # Compile-time version constants from Cargo.toml
│   ├── window_sys.rs          # Windows-specific frameless window behaviors
│   └── utils/                 # Path helpers, platform detection
│
├── models/                    # Data structures
│   ├── download.rs            # DownloadItem, state machine, HTTP/Torrent metadata
│   └── schedule.rs            # Time-based scheduler rules
│
├── services/                  # Backend engines
│   ├── engine.rs              # Unified facade coordinating HTTP + Torrent subsystems
│   ├── http/                  # Segmented HTTP engine (metadata, workers, writer, throttler)
│   ├── torrent/               # BitTorrent engine (librqbit wrapper, DHT, staging)
│   ├── shared/                # Network monitor, scheduler, JSON persistence
│   ├── updater/               # Auto-update check, download, verify, install
│   └── tray.rs                # System tray service
│
├── theme/                     # Visual theming
│   ├── colors.rs              # Color palette
│   └── styles.rs              # Widget style functions
│
└── views/                     # UI components
    ├── components/            # Title bar, sidebar, toolbar
    ├── downloads/             # Download list & item cards
    ├── dialogues/             # Add, delete, conflict, detail, mirror, update dialogs
    ├── settings/              # General, downloads, torrents, scheduler, updates tabs
    └── preferences/           # Queue reordering view
```

### Configuration & Data

QDM stores all user data in `~/.qdm/`:

| File | Purpose |
|------|---------|
| `downloads.json` | Serialized download list (survives restarts) |
| `settings.json` | User preferences and configuration |
| `dht.json` | Persistent DHT routing table for BitTorrent |
| `updates/` | Staging directory for downloaded update installers |

---

## Contributing

Contributions are welcome and appreciated! Whether it's a bug fix, new feature, documentation improvement, or UI polish — all PRs are valued.

### How to Contribute

1. **Fork** the repository and create a feature branch from `main`.
2. **Make your changes** and ensure they follow the project's code style.
3. **Run the checks locally** before pushing:
   ```bash
   cargo fmt --all -- --check     # Code formatting
   cargo clippy --all-targets -- -D warnings   # Linting
   cargo test --all-targets       # Automated tests
   ```
4. **Open a Pull Request** against `main` with a clear description of what you changed and why.

### PR Requirements

All pull requests are automatically validated by CI. Your PR must pass:

- ✅ **Formatting** — `cargo fmt` compliance
- ✅ **Linting** — zero Clippy warnings
- ✅ **Tests** — all existing tests must pass
- ✅ **Cross-platform check** — code must compile on Windows, Linux, and macOS

### Guidelines

- Keep PRs focused — one feature or fix per PR is ideal.
- Add tests for new functionality when possible.
- Follow existing code patterns and naming conventions.
- Update documentation or comments if your change affects public behavior.

---

## License

This project is licensed under the [MIT License](LICENSE).

---

<p align="center">
  Built with ❤️ in Rust by <a href="https://github.com/ShahStudioz">ShahStudioz</a>
</p>
