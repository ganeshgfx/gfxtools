# Paste Link Downloader (Video Yoinker)

A native Windows application that adds a **"Paste link"** entry to the Windows Explorer right-click context menu, letting you download videos from YouTube, Pinterest, Instagram — and any site supported by yt-dlp — directly into the folder you right-clicked in. Also ships an **Adobe CEP plugin** for Premiere Pro and After Effects that downloads and auto-imports media from inside the NLE.

---

## Features

- **Right-click → Paste link** in any Explorer folder to download video from clipboard URL
- **Shift+Click advanced options**: choose resolution, audio-only/video-only, audio format, bitrate, and trim start/end time
- **Dual download engine**: yt-dlp (video) with automatic gallery-dl fallback (images/galleries)
- **Post-processing context menus** on video files:
  - **Convert to Compatible** — re-encode to H.264+AAC MP4 for NLE compatibility (Premiere Pro, After Effects, DaVinci Resolve)
  - **Compress** — re-encode to HEVC+AAC MP4 for efficient storage (outputs to `small/` subfolder)
- **NVIDIA GPU acceleration** — post-processing uses NVENC (`h264_nvenc` / `hevc_nvenc`) when available, with automatic CPU fallback (`libx264` / `libx265`)
- **Multi-select support** — select multiple video files in Explorer, right-click → Convert/Compress, all processed in a single GUI window
- **Native Win32 GUI**: dark-themed download progress window with progress bar, cancel, and open-folder buttons
- **eframe/egui GUIs**: settings panel, advanced download options, and post-processing progress windows
- **Adobe CEP Plugin**: Premiere Pro / After Effects panel — paste URL, download, auto-import into project bin
- **NLE-optimized encoding**: forces H.264 + AAC transcoding so downloaded files open without issues in Premiere/AE/DaVinci
- **Cookie support**: extract cookies from Edge/Chrome/Firefox or use a `cookies.txt` file for login-gated sites
- **Per-user installation** — no admin privileges, HKCU registry only
- **Structured logging** with daily log rotation via `tracing`
- **Windows notifications** via MessageBox dialogs (configurable)
- **Filename collision handling** via yt-dlp `--no-overwrites`
- **HTTPS-only** URL validation — HTTP, FTP, and `javascript:` schemes rejected
- **Application icon** embedded in the executable and displayed in all GUI windows

---

## Architecture Overview

```
                    ┌──────────────────────────────────────────────┐
                    │              paste-link-downloader.exe       │
                    │  (Windows subsystem = "windows" in release)  │
                    └──────┬───────────────────────────┬───────────┘
                           │                           │
              CLI dispatch (cli.rs)         Explorer context-menu
              parses --flags or bare          invokes exe with "%V"
              directory argument              (folder path / file path)
                           │                           │
           ┌───────────────┴───────────────────────────┘
           ▼
  ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
  │   download_gui   │    │   settings_gui   │    │   context_menu   │
  │   (Win32 GUI)    │    │   (eframe/egui)  │    │   (Registry)     │
  │                  │    │                  │    │                  │
  │ progress window  │    │ config editor    │    │ install/uninstall│
  │ worker thread    │    │ file pickers     │    │ HKCU registry    │
  │ cancel/open btn  │    │ combo boxes      │    │ SHChangeNotify   │
  └────────┬─────────┘    └──────────────────┘    └──────────────────┘
           │
           │  Shift+Click?
           ▼
  ┌──────────────────┐
  │ advanced_download│
  │ _gui (eframe)   │
  │                  │
  │ resolution cap   │
  │ audio/video only │
  │ trim start/end   │
  │ audio format/br  │
  └────────┬─────────┘
           │
           ▼
  ┌──────────────────┐         ┌──────────────────┐
  │    downloader    │────────▶│     progress     │
  │                  │         │                  │
  │ spawn yt-dlp     │         │ regex parser for │
  │ spawn gallery-dl │         │ yt-dlp stdout    │
  │ pipe stdout/err  │         │ % / speed / ETA  │
  │ cancellation     │         │ errors/warnings  │
  └──────────────────┘         └──────────────────┘
           │
   uses: clipboard, config, platform, notification, error, logging

  ┌──────────────────┐         ┌──────────────────┐
  │   postprocess    │────────▶│ postprocess_gui  │
  │                  │         │   (eframe/egui)  │
  │ FFmpeg convert   │         │                  │
  │ FFmpeg compress  │         │ per-file + total │
  │ NVENC detection  │         │ progress bar     │
  │ batch files      │         │ cancel/open btn  │
  └──────────────────┘         └──────────────────┘
```

