// src/downloader.rs
// Core download engine.
//
// Responsibilities:
//  - Resolve yt-dlp and FFmpeg paths (bundled → config → PATH).
//  - Spawn yt-dlp as a child process (never via cmd.exe/shell).
//  - Stream stdout/stderr line-by-line to the progress callback.
//  - Support cooperative cancellation via `Arc<AtomicBool>`.
//  - Clean exit code handling.
//
// Security: URL is passed as a literal argument, never interpolated into a
// shell string. No Command::new("cmd") usage.

use crate::config::Config;
use crate::error::AppError;
use crate::progress::{parse_line, ProgressEvent};
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Win32 CREATE_NO_WINDOW flag — prevents child console apps from opening a terminal.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// ── Binary resolution ────────────────────────────────────────────────────────

/// Returns the directory of the currently running executable.
fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

/// Resolves the path to yt-dlp.exe.
///
/// Resolution order:
/// 1. Explicit `config.yt_dlp_path` (if non-empty and the file exists).
/// 2. `<exe_dir>/bin/yt-dlp.exe` (bundled).
/// 3. `yt-dlp` on the system PATH.
pub fn resolve_yt_dlp(config: &Config) -> Result<PathBuf, AppError> {
    // 1. Config override
    if !config.yt_dlp_path.is_empty() {
        let p = PathBuf::from(&config.yt_dlp_path);
        if p.exists() {
            info!("yt-dlp from config: {:?}", p);
            return Ok(p);
        }
        warn!("Configured yt_dlp_path {:?} not found; falling back", p);
    }

    // 2. Bundled
    if let Some(dir) = exe_dir() {
        let bundled = dir.join("bin").join("yt-dlp.exe");
        if bundled.exists() {
            info!("yt-dlp bundled: {:?}", bundled);
            return Ok(bundled);
        }
    }

    // 3. PATH
    if which_in_path("yt-dlp.exe").is_some() || which_in_path("yt-dlp").is_some() {
        info!("yt-dlp found on PATH");
        return Ok(PathBuf::from("yt-dlp"));
    }

    Err(AppError::MissingYtDlp)
}

/// Resolves the FFmpeg *directory* (containing ffmpeg.exe).
///
/// Resolution order:
/// 1. Explicit `config.ffmpeg_dir` (if non-empty and ffmpeg.exe exists inside).
/// 2. `<exe_dir>/bin/` (bundled).
/// 3. System PATH directory containing ffmpeg.exe.
pub fn resolve_ffmpeg_dir(config: &Config) -> Result<PathBuf, AppError> {
    // 1. Config override
    if !config.ffmpeg_dir.is_empty() {
        let dir = PathBuf::from(&config.ffmpeg_dir);
        if dir.join("ffmpeg.exe").exists() {
            info!("FFmpeg from config dir: {:?}", dir);
            return Ok(dir);
        }
        warn!("Configured ffmpeg_dir {:?} has no ffmpeg.exe; falling back", dir);
    }

    // 2. Bundled
    if let Some(exe_dir) = exe_dir() {
        let bin_dir = exe_dir.join("bin");
        if bin_dir.join("ffmpeg.exe").exists() {
            info!("FFmpeg bundled: {:?}", bin_dir);
            return Ok(bin_dir);
        }
    }

    // 3. PATH
    if let Some(ffmpeg_path) = which_in_path("ffmpeg.exe").or_else(|| which_in_path("ffmpeg")) {
        if let Some(dir) = ffmpeg_path.parent() {
            info!("FFmpeg on PATH at: {:?}", dir);
            return Ok(dir.to_path_buf());
        }
    }

    Err(AppError::MissingFfmpeg)
}

/// Checks whether `name` is findable on PATH; returns absolute path if so.
fn which_in_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

// ── Download execution ───────────────────────────────────────────────────────

