---
name: video-yoinker-project
description: >
  Comprehensive project context for the Paste Link Downloader (Video Yoinker) codebase.
  Read this skill when working on any part of this project to understand the architecture,
  module responsibilities, conventions, and key design decisions.
---

# Video Yoinker — Project Context for AI Agents

## What Is This Project?

**Paste Link Downloader** (also called **Video Yoinker**) is a native Windows desktop application written in **Rust** that adds a **"Paste link"** entry to the Windows Explorer right-click context menu. Users copy a video URL to their clipboard, right-click empty space in any folder, and click "Paste link" — the app downloads the video into that folder using `yt-dlp` + `FFmpeg`, with a native Win32 GUI showing progress.

The project also includes an **Adobe CEP plugin** for Premiere Pro and After Effects that provides the same download functionality as an in-NLE panel with auto-import into project bins.

## Tech Stack

- **Language**: Rust (edition 2021)
- **GUI**: Native Win32 API via `windows` crate 0.58 — no GUI framework (no egui, no winit, no GTK)
- **Build**: Cargo, with `build.rs` setting Windows subsystem to "windows" in release builds
- **Target**: `x86_64-pc-windows-msvc` (primary) or `x86_64-pc-windows-gnu`
- **External tools**: yt-dlp, FFmpeg, gallery-dl (optional)
- **Plugin**: Adobe CEP (HTML/CSS/JS + ExtendScript) with Node.js child process spawning

## Crate Name vs Directory Name

- **Cargo package name**: `paste-link-downloader`
- **Library crate name**: `paste_link_downloader`
- **Binary name**: `paste-link-downloader`
- **Repository directory**: `video_yoinker`

## Architecture