The application is built as a single Rust binary with `#![windows_subsystem = "windows"]` so no console window flashes when launched from Explorer. A console is allocated on-demand only for CLI commands like `--diagnostics`.

---

## Requirements

- **Windows 10** or **Windows 11**
- **Rust toolchain** (for building): https://rustup.rs
  - Target: `x86_64-pc-windows-msvc` or `x86_64-pc-windows-gnu`
- **yt-dlp.exe** — https://github.com/yt-dlp/yt-dlp/releases/latest
- **ffmpeg.exe** + **ffprobe.exe** — https://www.gyan.dev/ffmpeg/builds/
- **gallery-dl.exe** *(optional)* — https://github.com/mikf/gallery-dl/releases (enables image gallery fallback)
- **NVIDIA GPU** *(optional)* — NVENC-capable GPU for hardware-accelerated post-processing

---

## Build

```powershell
# Install Rust (if not already installed)
winget install Rustlang.Rustup

# Build release binary
cargo build --release
```

The binary will be at:
```
target\release\paste-link-downloader.exe
```

> **Toolchain note:** The project includes `.cargo/config.toml` documenting GNU (MinGW) toolchain support. MSVC target requires Visual Studio 2022 Build Tools with the "Desktop development with C++" workload.

---

## Install

### Quick Install (recommended)

```powershell
.\install.ps1
```

This master script:
1. Builds the release binary (`cargo build --release`)
2. Uninstalls old context menus and CEP plugin
3. Installs new context menus (Paste link + Convert/Compress on video files)
4. Installs CEP plugin for Adobe apps

Options: `-SkipBuild`, `-SkipPlugin`, `-SkipContextMenu`.

### Manual Install

#### 1. Place required executables

```
bin\
├── yt-dlp.exe      ← download from GitHub
├── ffmpeg.exe      ← download from gyan.dev
├── ffprobe.exe     ← same archive as ffmpeg
└── gallery-dl.exe  ← optional, for image galleries
```

See [`bin/README.md`](bin/README.md) for download links.

#### 2. Run installer

```powershell
.\target\release\paste-link-downloader.exe --install
```

This copies the application to `%LOCALAPPDATA%\PasteLinkDownloader\` and registers all Explorer context menu entries under HKCU (no admin needed).

#### 3. Verify

```powershell
paste-link-downloader.exe --diagnostics
```

```
=== Paste Link Downloader — Diagnostics ===

✓ yt-dlp       : C:\Users\...\PasteLinkDownloader\bin\yt-dlp.exe
✓ FFmpeg       : C:\Users\...\PasteLinkDownloader\bin\ffmpeg.exe
✓ gallery-dl   : C:\Users\...\PasteLinkDownloader\bin\gallery-dl.exe
  Log dir      : C:\Users\...\AppData\Local\PasteLinkDownloader\logs
  Config       : C:\Users\...\AppData\Roaming\PasteLinkDownloader\config.toml
