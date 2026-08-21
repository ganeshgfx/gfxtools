# Paste Link Downloader

A native Windows application that adds a **"Paste link"** entry to the Windows Explorer context menu, allowing you to download YouTube, Pinterest, and Instagram videos by right-clicking inside any folder.

## Features

- Right-click empty space in any folder → **Paste link**
- Downloads from YouTube, Pinterest, Instagram (and any site yt-dlp supports)
- Uses `yt-dlp` + `FFmpeg` — no custom scrapers
- Saves directly into the Explorer folder you right-clicked in
- Shows download progress in a console window
- Success/failure notification dialogs
- Per-user installation — no administrator privileges needed
- Full install / uninstall via `--install` / `--uninstall` flags
- Configurable via `%APPDATA%\PasteLinkDownloader\config.toml`

---

## Requirements

- Windows 10 or Windows 11
- Rust toolchain (for building): https://rustup.rs
  - Target: `x86_64-pc-windows-msvc`
- `yt-dlp.exe` — https://github.com/yt-dlp/yt-dlp/releases/latest
- `ffmpeg.exe` + `ffprobe.exe` — https://www.gyan.dev/ffmpeg/builds/

---

## Build

```powershell
# Install Rust (if not already installed)
winget install Rustlang.Rustup

# Build release binary (uses GNU toolchain — no VS Build Tools required)
cargo build --release
```

The binary will be at:
```
target\release\paste-link-downloader.exe
```

> **MSVC target (optional):** If you have Visual Studio 2022 Build Tools installed with the
> "Desktop development with C++" workload, you can build a native MSVC binary instead:
> ```powershell
> rustup default stable-x86_64-pc-windows-msvc
> cargo build --release
> ```

---

## Install

### 1. Place required executables

```
bin\
├── yt-dlp.exe
├── ffmpeg.exe
└── ffprobe.exe
```

See [`bin/README.md`](bin/README.md) for download links.

### 2. Run installer

```powershell
.\target\x86_64-pc-windows-msvc\release\paste-link-downloader.exe --install
```

This copies the application to:
```
%LOCALAPPDATA%\PasteLinkDownloader\
```
and registers the Explorer context menu entry.

### 3. Verify

```powershell
paste-link-downloader.exe --diagnostics
```

You should see:

```
=== Paste Link Downloader — Diagnostics ===

✓ yt-dlp  : C:\Users\...\PasteLinkDownloader\bin\yt-dlp.exe
✓ FFmpeg  : C:\Users\...\PasteLinkDownloader\bin\ffmpeg.exe
  Log dir  : C:\Users\...\AppData\Local\PasteLinkDownloader\logs
  Config   : C:\Users\...\AppData\Roaming\PasteLinkDownloader\config.toml
```

---

## Usage

1. Copy a video URL to the clipboard:
   - `https://www.youtube.com/watch?v=...`
   - `https://youtu.be/...`
   - `https://www.pinterest.com/pin/...`
   - `https://pin.it/...`
   - `https://www.instagram.com/reel/...`
   - `https://www.instagram.com/p/...`

2. Open any folder in Windows Explorer.

3. Right-click on **empty space** in the folder.

4. Click **Paste link**.

5. A console window shows download progress.

6. A notification confirms success or reports an error.

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

```toml
# Path to yt-dlp.exe (empty = auto-detect: bundled → PATH)
yt_dlp_path = ""

# Directory containing ffmpeg.exe (empty = auto-detect: bundled → PATH)
ffmpeg_dir = ""

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

Daily rotation. Old log files have date suffixes.

---

## Security Notes

- Clipboard contents are **never** executed as shell commands.
- `yt-dlp` is invoked directly (`Command::new(yt_dlp_path).arg(url)`), not via `cmd.exe`.
- URL scheme is validated (`https` only).
- No binaries are auto-downloaded from the internet.
- HKCU registry only — no system-wide changes, no UAC prompts.

---

## Filename Collision Handling

yt-dlp is invoked with `--no-overwrites`. If a file with the same name already exists, yt-dlp automatically appends a counter suffix:

```
My Video.mp4
My Video (1).mp4
My Video (2).mp4
```

---

## Download Quality

Default format selection:

```
-f bestvideo*+bestaudio/best --merge-output-format mp4
```

This selects the best available video+audio streams and merges them into an MP4 container using FFmpeg. If a site only provides a combined stream, yt-dlp uses that directly.

---

## Project Structure

```
paste-link-downloader/
├── Cargo.toml
├── build.rs
├── README.md
├── bin/
│   ├── README.md          ← instructions for placing yt-dlp/ffmpeg
│   ├── yt-dlp.exe         ← you provide
│   ├── ffmpeg.exe         ← you provide
│   └── ffprobe.exe        ← you provide
├── src/
│   ├── main.rs            ← entry point, CLI dispatch
│   ├── lib.rs             ← public API for tests
│   ├── cli.rs             ← argument parsing
│   ├── clipboard.rs       ← clipboard reading
│   ├── config.rs          ← TOML config
│   ├── context_menu.rs    ← registry install/uninstall
│   ├── downloader.rs      ← yt-dlp process management
│   ├── error.rs           ← error types
│   ├── logging.rs         ← tracing setup
│   ├── notification.rs    ← MessageBox dialogs
│   ├── platform.rs        ← URL validation + platform detection
│   └── progress.rs        ← yt-dlp output parser
└── tests/
    ├── platform_tests.rs
    ├── url_tests.rs
    └── filename_tests.rs
```

---

## Running Tests

```powershell
cargo test
```

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
