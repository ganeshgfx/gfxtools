---
name: video-yoinker-project
description: >
  Comprehensive project context for the GFX Tools codebase.
  Read this skill when working on any part of this project to understand the architecture,
  module responsibilities, conventions, and key design decisions.
---

# GFX Tools — Project Context for AI Agents

## What Is This Project?

**GFX Tools** is a native Windows desktop application written in **Rust** that adds a **"Paste link"** entry to the Windows Explorer right-click context menu. Users copy a video URL to their clipboard, right-click empty space in any folder, and click "Paste link" — the app downloads the video into that folder using `yt-dlp` + `FFmpeg`, with a native Win32 GUI showing progress.

The project also includes:
- **Extended context menus** on video files: **"Convert to Compatible"** (H.264+AAC for NLEs) and **"Compress"** (HEVC for storage).
- An **Adobe CEP plugin** for Premiere Pro and After Effects that provides download + auto-import functionality.
- **Advanced download options** via Shift+Click: resolution cap, audio-only/video-only, audio format, bitrate, and time trimming.
- An **Inno Setup installer** (`installer.iss`) that auto-downloads yt-dlp, FFmpeg, and gallery-dl post-install via `scripts/download-deps.ps1`.
- **Chocolatey** and **winget** distribution packages.
- **GitHub Actions CI** for automated release builds.

## Tech Stack

- **Language**: Rust (edition 2021)
- **GUI**: Mixed — native Win32 API (`windows` crate 0.58) for the download progress window, **eframe/egui** 0.28 for settings, advanced download options, and post-processing GUIs
- **Build**: Cargo, with `build.rs` setting Windows subsystem + compiling the resource file (app icon)
- **Target**: `x86_64-pc-windows-msvc` (primary) or `x86_64-pc-windows-gnu`
- **External tools**: yt-dlp, FFmpeg (+ optional NVENC GPU), gallery-dl (optional)
- **Plugin**: Adobe CEP (HTML/CSS/JS + ExtendScript) with Node.js child process spawning
- **Icon embedding**: `embed-resource` crate compiles `resources.rc` → embeds `ico/main.ico` into the .exe; `app_icon.rs` loads `ico/32.png` at runtime for eframe window icons
- **Installer**: Inno Setup 6 (`installer.iss`), packaged via `scripts/build-installer.ps1`
- **Packages**: Chocolatey (`gfxtools.nuspec` + `tools/`), winget (`manifests/`)

## Crate Name vs Directory Name

- **Cargo package name**: `gfx-tools`
- **Library crate name**: `gfx_tools`
- **Binary name**: `gfx-tools`
- **Repository directory**: `video_yoinker`

## Architecture