/// Advanced per-download options, set via the Shift+Click options GUI.
#[derive(Debug, Clone)]
pub struct AdvancedOptions {
    /// Download the video stream.
    pub video: bool,
    /// Download the audio stream.
    pub audio: bool,
    /// Audio format for extract-audio (mp3, m4a, opus, flac, wav). Only used when audio-only.
    pub audio_format: Option<String>,
    /// Maximum video height (e.g. 1080, 720). None = best available.
    pub max_resolution: Option<u32>,
    /// Audio bitrate (e.g. "192k", "320k"). None = default.
    pub audio_bitrate: Option<String>,
    /// FFmpeg start time (e.g. "00:01:30" or "90").
    pub start_time: Option<String>,
    /// FFmpeg end time (e.g. "00:05:00" or "300").
    pub end_time: Option<String>,
}

/// Options for a single download.
#[derive(Debug, Clone)]
pub struct DownloadOptions {
    /// The video URL (already validated).
    pub url: String,
    /// Directory to save the video into.
    pub output_dir: PathBuf,
    /// Preferred output container format ("mp4", "mkv", …).
    pub format: String,
    /// Advanced per-download options from Shift+Click GUI. None = default behaviour.
    pub advanced: Option<AdvancedOptions>,
}

/// Progress callback type. Called from the reading thread.
pub type ProgressCallback = Box<dyn Fn(ProgressEvent) + Send + 'static>;

/// Build the yt-dlp `-f` format string, merge format, and postprocessor args
/// based on whether advanced options are present.
///
/// Returns `(format_str, merge_format, postprocessor_args, is_audio_only)`.
fn build_yt_dlp_args(opts: &DownloadOptions) -> (String, String, String, bool) {
    let default_format = concat!(
        "bestvideo[vcodec^=avc1]+bestaudio[ext=m4a]",
        "/bestvideo[vcodec^=avc1]+bestaudio",
        "/bestvideo[vcodec^=h264]+bestaudio[ext=m4a]",
        "/bestvideo[vcodec^=h264]+bestaudio",
        "/bestvideo[vcodec!*=vp9][vcodec!*=vp09][vcodec!*=av01][vcodec!*=vp8]+bestaudio",
        "/best[vcodec!*=vp9][vcodec!*=vp09][vcodec!*=av01]",
        "/bestvideo+bestaudio",
        "/best"
    );

    let default_pp = "ffmpeg:-c:v libx264 -preset fast -crf 18 -c:a aac -b:a 192k".to_string();

    let Some(ref adv) = opts.advanced else {
        // No advanced options — use defaults.
        return (default_format.to_string(), opts.format.clone(), default_pp, false);
    };

    let audio_bitrate = adv.audio_bitrate.as_deref().unwrap_or("192k");

    match (adv.video, adv.audio) {
        // Audio-only
        (false, true) => {
            let fmt = "bestaudio".to_string();
            // Don't pass codec postprocessor args — --extract-audio --audio-format
            // handles the conversion. Passing -c:a aac would conflict with mp3/flac/etc.
            let pp = String::new();
            (fmt, opts.format.clone(), pp, true)
        }
        // Video-only (no audio stream)
        (true, false) => {
            let fmt = if let Some(h) = adv.max_resolution {
                format!("bestvideo[height<={}]", h)
            } else {
                "bestvideo".to_string()
            };
            let pp = "ffmpeg:-c:v libx264 -preset fast -crf 18 -an".to_string();
            (fmt, opts.format.clone(), pp, false)
        }
        // Both video + audio (normal, possibly resolution-limited)
        (true, true) | (false, false) => {
            let fmt = if let Some(h) = adv.max_resolution {
                format!(
                    "bestvideo[vcodec^=avc1][height<={}]+bestaudio[ext=m4a]\
                     /bestvideo[vcodec^=avc1][height<={}]+bestaudio\
                     /bestvideo[height<={}]+bestaudio\
                     /best[height<={}]",
                    h, h, h, h
                )
            } else {
                default_format.to_string()
            };
            let pp = format!(
                "ffmpeg:-c:v libx264 -preset fast -crf 18 -c:a aac -b:a {}",
                audio_bitrate
            );
            (fmt, opts.format.clone(), pp, false)
        }
    }
}