### Entry Flow
1. `main.rs` → `cli::parse_args()` → `Command` enum variant
2. Allocate console only for CLI commands (download uses GUI window)
3. `Config::load()` → load from `%APPDATA%\PasteLinkDownloader\config.toml`
4. `logging::init()` → rolling file appender to `%LOCALAPPDATA%\PasteLinkDownloader\logs\`
5. Dispatch to handler: install, uninstall, download-gui, download-images, settings, diagnostics

### Download Flow (Primary)
1. `clipboard::read_clipboard_text()` → get URL from Windows clipboard
2. `platform::validate_and_detect()` → HTTPS validation + platform categorisation
3. `download_gui::run_download_window()` → opens native Win32 progress window
4. Worker thread: `downloader::download()` → spawn yt-dlp → stream stdout to progress callback
5. If yt-dlp fails: fallback to `downloader::download_images()` → spawn gallery-dl
6. GUI polls shared `Arc<Mutex<GuiState>>` via `WM_TIMER` every 80ms

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
| Entry point | `src/main.rs` | CLI dispatch, install/uninstall, download orchestration, console allocation, Ctrl+C handler |
| Library API | `src/lib.rs` | Re-exports all modules for integration tests |
| CLI parsing | `src/cli.rs` | `Command` enum, `parse_args()`, `print_usage()` — no external CLI framework |
| Clipboard | `src/clipboard.rs` | Read clipboard text via `arboard` crate, trim, empty check |
| Config | `src/config.rs` | `Config` struct with serde, TOML load/save, default values, `%APPDATA%` path resolution |
| Context menu | `src/context_menu.rs` | HKCU registry read/write for Explorer shell extension, `SHChangeNotify` |
| Download GUI | `src/download_gui.rs` | Native Win32 progress window: labels, progress bar (smooth/marquee), cancel/open buttons, `WM_TIMER` polling, dark grayscale theme |
| Downloader | `src/downloader.rs` | Core engine: resolve binaries, spawn yt-dlp/gallery-dl, pipe stdout/stderr, cancellation, format selection with H.264 priority, FFmpeg post-processing args |
| Errors | `src/error.rs` | `AppError` enum with `thiserror` — clipboard, URL, binary, directory, process, registry, config, I/O variants |
| Logging | `src/logging.rs` | `tracing-subscriber` + `tracing-appender` daily rolling file, stderr in debug builds, guard leak pattern |
| Notifications | `src/notification.rs` | Win32 `MessageBoxW` wrapper (success/error/cancelled), UTF-8→UTF-16 conversion |
| Platform | `src/platform.rs` | `Platform` enum (YouTube/Pinterest/Instagram/Unsupported), URL scheme validation, host detection with `www.` stripping |
| Progress | `src/progress.rs` | `ProgressEvent` enum, regex-based yt-dlp stdout parser (`OnceLock` for lazy compilation) |
| Settings GUI | `src/settings_gui.rs` | Native Win32 settings window: edit fields with Browse buttons, combo boxes, checkbox, Save/Cancel — reads/writes `Config` |

## Key Dependencies

- **`windows` 0.58**: Win32 API (Registry, UI, Shell, Console, GDI, LibraryLoader, Threading, Security)
- **`arboard` 3**: Clipboard read
- **`url` 2**: URL parse + validation
- **`serde` + `toml` 0.8**: Config serialization
- **`thiserror` 1**: Error derive macro
- **`tracing` + `tracing-subscriber` + `tracing-appender`**: Structured logging
- **`regex` 1**: yt-dlp output parsing

## Filesystem Locations (Runtime)

| Path | Content |
|------|---------|
| `%LOCALAPPDATA%\PasteLinkDownloader\` | Installed exe + `bin/` folder |
| `%LOCALAPPDATA%\PasteLinkDownloader\logs\app.log` | Daily rotating log file |
| `%APPDATA%\PasteLinkDownloader\config.toml` | User configuration |
| `HKCU\Software\Classes\Directory\Background\shell\PasteLink` | Context menu registry key |
| `%APPDATA%\Adobe\CEP\extensions\VideoYoinker\` | CEP plugin (if installed) |

## Test Structure

All tests are offline (no network). Located in `tests/`:
- **`platform_tests.rs`**: URL validation + platform detection across all supported domains
- **`url_tests.rs`**: Edge cases (empty, HTTP, injection, Unicode)
- **`filename_tests.rs`**: Output template path safety

Run with `cargo test`.

## Adobe CEP Plugin

Located in `plugin/`. A CEP panel for Premiere Pro / After Effects:
- Uses Node.js (`--enable-nodejs` in manifest) to spawn `paste-link-downloader.exe`
- `downloader.js` spawns the Rust binary as child process, parses stdout
- `host.jsx` (ExtendScript) creates project bins and imports downloaded files
- `main.js` handles UI: URL validation, output dir auto-detection from active project
- Install via `install-plugin.ps1` which enables CEP debug mode and copies to Adobe CEP extensions dir

## Conventions & Patterns

1. **No `cmd.exe`**: All child processes spawned directly via `Command::new(path)`, never through a shell
2. **`CREATE_NO_WINDOW`**: All child processes use this Windows flag to prevent console flashing
3. **Error propagation**: Functions return `Result<(), AppError>` using the central error enum
4. **Config defaults**: All config fields have `serde(default)` — app works without any config file
5. **Win32 GUI pattern**: Register window class → `CreateWindowExW` → message loop → `GWLP_USERDATA` for per-window state
6. **Shared state**: GUI and worker threads communicate via `Arc<Mutex<GuiState>>` + `Arc<AtomicBool>` for cancellation
7. **UTF-16 conversion**: Every module that calls Win32 APIs has its own `to_wide()` / `w()` helper function
8. **Codec preference**: Downloads prefer H.264 + AAC for NLE compatibility; VP9/AV1/VP8 explicitly excluded from preferred tiers; all output post-processed with `libx264 + AAC`
9. **yt-dlp → gallery-dl fallback**: If yt-dlp fails, gallery-dl is attempted as a fallback for image galleries
10. **Cookie injection**: `cookies_file` takes priority over `cookies_from_browser`; Chrome 127+ App-Bound Encryption is documented as a known limitation

## Common Tasks

### Adding a new CLI command
1. Add variant to `Command` enum in `cli.rs`
2. Add pattern match in `parse_args()` in `cli.rs`
3. Add dispatch case in `main()` in `main.rs`
4. Update `print_usage()` in `cli.rs`

### Adding a new platform
1. Add variant to `Platform` enum in `platform.rs`
2. Add host matching patterns in `detect_platform()`
3. Add tests in `platform.rs` unit tests and `tests/platform_tests.rs`

### Adding a new config field
1. Add field to `Config` struct in `config.rs` with `#[serde(default)]` or `#[serde(default = "fn")]`
2. Add default value in `impl Default for Config`
3. Add control in `settings_gui.rs` (`create_controls()` + read in `save_config()`)
4. Use the field in the relevant module (e.g., `downloader.rs`)

### Modifying the download GUI
- All Win32 window code is in `download_gui.rs`
- Control IDs are constants at the top of the file
- Layout uses absolute pixel positioning (no layout manager)
- Colors defined as `const` tuples at the top
- The `wnd_proc` unsafe extern function handles all window messages