### Entry Flow
1. `main.rs` → `cli::parse_args()` → `Command` enum variant
2. Allocate console only for CLI commands (download/postprocess/settings use GUI windows)
3. `Config::load()` → load from `%APPDATA%\GFXTools\config.toml`
4. `logging::init()` → rolling file appender to `%LOCALAPPDATA%\GFXTools\logs\`
5. Dispatch to handler: install, uninstall, download-gui, download-images, convert-compatible, compress, settings, diagnostics

### Download Flow (Primary)
1. `clipboard::read_clipboard_text()` → get URL from Windows clipboard
2. `platform::validate_and_detect()` → HTTPS validation + platform categorisation
3. If Shift held: `advanced_download_gui::show_advanced_options()` → eframe window for resolution/audio/trim options
4. `download_gui::run_download_window()` → opens native Win32 progress window
5. Worker thread: `downloader::download()` → spawn yt-dlp → stream stdout to progress callback
6. If yt-dlp fails: fallback to `downloader::download_images()` → spawn gallery-dl
7. GUI polls shared `Arc<Mutex<GuiState>>` via `WM_TIMER` every 80ms

### Post-processing Flow (Convert to Compatible / Compress)
1. Context menu fires `--convert-compatible "%1"` or `--compress "%1"` per selected file
2. Single file → batch collection via named mutex leader/follower pattern (multi-select support)
3. Leader waits ~500ms for follower processes to append paths to temp batch file
4. `postprocess_gui::run_postprocess_window()` → eframe/egui progress window
5. Worker thread: `postprocess::run_postprocess()` → FFmpeg per file, with NVENC GPU detection
6. ConvertCompatible: H.264+AAC `_edit.mp4` alongside original
7. Compress: HEVC+AAC into `small/` subfolder

### Binary Resolution (3-tier)
All external tools (yt-dlp, FFmpeg, gallery-dl) are resolved in order:
1. Explicit path from `config.toml`
2. Bundled: `<exe_dir>/bin/<tool>.exe`
3. System PATH

### Security Model
- URLs are **never** shell-interpolated — always passed as literal `Command::arg()`
- Clipboard contents are treated as untrusted user data
- Only HTTPS URLs are accepted (http/ftp/javascript rejected)
- Child processes spawned with `CREATE_NO_WINDOW` flag
- HKCU registry only — no admin, no system-wide changes

## Module Reference

| Module | File | Responsibility |
|--------|------|----------------|
| Entry point | `src/main.rs` | CLI dispatch, install/uninstall, download orchestration, postprocess batch collection, console allocation, Ctrl+C handler, Shift detection |
| Library API | `src/lib.rs` | Re-exports all modules for integration tests |
| CLI parsing | `src/cli.rs` | `Command` enum, `parse_args()`, `print_usage()` — no external CLI framework |
| Clipboard | `src/clipboard.rs` | Read clipboard text via `arboard` crate, trim, empty check |
| Config | `src/config.rs` | `Config` struct with serde, TOML load/save, default values, `%APPDATA%` path resolution |
| Context menu | `src/context_menu.rs` | HKCU registry: folder background "Paste link", per-extension "Convert to Compatible" / "Compress" via `SystemFileAssociations`, `SHChangeNotify` |
| Download GUI | `src/download_gui.rs` | Native Win32 progress window: labels, progress bar, cancel/open buttons, `WM_TIMER` polling, dark grayscale theme |
| Advanced Download GUI | `src/advanced_download_gui.rs` | eframe/egui options window (Shift+Click): video/audio toggles, audio format, resolution cap, audio bitrate, start/end time trim |
| Downloader | `src/downloader.rs` | Core engine: resolve binaries, spawn yt-dlp/gallery-dl, pipe stdout/stderr, cancellation, format selection with H.264 priority, FFmpeg post-processing args, `AdvancedOptions` / `DownloadOptions` structs |
| Post-processing | `src/postprocess.rs` | FFmpeg-based batch operations: ConvertCompatible (H.264+AAC) and Compress (HEVC+AAC), NVENC GPU detection with CPU fallback, progress via `time=` stderr parsing, `find_video_files()` |
| Post-processing GUI | `src/postprocess_gui.rs` | eframe/egui progress window for post-processing: per-file + overall progress, cancel, open-folder, results summary |
| App icon | `src/app_icon.rs` | Loads embedded 32x32 PNG icon for eframe/egui `ViewportBuilder::with_icon()` |
| Errors | `src/error.rs` | `AppError` enum with `thiserror` — clipboard, URL, binary, directory, process, registry, config, I/O, cancellation variants |
| Logging | `src/logging.rs` | `tracing-subscriber` + `tracing-appender` daily rolling file, stderr in debug builds, guard leak pattern |
| Notifications | `src/notification.rs` | Win32 `MessageBoxW` wrapper (success/error/cancelled), UTF-8→UTF-16 conversion |
| Platform | `src/platform.rs` | `Platform` enum (YouTube/Pinterest/Instagram/Unsupported), URL scheme validation, host detection with `www.` stripping |
| Progress | `src/progress.rs` | `ProgressEvent` enum, regex-based yt-dlp stdout parser (`OnceLock` for lazy compilation) |
| Settings GUI | `src/settings_gui.rs` | eframe/egui settings window: edit fields with Browse buttons, combo boxes, checkbox, Save/Cancel — reads/writes `Config` |

## Key Dependencies

- **`windows` 0.58**: Win32 API (Registry, UI, Shell, Console, GDI, LibraryLoader, Threading, Security, KeyboardAndMouse)
- **`eframe` 0.28 + `egui` 0.28**: Immediate-mode GUI for settings, advanced download options, and post-processing windows
- **`rfd` 0.17**: Native file dialog for browse buttons in settings GUI
- **`image` 0.25** (PNG only): Decode embedded app icon for eframe windows
- **`embed-resource` 3** (build-dep): Compile `resources.rc` to embed app icon in .exe
- **`arboard` 3**: Clipboard read
- **`url` 2**: URL parse + validation
- **`serde` + `toml` 0.8**: Config serialization
- **`thiserror` 1**: Error derive macro
- **`anyhow` 1**: Error context
- **`tracing` + `tracing-subscriber` + `tracing-appender`**: Structured logging
- **`regex` 1**: yt-dlp output parsing

## Filesystem Locations (Runtime)

| Path | Content |
|------|---------|
| `%LOCALAPPDATA%\GFXTools\` | Installed exe + `bin/` folder |
| `%LOCALAPPDATA%\GFXTools\logs\app.log` | Daily rotating log file |
| `%APPDATA%\GFXTools\config.toml` | User configuration |
| `HKCU\Software\Classes\Directory\Background\shell\GFXTools` | "Paste link" context menu |
| `HKCU\Software\Classes\SystemFileAssociations\.<ext>\shell\GFXToolsConvert` | "Convert to Compatible" per video extension |
| `HKCU\Software\Classes\SystemFileAssociations\.<ext>\shell\GFXToolsCompress` | "Compress" per video extension |
| `%APPDATA%\Adobe\CEP\extensions\GFXTools\` | CEP plugin (if installed) |

## Scripts Reference

All scripts are in `scripts/` (moved from repo root in the latest refactor).

| Script | Purpose |
|--------|---------|
| `scripts/install.ps1` | Master installer: cargo build → uninstall old → install new context menu + CEP plugin. Supports `-SkipBuild`, `-SkipPlugin`, `-SkipContextMenu`. `$RepoRoot` is `Split-Path $PSScriptRoot -Parent`. |
| `scripts/install-plugin.ps1` | CEP-only: enable debug mode, copy plugin to Adobe extensions dir. Supports `-Uninstall` |
| `scripts/build-installer.ps1` | Build release binary + run Inno Setup (`ISCC.exe`) to produce `Output\GFXTools_Installer.exe`. Supports `-SkipBuild`, `-SkipInstaller`. Searches common ISCC paths. |
| `scripts/download-deps.ps1` | Downloads yt-dlp, FFmpeg (BtbN builds), gallery-dl in parallel background jobs using `curl.exe`. Accepts `-InstallDir` (used by Inno Setup post-install hook). Best-effort — one failure does not abort. Writes log to `<dir>\deps-download.log`. |
| `scripts/get.ps1` | Minimal bootstrap script |

## Installer (Inno Setup)

`installer.iss` produces `Output\GFXTools_Installer.exe`:
- Installs to `{localappdata}\GFXTools` (no admin required — `PrivilegesRequired=lowest`)
- Copies `gfx-tools.exe` and `scripts\download-deps.ps1`
- Copies entire `plugin\` to `%APPDATA%\Adobe\CEP\extensions\GFXTools\`
- Sets `PlayerDebugMode=1` in registry for Adobe CSXS.10 through CSXS.16
- `[Run]`: calls `gfx-tools.exe install` then runs `download-deps.ps1 -InstallDir {app}` hidden
- `[UninstallRun]`: calls `gfx-tools.exe uninstall` hidden
- Start Menu shortcuts: Settings, Diagnostics, Uninstall
- Auto-detects and offers to uninstall previous version via `InitializeSetup()` Pascal code

## Distribution Packages

- **Chocolatey**: `gfxtools.nuspec` + `tools/chocolateyInstall.ps1` (runs installer silently with `/VERYSILENT`) + `tools/chocolateyUninstall.ps1` (runs `unins000.exe /SILENT`) + `tools/VERIFICATION.txt`
- **Winget**: manifests in `manifests/` directory (GaneshGFX.GFXTools)
- **GitHub Actions**: `.github/workflows/` — automated builds and release packaging on tag push

## Test Structure

All tests are offline (no network). Located in `tests/`:
- **`platform_tests.rs`**: URL validation + platform detection across all supported domains
- **`url_tests.rs`**: Edge cases (empty, HTTP, injection, Unicode)
- **`filename_tests.rs`**: Output template path safety

Run with `cargo test`.

## Adobe CEP Plugin

Located in `plugin/`. A CEP panel for Premiere Pro / After Effects:
- Uses Node.js (`--enable-nodejs` in manifest) to spawn `gfx-tools.exe`
- `downloader.js` spawns the Rust binary as child process, parses stdout
- `host.jsx` (ExtendScript) creates project bins and imports downloaded files
- `main.js` handles UI: URL validation, output dir auto-detection from active project
- Install via `scripts\install-plugin.ps1` which enables CEP debug mode and copies to Adobe CEP extensions dir

## Conventions & Patterns

1. **No `cmd.exe`**: All child processes spawned directly via `Command::new(path)`, never through a shell
2. **`CREATE_NO_WINDOW`**: All child processes use this Windows flag to prevent console flashing
3. **Error propagation**: Functions return `Result<(), AppError>` using the central error enum
4. **Config defaults**: All config fields have `serde(default)` — app works without any config file
5. **Win32 GUI pattern** (download_gui only): Register window class → `CreateWindowExW` → message loop → `GWLP_USERDATA` for per-window state
6. **eframe/egui GUI pattern** (settings, advanced options, postprocess): `eframe::run_native()` → `App::update()` → shared state via `Arc<Mutex<T>>` with worker thread
7. **Shared state**: GUI and worker threads communicate via `Arc<Mutex<State>>` + `Arc<AtomicBool>` for cancellation
8. **UTF-16 conversion**: Win32-calling modules have their own `to_wide()` / `w()` helper
9. **Codec preference**: Downloads prefer H.264 + AAC for NLE compatibility; VP9/AV1/VP8 excluded from preferred tiers; all output post-processed with `libx264 + AAC`
10. **yt-dlp → gallery-dl fallback**: If yt-dlp fails, gallery-dl is attempted as fallback for image galleries
11. **Cookie injection**: `cookies_file` takes priority over `cookies_from_browser`; Chrome 127+ App-Bound Encryption is documented as a known limitation
12. **NVENC GPU acceleration**: Post-processing detects `h264_nvenc` / `hevc_nvenc` at runtime; falls back to CPU `libx264` / `libx265`
13. **Multi-select batch processing**: Named mutex (`Local\GFXToolsBatch_<op>`) + temp file to collect paths from concurrent Explorer-spawned processes into a single GUI window
14. **App icon**: Embedded at compile time via `resources.rc` (Win32 exe icon) and `app_icon.rs` (eframe window icon from `ico/32.png`)
15. **Dark grayscale theme**: All GUIs (Win32 and eframe) share a consistent dark theme with bg=#1C1C1C, surface=#272727, accent=#888888
16. **Scripts in `scripts/`**: All PowerShell scripts live under `scripts/` (not root). `$RepoRoot = Split-Path $PSScriptRoot -Parent` resolves the repo root from inside any script.

## Common Tasks

### Adding a new CLI command
1. Add variant to `Command` enum in `cli.rs`
2. Add pattern match in `parse_args()` in `cli.rs`
3. Add dispatch case in `main()` in `main.rs`
4. Update `print_usage()` in `cli.rs`
5. If GUI: skip console allocation in the `match &command` block in `main()`

### Adding a new platform
1. Add variant to `Platform` enum in `platform.rs`
2. Add host matching patterns in `detect_platform()`
3. Add tests in `platform.rs` unit tests and `tests/platform_tests.rs`

### Adding a new config field
1. Add field to `Config` struct in `config.rs` with `#[serde(default)]` or `#[serde(default = "fn")]`
2. Add default value in `impl Default for Config`
3. Add control in `settings_gui.rs` (eframe UI code)
4. Use the field in the relevant module (e.g., `downloader.rs`)