/// Run a yt-dlp download.
///
/// - `cancelled`: set to `true` from another thread to request cancellation.
/// - `on_progress`: called for each parsed output line.
pub fn download(
    opts: &DownloadOptions,
    config: &Config,
    cancelled: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<(), AppError> {
    let yt_dlp = resolve_yt_dlp(config)?;
    let ffmpeg_dir = resolve_ffmpeg_dir(config)?;

    // Validate output directory
    if !opts.output_dir.exists() {
        return Err(AppError::InvalidDirectory(
            opts.output_dir.display().to_string(),
        ));
    }

    // Build output template: <output_dir>/%(title)s.%(ext)s
    // yt-dlp handles Windows-unsafe character sanitisation in filenames.
    let output_template = opts
        .output_dir
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string();

    info!("Starting download: url={} output_dir={:?}", opts.url, opts.output_dir);
    info!("yt-dlp: {:?}", yt_dlp);
    info!("FFmpeg dir: {:?}", ffmpeg_dir);

    // Build format string and postprocessor args based on advanced options.
    let (format_str, merge_format, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    let mut cmd = Command::new(&yt_dlp);
    cmd
        // Progress on individual lines so we can parse easily
        .arg("--newline")
        // Don't just simulate; actually download
        .arg("--no-simulate")
        .arg("-f")
        .arg(&format_str);

    // Audio-only: extract audio to the chosen format
    if is_audio_only {
        cmd.arg("--extract-audio");
        if let Some(ref adv) = opts.advanced {
            if let Some(ref afmt) = adv.audio_format {
                cmd.arg("--audio-format").arg(afmt);
            }
            // Set audio quality (bitrate) via yt-dlp's own flag rather than
            // postprocessor args, which would conflict with --audio-format.
            if let Some(ref br) = adv.audio_bitrate {
                // Strip trailing 'k' — yt-dlp --audio-quality accepts e.g. "192K" or raw kbps
                cmd.arg("--audio-quality").arg(br);
            }
        }
    } else {
        // Merge to preferred container (not applicable for audio-only)
        cmd.arg("--merge-output-format").arg(&merge_format);
    }

    // Postprocessor args (codec transcoding) — skip for audio-only
    if !pp_args.is_empty() {
        cmd.arg("--postprocessor-args").arg(&pp_args);
    }

    // Trim: use yt-dlp's native --download-sections for reliable cutting.
    // Format: "*START-END" where timestamps are HH:MM:SS or seconds.
    // --force-keyframes-at-cuts ensures frame-accurate cuts.
    if let Some(ref adv) = opts.advanced {
        let ss = adv.start_time.as_deref().unwrap_or("").trim();
        let to = adv.end_time.as_deref().unwrap_or("").trim();
        if !ss.is_empty() || !to.is_empty() {
            let start = if ss.is_empty() { "0" } else { ss };
            let end = if to.is_empty() { "inf" } else { to };
            cmd.arg("--download-sections").arg(format!("*{}-{}", start, end));
            cmd.arg("--force-keyframes-at-cuts");
        }
    }

    cmd
        // Tell yt-dlp where to find FFmpeg
        .arg("--ffmpeg-location")
        .arg(&ffmpeg_dir)
        // Output template
        .arg("-o")
        .arg(&output_template)
        // Don't overwrite existing files — yt-dlp adds (n) suffix automatically
        .arg("--no-overwrites")
        // Restrict filenames to ASCII-safe characters (extra safety on Windows)
        .arg("--windows-filenames");

    // Cookie injection: cookies_file takes priority over cookies_from_browser.
    // cookies_file = Netscape .txt exported from browser extension (works with Chrome 127+ App-Bound Encryption).
    // cookies_from_browser = auto-extract (blocked on Chrome 127+ without elevation).
    if !config.cookies_file.is_empty() {
        cmd.arg("--cookies").arg(&config.cookies_file);
        info!("Using cookies file: {}", config.cookies_file);
    } else if !config.cookies_from_browser.is_empty() {
        cmd.arg("--cookies-from-browser").arg(&config.cookies_from_browser);
        info!("Using cookies from browser: {}", config.cookies_from_browser);
    }

    cmd
        // The URL — passed as a literal argument, NOT shell-interpolated
        .arg(&opts.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    debug!("yt-dlp command: {:?}", cmd);

    let mut child: Child = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::ProcessSpawnError {
        binary: yt_dlp.display().to_string(),
        source: e,
    })?;

    // Read stdout in the current thread; stderr in a spawned thread.
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    // Spawn stderr reader thread
    let cancelled_clone = cancelled.clone();
    let stderr_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            if cancelled_clone.load(Ordering::Relaxed) {
                break;
            }
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let l = String::from_utf8_lossy(&buf);
                    let l = l.trim_end_matches(&['\r', '\n'][..]);
                    let ev = parse_line(l);
                    debug!("stderr: {l}");
                    let _ = ev; // stderr events reported to log only
                }
                Err(e) => {
                    warn!("stderr read error: {e}");
                    break;
                }
            }
        }
    });

    // Read stdout in current thread, calling on_progress for each line
    let mut stdout_reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            info!("Cancellation requested; killing yt-dlp");
            let _ = child.kill();
            let _ = stderr_thread.join();
            return Err(AppError::Cancelled);
        }
        buf.clear();
        match stdout_reader.read_until(b'\n', &mut buf) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let l = String::from_utf8_lossy(&buf);
                let l = l.trim_end_matches(&['\r', '\n'][..]);
                let ev = parse_line(l);
                on_progress(ev);
            }
            Err(e) => {
                warn!("stdout read error: {e}");
                break;
            }
        }
    }

    let _ = stderr_thread.join();

    let status = child.wait()?;
    let code = status.code().unwrap_or(-1);

    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    if !status.success() {
        error!("yt-dlp exited with code {code}");
        return Err(AppError::YtDlpExitCode(code));
    }

    info!("yt-dlp completed successfully (exit 0)");
    Ok(())
}