```

---

## Usage

### Explorer Context Menu (primary workflow)

1. Copy a video URL to the clipboard (YouTube, Pinterest, Instagram, or any yt-dlp-supported site).
2. Open any folder in Windows Explorer.
3. Right-click on **empty space** in the folder.
4. Click **Paste link**.
5. A native Win32 progress window shows download progress with a progress bar.
6. On success, click **Open Folder** to jump to the downloaded file. On failure, the error is shown inline.

### Advanced Download (Shift+Click)

Hold **Shift** while clicking "Paste link" to open an advanced options window:
- **Video / Audio toggles** — download video-only, audio-only, or both
- **Audio format** (audio-only mode) — mp3, m4a, opus, flac, wav
- **Resolution cap** — Best, 2160p, 1440p, 1080p, 720p, 480p, 360p
- **Audio bitrate** — Best, 320k, 256k, 192k, 128k, 96k
- **Trim** — start and end time (HH:MM:SS or seconds)

### Video File Context Menu

Right-click on any video file (`.mp4`, `.mkv`, `.webm`, `.avi`, `.mov`, `.ts`, `.flv`, `.m4v`, `.wmv`):

- **Convert to Compatible** — re-encode to H.264 + AAC MP4 (output: `<name>_edit.mp4`)
- **Compress** — re-encode to HEVC + AAC MP4 (output: `small/<name>.mp4`)

Both support **multi-select**: select multiple files, right-click → Convert/Compress, all processed in a single progress window.

Uses NVIDIA NVENC hardware encoding when available, with automatic CPU fallback.

### CLI Usage

```powershell
paste-link-downloader.exe <directory>                 # Download video (same as context menu)
paste-link-downloader.exe --download-images <dir>     # Download images via gallery-dl
paste-link-downloader.exe --convert-compatible <dir>   # Convert videos to Premiere Pro compatible
paste-link-downloader.exe --compress <dir>             # Compress videos for efficient storage
paste-link-downloader.exe --install                    # Register context menus
paste-link-downloader.exe --uninstall                  # Remove context menus
paste-link-downloader.exe --diagnostics                # Check binary resolution
paste-link-downloader.exe --settings                   # Open settings GUI
paste-link-downloader.exe --version                    # Print version
```

---

## Uninstall

```powershell
paste-link-downloader.exe --uninstall
```

Then optionally delete application files:

```powershell
Remove-Item -Recurse -Force "$env:LOCALAPPDATA\PasteLinkDownloader"
```

> Your downloaded videos are **never** deleted by uninstall.

---

## Configuration

Optional config file at:
```
%APPDATA%\PasteLinkDownloader\config.toml
```

You can also edit settings via the native GUI: `paste-link-downloader.exe --settings`

```toml
# Path to yt-dlp.exe (empty = auto-detect: bundled → PATH)
yt_dlp_path = ""

# Directory containing ffmpeg.exe (empty = auto-detect: bundled → PATH)
ffmpeg_dir = ""

# Path to gallery-dl.exe (empty = auto-detect: bundled → PATH)
gallery_dl_path = ""

# Show MessageBox on success/failure
notifications = true

# Output container format
preferred_format = "mp4"

# Log verbosity: "error" | "warn" | "info" | "debug" | "trace"
log_level = "info"

# Browser to extract cookies from (for Instagram and other login-gated sites).
# Supported: "chrome", "firefox", "edge", "brave", "opera", "chromium"
# NOTE: Chrome 127+ blocks cookie decryption (App-Bound Encryption) — use "edge" instead.
# Empty string = disabled.
cookies_from_browser = "edge"