### Adding a new post-processing operation
1. Add variant to `PostprocessOp` enum in `postprocess.rs`
2. Add `Display` impl for the new variant
3. Add FFmpeg encoder/args logic in `run_postprocess_files()` and `run_postprocess_single()`
4. Add CLI variant in `cli.rs` + dispatch in `main.rs`
5. Add context menu registration in `context_menu.rs` `install_extended()`

### Modifying the download GUI (Win32)
- All Win32 window code is in `download_gui.rs`
- Control IDs are constants at the top of the file
- Layout uses absolute pixel positioning (no layout manager)
- Colors defined as `const` tuples at the top
- The `wnd_proc` unsafe extern function handles all window messages

### Modifying eframe/egui GUIs (settings, advanced options, postprocess)
- Each GUI has its own `setup_custom_styles()` that sets the dark grayscale theme
- State shared with worker threads via `Arc<Mutex<State>>` — call `ctx.request_repaint()` after mutations
- Use `crate::app_icon::load_icon()` for window icon
- Window size set in `NativeOptions::viewport`

### Adding a new context menu entry
1. Add registry path constants in `context_menu.rs`
2. Add creation logic in `install()` or `install_extended()`
3. Add cleanup logic in `uninstall()` or `uninstall_extended()`
4. Call `notify_shell()` after changes

### Updating install scripts
- Scripts live in `scripts/`, not at repo root
- `$RepoRoot = Split-Path $PSScriptRoot -Parent` inside scripts gives you the repo root
- `$ReleaseBin = Join-Path $Root "target\release\gfx-tools.exe"`
- Inno Setup output goes to `Output\GFXTools_Installer.exe` (gitignored)
- `download-deps.ps1` uses parallel PowerShell jobs and `curl.exe` (Windows built-in); logs to `deps-download.log`