// ── gallery-dl resolution ─────────────────────────────────────────────────────

/// Resolves the path to gallery-dl.exe.
///
/// Resolution order:
/// 1. Explicit `config.gallery_dl_path` (if non-empty and the file exists).
/// 2. `<exe_dir>/bin/gallery-dl.exe` (bundled).
/// 3. `gallery-dl` on the system PATH.
pub fn resolve_gallery_dl(config: &Config) -> Result<PathBuf, AppError> {
    // 1. Config override
    if !config.gallery_dl_path.is_empty() {
        let p = PathBuf::from(&config.gallery_dl_path);
        if p.exists() {
            info!("gallery-dl from config: {:?}", p);
            return Ok(p);
        }
        warn!("Configured gallery_dl_path {:?} not found; falling back", p);
    }

    // 2. Bundled
    if let Some(dir) = exe_dir() {
        let bundled = dir.join("bin").join("gallery-dl.exe");
        if bundled.exists() {
            info!("gallery-dl bundled: {:?}", bundled);
            return Ok(bundled);
        }
    }

    // 3. PATH
    if which_in_path("gallery-dl.exe").is_some() || which_in_path("gallery-dl").is_some() {
        info!("gallery-dl found on PATH");
        return Ok(PathBuf::from("gallery-dl"));
    }

    Err(AppError::MissingGalleryDl)
}

// ── Image download execution ──────────────────────────────────────────────────

/// Options for a gallery-dl image download.
#[derive(Debug, Clone)]
pub struct ImageDownloadOptions {
    /// The URL (already validated).
    pub url: String,
    /// Directory to save images into.
    pub output_dir: PathBuf,
}