# Path to a Netscape-format cookies.txt file (alternative to cookies_from_browser).
# Takes priority over cookies_from_browser when set.
# Export from Chrome using the "Get cookies.txt LOCALLY" browser extension.
# Empty string = disabled.
cookies_file = ""
```

---

## Logging

Logs are written to:
```
%LOCALAPPDATA%\PasteLinkDownloader\logs\app.log
```

Daily rotation via `tracing-appender`. Old log files have date suffixes. In debug builds, logs also print to stderr.

---

## Download Quality

Default format selection prefers NLE-compatible codecs:

```
Tier 1: H.264 (avc1) video + AAC (m4a) audio     ← ideal, no transcode
Tier 2: H.264 video + any audio
Tier 3: Any non-VP9/AV1/VP8 video + audio
Tier 4: Absolute fallback — best available stream
```

All downloads are post-processed with:
```
ffmpeg -c:v libx264 -preset fast -crf 18 -c:a aac -b:a 192k
```

This ensures every downloaded file is H.264 + AAC regardless of source codec, making it compatible with Premiere Pro, After Effects, DaVinci Resolve, and other NLEs.

---

## Post-processing Quality

### Convert to Compatible (H.264 + AAC)
| Setting | NVENC (GPU) | CPU Fallback |
|---------|-------------|--------------|
| Video codec | `h264_nvenc` | `libx264` |
| Preset | `p4` (balanced) | `medium` |
| Quality | CQ 18 (VBR) | CRF 18 |
| Audio | AAC 192k | AAC 192k |
| Pixel format | `yuv420p` | `yuv420p` |
| Container | MP4 (+faststart) | MP4 (+faststart) |

### Compress (HEVC + AAC)
| Setting | NVENC (GPU) | CPU Fallback |
|---------|-------------|--------------|
| Video codec | `hevc_nvenc` | `libx265` |
| Preset | `p5` (quality) | `medium` |
| Quality | CQ 26 (VBR) | CRF 26 |
| Audio | AAC 128k | AAC 128k |
| Container | MP4 (+faststart) | MP4 (+faststart) |

---

## Security Notes

- Clipboard contents are **never** executed as shell commands.
- `yt-dlp` and `gallery-dl` are invoked directly (`Command::new(path).arg(url)`), not via `cmd.exe`.
- URL scheme is validated (`https` only) — `http`, `ftp`, `javascript:` are all rejected.
- No binaries are auto-downloaded from the internet.
- HKCU registry only — no system-wide changes, no UAC prompts.
- Child processes are spawned with `CREATE_NO_WINDOW` flag to prevent console flashing.

---

## Filename Collision Handling

yt-dlp is invoked with `--no-overwrites` and `--windows-filenames`. If a file with the same name already exists, yt-dlp automatically appends a counter suffix:

```
My Video.mp4
My Video (1).mp4
My Video (2).mp4
```

---

## Project Structure

```
video_yoinker/
├── .cargo/
│   └── config.toml                  ← Cargo build configuration (GNU/MSVC toolchain notes)
├── Cargo.toml                       ← Package manifest, dependencies, release profile
├── Cargo.lock                       ← Dependency lock file
├── build.rs                         ← Build script: Windows subsystem + resource file compilation (app icon)
├── resources.rc                     ← Windows resource file — embeds ico/main.ico into the .exe
├── README.md                        ← This file
├── install.ps1                      ← Master install script: build + uninstall old + install new
├── ico/                             ← Application icon assets (PNGs at various sizes + .ico + source PSD)
│
├── bin/                             ← User-provided external binaries
│   └── README.md                    ← Download links for yt-dlp, ffmpeg, ffprobe
│
├── src/                             ← Rust source code
│   ├── main.rs                      ← Entry point, CLI dispatch, install/uninstall, download orchestration,
│   │                                   postprocess batch collection (multi-select via named mutex)
│   ├── lib.rs                       ← Library crate — re-exports modules for integration tests
│   ├── cli.rs                       ← Argument parsing: maps argv → Command enum
│   ├── clipboard.rs                 ← Reads text from Windows clipboard via `arboard`
│   ├── config.rs                    ← TOML config file loading/saving (%APPDATA%)
│   ├── context_menu.rs              ← Windows Registry: folder background "Paste link" + per-extension
│   │                                   "Convert to Compatible" / "Compress" via SystemFileAssociations
│   ├── download_gui.rs              ← Native Win32 download progress window (GUI thread + worker thread)
│   ├── advanced_download_gui.rs     ← eframe/egui advanced options (Shift+Click): resolution, audio, trim
│   ├── downloader.rs                ← Core download engine: yt-dlp/gallery-dl process spawning + streaming
│   ├── postprocess.rs               ← FFmpeg post-processing: Convert to Compatible / Compress, NVENC GPU
│   ├── postprocess_gui.rs           ← eframe/egui post-processing progress window
│   ├── app_icon.rs                  ← Loads embedded PNG icon for eframe window icons
│   ├── error.rs                     ← Central error enum (thiserror) with user-facing messages
│   ├── logging.rs                   ← tracing subscriber init with rolling file appender
│   ├── notification.rs              ← Windows MessageBox dialogs (success/error/cancelled)
│   ├── platform.rs                  ← URL validation + platform detection (YouTube/Pinterest/Instagram)
│   ├── progress.rs                  ← Regex-based parser for yt-dlp stdout progress lines
│   └── settings_gui.rs             ← eframe/egui settings window with file pickers & combo boxes
│
├── tests/                           ← Integration tests (offline, no network)
│   ├── platform_tests.rs            ← URL validation + platform detection for all supported domains
│   ├── url_tests.rs                 ← URL edge cases: empty, HTTP, injection, unicode
│   └── filename_tests.rs            ← Output template path construction safety
│
├── plugin/                          ← Adobe CEP plugin for Premiere Pro / After Effects
│   ├── CSXS/
│   │   └── manifest.xml             ← CEP bundle manifest (host apps, panel size, Node.js flag)
│   ├── index.html                   ← Panel UI: URL input, format selector, progress bar, log
│   ├── css/
│   │   └── panel.css                ← Dark theme panel styles
│   ├── js/
│   │   ├── cep_init.js              ← CSInterface loader
│   │   ├── downloader.js            ← Spawns paste-link-downloader.exe as child process
│   │   ├── main.js                  ← UI controller: validation, download flow, project detection
│   │   └── lib/
│   │       └── CSInterface.js       ← Adobe CEP JavaScript API (from Adobe-CEP GitHub)
│   ├── jsx/
│   │   └── host.jsx                 ← ExtendScript: creates project bins, imports downloaded files
│   ├── bin/
│   │   └── paste-link-downloader.exe  ← Built binary (copied by install-plugin.ps1)
│   └── README-INSTALL.md            ← Plugin installation guide
│
└── install-plugin.ps1               ← PowerShell script: installs CEP plugin to Adobe extensions dir
```

---

## Source Module Details

### `main.rs` — Entry Point & Orchestration
The application entry point. Parses CLI arguments via `cli.rs`, optionally allocates a console (only for CLI commands — download and post-processing use GUI windows), loads config, initialises logging, and dispatches to the appropriate handler: install, uninstall, download (GUI), download-images (console), convert-compatible, compress, settings, diagnostics, or usage. Contains the `run_download_gui()` flow that reads the clipboard, validates the URL, detects Shift key for advanced options, and opens the progress window. Also contains `batch_collect_and_process()` for multi-select file processing via named mutex leader/follower protocol and `run_postprocess_cmd()` for dispatching post-processing to the GUI.

### `cli.rs` — Argument Parsing
Defines the `Command` enum with variants: `Download`, `DownloadImages`, `ConvertCompatible`, `Compress`, `Install`, `Uninstall`, `Diagnostics`, `Settings`, `Version`, `Usage`. Parses `std::env::args()` with pattern matching — no external CLI framework. A bare positional argument (from Explorer's `%V`) is interpreted as a download directory.

### `clipboard.rs` — Clipboard Access
Uses the `arboard` crate to read text from the Windows clipboard. Returns trimmed, non-empty text or a typed error (`ClipboardEmpty`, `ClipboardError`). Security: clipboard contents are never executed — only passed as a literal argument to yt-dlp.

### `config.rs` — Configuration Management
Defines the `Config` struct with serde `Serialize`/`Deserialize`. Loads from `%APPDATA%\PasteLinkDownloader\config.toml`, falls back to sensible defaults if the file doesn't exist. Fields: `yt_dlp_path`, `ffmpeg_dir`, `gallery_dl_path`, `cookies_from_browser`, `cookies_file`, `preferred_format`, `log_level`, `notifications`. Provides `load()`, `save()`, and `write_default()` methods.

### `context_menu.rs` — Registry Integration
Registers/unregisters Explorer context-menu entries:
- **Folder background**: "Paste link" under `HKCU\...\Directory\Background\shell\PasteLink`
- **Video file extensions**: "Convert to Compatible" and "Compress" under `HKCU\...\SystemFileAssociations\.<ext>\shell\PasteLinkConvert` and `PasteLinkCompress` for each of `.mp4`, `.mkv`, `.webm`, `.avi`, `.mov`, `.ts`, `.flv`, `.m4v`, `.wmv`

Uses the `windows` crate for raw Win32 registry APIs. Calls `SHChangeNotify(SHCNE_ASSOCCHANGED)` so Explorer refreshes immediately. Supports `MultiSelectModel=Player` for multi-file selection.

### `download_gui.rs` — Download Progress Window
A native Win32 window (no framework) with: platform/URL/directory labels, a progress bar (smooth mode for yt-dlp percentage, marquee mode for gallery-dl/starting), status text with colour coding, Cancel/Close button, and Open Folder button (enabled on success). Architecture: GUI thread runs a Win32 message loop with `WM_TIMER` polling every 80ms; a worker thread runs `download()` and updates shared `Arc<Mutex<GuiState>>`. Dark grayscale colour palette.

### `advanced_download_gui.rs` — Advanced Download Options
An eframe/egui window shown when the user holds Shift while clicking "Paste link". Provides controls for: video/audio stream toggles, audio format (mp3/m4a/opus/flac/wav for audio-only), resolution cap (Best to 360p), audio bitrate (Best to 96k), and start/end time trimming. Returns `Some(AdvancedOptions)` on Download or `None` on Cancel.

### `downloader.rs` — Download Engine
Core download logic. Resolves yt-dlp, FFmpeg, and gallery-dl binary paths with a 3-tier strategy: config override → bundled (`<exe_dir>/bin/`) → system PATH. Spawns yt-dlp as a child process with `CREATE_NO_WINDOW`, pipes stdout/stderr, and streams output line-by-line to a progress callback. Supports cooperative cancellation via `Arc<AtomicBool>`. Format selection prioritises H.264 + AAC with fallback tiers. The `build_yt_dlp_args()` function handles advanced options: audio-only extraction, resolution capping, trim via `--download-sections`. Also contains `download_images()` for gallery-dl and `run_diagnostics()`.

### `postprocess.rs` — Post-processing Engine
FFmpeg-based batch video processing with two operations:
- **ConvertCompatible**: H.264 + AAC → `<name>_edit.mp4` (NLE-ready)
- **Compress**: HEVC + AAC → `small/<name>.mp4` (storage-efficient)

Detects NVENC GPU encoders (`h264_nvenc`, `hevc_nvenc`) at runtime with CPU fallback. Parses FFmpeg's `time=` stderr output for progress reporting. Supports single-file, multi-file, and directory scanning modes. Skips already-processed files (`_edit` suffix or existing in `small/`).

### `postprocess_gui.rs` — Post-processing Progress Window
An eframe/egui window showing: operation name, directory/file path, current file name with index, progress bar (per-file mapped to overall), status text, results summary (success/skipped/failed counts), Cancel/Close button, and Open Folder button. Worker thread communicates via `Arc<Mutex<PostprocessState>>` with `ctx.request_repaint()`. Supports three entry points: directory scan, explicit file list, and single file.

### `app_icon.rs` — Application Icon
Loads the embedded 32×32 PNG (`ico/32.png`) at compile time via `include_bytes!()` and decodes it with the `image` crate. Returns `egui::IconData` for use with `ViewportBuilder::with_icon()`. The .exe icon itself is embedded separately via `resources.rc` + `embed-resource`.

### `error.rs` — Error Types
Central `AppError` enum using `thiserror`. Variants cover: clipboard errors, URL validation failures, missing binaries (yt-dlp, FFmpeg, gallery-dl), directory issues, process spawn failures, yt-dlp/gallery-dl exit codes, download failures, cancellation, registry errors, config errors, and I/O errors. Every variant has a user-facing error message.

### `logging.rs` — Tracing Setup
Initialises a `tracing-subscriber` with a daily-rotating file appender (`tracing-appender`) writing to `%LOCALAPPDATA%\PasteLinkDownloader\logs\app.log`. In debug builds, also writes to stderr with ANSI colours. The non-blocking writer guard is intentionally leaked (`mem::forget`) to keep the background writer thread alive for the process lifetime.

### `notification.rs` — User Notifications
Wraps Win32 `MessageBoxW` for success, error, and cancellation dialogs. All functions take plain `&str` and handle UTF-8 → UTF-16 conversion internally.

### `platform.rs` — URL Validation & Platform Detection
Validates URL scheme (HTTPS only) using the `url` crate. Detects platform from hostname: YouTube (6 domains), Pinterest (25+ country domains + pin.it), Instagram (2 domains). Unknown hosts return `Platform::Unsupported` (not an error) — yt-dlp still attempts the download. All host matching is case-insensitive with `www.` prefix stripping.

### `progress.rs` — yt-dlp Output Parser
Parses yt-dlp's `--newline` stdout output into typed `ProgressEvent` variants: `Percent(f64)`, `Speed(String)`, `Eta(String)`, `Merging(String)`, `Warning(String)`, `Error(String)`, `Complete`, `Other(String)`. Uses lazy-compiled `Regex` patterns via `OnceLock` for efficiency.

### `settings_gui.rs` — Settings Window
An eframe/egui settings window with: edit fields for yt-dlp/FFmpeg/gallery-dl/cookies-file paths (with Browse file picker buttons via `rfd`), combo boxes for cookie browser/output format/log level, a notifications checkbox, Save/Cancel/Open Config Folder buttons. Reads the current `Config` on open, writes back to TOML on Save. Dark grayscale theme matching all other GUI windows.

---

## Adobe CEP Plugin

The `plugin/` directory contains a CEP (Common Extensibility Platform) panel for **Premiere Pro CC 2019+** and **After Effects CC 2019+**.

### What it does
- Adds a **Video Yoinker** panel under Window → Extensions
- User pastes a URL, selects format, clicks **Download & Import**
- The panel spawns `paste-link-downloader.exe` as a Node.js child process
- Downloaded file is auto-imported into a project bin named "Downloaded"
- Shows real-time progress bar and log output in the panel

### Plugin architecture
- **`manifest.xml`** — CEP bundle definition, enables Node.js (`--enable-nodejs`) for child process spawning
- **`index.html`** — Panel UI with dark theme
- **`panel.css`** — Dark themed styles matching Adobe's UI
- **`cep_init.js`** — Loads the CSInterface library
- **`downloader.js`** — Spawns the Rust binary, parses stdout for progress
- **`main.js`** — UI controller: URL validation, output directory auto-detection from active project, download orchestration
- **`host.jsx`** — ExtendScript: creates bins, imports files into the project

### Installing the plugin

```powershell
.\install-plugin.ps1            # Install
.\install-plugin.ps1 -Uninstall # Uninstall
```

Or use the master install script which includes the plugin:
```powershell
.\install.ps1
```

See [`plugin/README-INSTALL.md`](plugin/README-INSTALL.md) for manual installation instructions.

---

## Running Tests

```powershell
cargo test
```

All tests run offline with no network access. Test coverage:
- **`platform_tests.rs`** — URL validation + platform detection for YouTube, Pinterest, Instagram, and unsupported domains
- **`url_tests.rs`** — Edge cases: empty strings, HTTP rejection, shell injection attempts, Unicode paths
- **`filename_tests.rs`** — Output template path construction safety (spaces, Unicode, long paths, traversal)

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `windows` 0.58 | Win32 API bindings (registry, UI, shell, console, GDI, keyboard) |
| `eframe` 0.28 | Native GUI framework for settings, advanced options, post-processing windows |
| `egui` 0.28 | Immediate-mode UI library (used by eframe) |
| `rfd` 0.17 | Native file dialog for browse buttons |
| `image` 0.25 | PNG decode for embedded app icon |
| `embed-resource` 3 | Build-time resource file compiler (app icon in .exe) |
| `url` 2 | URL parsing and validation |
| `arboard` 3 | Cross-platform clipboard access |
| `thiserror` 1 | Derive macro for error types |
| `anyhow` 1 | Error context propagation |
| `serde` 1 | Serialization/deserialization framework |
| `toml` 0.8 | TOML config file parsing |
| `tracing` 0.1 | Structured logging |
| `tracing-subscriber` 0.3 | Log formatting and filtering |
| `tracing-appender` 0.2 | Rolling file log appender |
| `regex` 1 | yt-dlp output line parsing |

---

## Release Build Procedure

```powershell
# 1. Build
cargo build --release --target x86_64-pc-windows-msvc

# 2. Collect artifacts
$out = "dist\paste-link-downloader"
New-Item -ItemType Directory -Force $out
Copy-Item "target\x86_64-pc-windows-msvc\release\paste-link-downloader.exe" $out
Copy-Item -Recurse "bin" $out

# 3. Distribute the dist\ folder
# Users run: paste-link-downloader.exe --install
```

---

## License

MIT