/// Run a gallery-dl download.
///
/// - `cancelled`: set to `true` from another thread to request cancellation.
/// - `on_progress`: called for each output line.
pub fn download_images(
    opts: &ImageDownloadOptions,
    config: &Config,
    cancelled: Arc<AtomicBool>,
    on_progress: ProgressCallback,
) -> Result<(), AppError> {
    let gallery_dl = resolve_gallery_dl(config)?;

    if !opts.output_dir.exists() {
        return Err(AppError::InvalidDirectory(
            opts.output_dir.display().to_string(),
        ));
    }

    info!("Starting image download: url={} output_dir={:?}", opts.url, opts.output_dir);
    info!("gallery-dl: {:?}", gallery_dl);

    let mut cmd = Command::new(&gallery_dl);
    cmd
        // Verbose output so we can stream progress lines
        .arg("--verbose")
        // --directory (-D) = exact destination, no subdirectories created
        .arg("--directory")
        .arg(&opts.output_dir);

    // Cookie injection: file takes priority over browser (same logic as yt-dlp)
    if !config.cookies_file.is_empty() {
        cmd.arg("--cookies").arg(&config.cookies_file);
        info!("Using cookies file: {}", config.cookies_file);
    } else if !config.cookies_from_browser.is_empty() {
        cmd.arg("--cookies-from-browser").arg(&config.cookies_from_browser);
        info!("Using cookies from browser: {}", config.cookies_from_browser);
    }

    cmd
        // URL last — passed as literal argument, NOT shell-interpolated
        .arg(&opts.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    debug!("gallery-dl command: {:?}", cmd);

    let mut child: Child = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::ProcessSpawnError {
        binary: gallery_dl.display().to_string(),
        source: e,
    })?;

    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    // Stream stderr (errors/warnings) in a background thread — forward to on_progress
    let cancelled_clone = cancelled.clone();
    let stderr_thread = std::thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            if cancelled_clone.load(Ordering::Relaxed) {
                break;
            }
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => break,
                Ok(_) => {
                    let l = String::from_utf8_lossy(&buf);
                    let l = l.trim_end_matches(&['\r', '\n'][..]);
                    debug!("gallery-dl stderr: {l}");
                }
                Err(e) => {
                    warn!("gallery-dl stderr read error: {e}");
                    break;
                }
            }
        }
    });

    // Stream stdout to progress callback
    let mut stdout_reader = BufReader::new(stdout);
    let mut buf = Vec::new();
    loop {
        if cancelled.load(Ordering::Relaxed) {
            info!("Cancellation requested; killing gallery-dl");
            let _ = child.kill();
            let _ = stderr_thread.join();
            return Err(AppError::Cancelled);
        }
        buf.clear();
        match stdout_reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let l = String::from_utf8_lossy(&buf);
                let l = l.trim_end_matches(&['\r', '\n'][..]);
                on_progress(ProgressEvent::Other(l.to_string()));
            }
            Err(e) => {
                warn!("gallery-dl stdout read error: {e}");
                break;
            }
        }
    }

    let _ = stderr_thread.join();

    let status = child.wait()?;
    let code = status.code().unwrap_or(-1);

    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    if !status.success() {
        error!("gallery-dl exited with code {code}");
        return Err(AppError::GalleryDlExitCode(code));
    }

    info!("gallery-dl completed successfully (exit 0)");
    Ok(())
}

// ── Diagnostics ──────────────────────────────────────────────────────────────

/// Print diagnostic information about binary resolution to stdout.
pub fn run_diagnostics(config: &Config) {
    println!("=== Paste Link Downloader — Diagnostics ===\n");

    match resolve_yt_dlp(config) {
        Ok(p) => println!("✓ yt-dlp       : {}", p.display()),
        Err(e) => println!("✗ yt-dlp       : {e}"),
    }

    match resolve_ffmpeg_dir(config) {
        Ok(d) => println!("✓ FFmpeg       : {}/ffmpeg.exe", d.display()),
        Err(e) => println!("✗ FFmpeg       : {e}"),
    }

    match resolve_gallery_dl(config) {
        Ok(p) => println!("✓ gallery-dl   : {}", p.display()),
        Err(e) => println!("✗ gallery-dl   : {e}"),
    }

    if let Some(log_dir) = crate::logging::log_dir() {
        println!("  Log dir      : {}", log_dir.display());
    }

    if let Some(cfg_path) = Config::config_path() {
        println!("  Config       : {}", cfg_path.display());
    }

    println!();
}
