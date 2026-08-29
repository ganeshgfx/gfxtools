// src/postprocess.rs
// FFmpeg-based post-processing operations for existing video files.
//
// Two operations:
//   1. Convert to Compatible — re-encode to H.264+AAC MP4 for Adobe Premiere Pro
//   2. Compress — re-encode to HEVC+AAC MP4 for efficient storage
//
// Both operations use NVENC (NVIDIA GPU) for encoding when available,
// with CPU fallback (libx264/libx265). FFmpeg naturally decodes on CPU
// while encoding on GPU, saturating both simultaneously.

use crate::config::Config;
use crate::downloader::resolve_ffmpeg_dir;
use crate::error::AppError;

use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Win32 CREATE_NO_WINDOW flag.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Video file extensions we process.
const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "webm", "avi", "mov", "ts", "flv", "m4v", "wmv"];

/// Result of processing a single file.
#[derive(Debug, Clone)]
pub enum FileResult {
    Success,
    Skipped(String),
    Failed(String),
}

/// Progress callback for post-processing. Called with (current_file_index, total_files, file_name, progress_pct).
/// `progress_pct` is 0.0–100.0 for the current file; negative means indeterminate.
pub type PostprocessCallback = Box<dyn Fn(usize, usize, &str, f64) + Send + 'static>;

/// Which operation to perform.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostprocessOp {
    ConvertCompatible,
    Compress,
}

impl std::fmt::Display for PostprocessOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostprocessOp::ConvertCompatible => write!(f, "Convert to Compatible"),
            PostprocessOp::Compress => write!(f, "Compress"),
        }
    }
}

/// Detect whether FFmpeg supports a given encoder (e.g. "h264_nvenc", "hevc_nvenc").
fn has_encoder(ffmpeg: &Path, encoder: &str) -> bool {
    let result = Command::new(ffmpeg)
        .args(["-encoders", "-hide_banner"])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.contains(encoder)
        }
        Err(e) => {
            warn!("Could not query FFmpeg encoders: {e}");
            false
        }
    }
}

/// Find all video files in a directory (non-recursive, sorted alphabetically).
pub fn find_video_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if VIDEO_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                        files.push(path);
                    }
                }
            }
        }
    }
    files.sort();
    files
}

/// Get the video duration in seconds by probing with FFprobe/FFmpeg.
fn get_duration_secs(ffmpeg_dir: &Path, file: &Path) -> Option<f64> {
    // Try ffprobe first
    let ffprobe = ffmpeg_dir.join("ffprobe.exe");
    let probe_cmd = if ffprobe.exists() {
        ffprobe
    } else {
        ffmpeg_dir.join("ffmpeg.exe")
    };

    // Use ffprobe to get duration
    if probe_cmd.file_name().map(|n| n == "ffprobe.exe").unwrap_or(false) {
        let output = Command::new(&probe_cmd)
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=noprint_wrappers=1:nokey=1",
            ])
            .arg(file)
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            .ok()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        return stdout.trim().parse::<f64>().ok();
    }

    None
}

/// Parse FFmpeg progress output from stderr to extract time in seconds.
/// FFmpeg outputs lines like: `frame= 1234 fps= 30 ... time=00:01:23.45 ...`
fn parse_ffmpeg_time(line: &str) -> Option<f64> {
    // Look for time=HH:MM:SS.mm pattern
    if let Some(idx) = line.find("time=") {
        let rest = &line[idx + 5..];
        let time_str = rest.split_whitespace().next().unwrap_or("");
        // Parse HH:MM:SS.mm or negative (N/A)
        if time_str.starts_with('-') || time_str == "N/A" {
            return None;
        }
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() == 3 {
            let hours: f64 = parts[0].parse().ok()?;
            let minutes: f64 = parts[1].parse().ok()?;
            let seconds: f64 = parts[2].parse().ok()?;
            return Some(hours * 3600.0 + minutes * 60.0 + seconds);
        }
    }
    None
}

/// Run a post-processing operation on all video files in a directory.
///
/// - `op`: which operation (ConvertCompatible or Compress)
/// - `directory`: folder containing video files
/// - `config`: app config (for FFmpeg resolution)
/// - `cancelled`: cooperative cancellation flag
/// - `on_progress`: callback for progress updates
pub fn run_postprocess(
    op: PostprocessOp,
    directory: &Path,
    config: &Config,
    cancelled: Arc<AtomicBool>,
    on_progress: PostprocessCallback,
) -> Result<Vec<(PathBuf, FileResult)>, AppError> {
    let files = find_video_files(directory);
    run_postprocess_files(op, &files, directory, config, cancelled, on_progress)
}

/// Run a post-processing operation on an explicit list of video files.
///
/// Like `run_postprocess`, but takes a pre-built file list instead of scanning
/// a directory. `directory` is used only for determining the output path.
pub fn run_postprocess_files(
    op: PostprocessOp,
    files: &[PathBuf],
    directory: &Path,
    config: &Config,
    cancelled: Arc<AtomicBool>,
    on_progress: PostprocessCallback,
) -> Result<Vec<(PathBuf, FileResult)>, AppError> {
    let ffmpeg_dir = resolve_ffmpeg_dir(config)?;
    let ffmpeg = ffmpeg_dir.join("ffmpeg.exe");

    if !ffmpeg.exists() {
        return Err(AppError::MissingFfmpeg);
    }

    // Detect GPU encoder availability
    let (nvenc_encoder, cpu_encoder) = match op {
        PostprocessOp::ConvertCompatible => ("h264_nvenc", "libx264"),
        PostprocessOp::Compress => ("hevc_nvenc", "libx265"),
    };

    let use_nvenc = has_encoder(&ffmpeg, nvenc_encoder);
    if use_nvenc {
        info!("{} encoder available — using GPU acceleration", nvenc_encoder);
    } else {
        warn!("{} not available — falling back to CPU encoder {}", nvenc_encoder, cpu_encoder);
    }

    let _encoder = if use_nvenc { nvenc_encoder } else { cpu_encoder };

    if files.is_empty() {
        info!("No video files to process");
        return Ok(Vec::new());
    }

    info!("Found {} video files for {:?}", files.len(), op);

    // For Compress, create the `small/` subfolder
    let output_dir = match op {
        PostprocessOp::Compress => {
            let small_dir = directory.join("small");
            std::fs::create_dir_all(&small_dir)?;
            small_dir
        }
        PostprocessOp::ConvertCompatible => directory.to_path_buf(),
    };

    let total = files.len();
    let mut results: Vec<(PathBuf, FileResult)> = Vec::with_capacity(total);

    for (i, file) in files.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            info!("Post-processing cancelled by user");
            return Err(AppError::Cancelled);
        }

        let file_name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
        let stem = file.file_stem().unwrap_or_default().to_string_lossy().to_string();

        // Skip files already processed
        match op {
            PostprocessOp::ConvertCompatible => {
                if stem.ends_with("_edit") {
                    info!("Skipping already-converted file: {}", file_name);
                    on_progress(i, total, &file_name, -1.0);
                    results.push((file.clone(), FileResult::Skipped("Already converted (_edit suffix)".to_string())));
                    continue;
                }
            }
            PostprocessOp::Compress => {
                // Skip if output already exists in small/
                let out_path = output_dir.join(&file_name);
                if out_path.exists() {
                    info!("Skipping already-compressed file: {}", file_name);
                    on_progress(i, total, &file_name, -1.0);
                    results.push((file.clone(), FileResult::Skipped("Already exists in small/".to_string())));
                    continue;
                }
            }
        }

        on_progress(i, total, &file_name, 0.0);

        // Get duration for progress calculation
        let duration = get_duration_secs(&ffmpeg_dir, file);

        // Build output path
        let output_path = match op {
            PostprocessOp::ConvertCompatible => {
                directory.join(format!("{}_edit.mp4", stem))
            }
            PostprocessOp::Compress => {
                // Keep original extension if mp4, otherwise use mp4
                let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                let out_ext = if ext.eq_ignore_ascii_case("mp4") { "mp4" } else { "mp4" };
                output_dir.join(format!("{}.{}", stem, out_ext))
            }
        };

        info!("Processing [{}/{}]: {} → {:?}", i + 1, total, file_name, output_path);

        // Build FFmpeg command
        let mut cmd = Command::new(&ffmpeg);
        cmd.arg("-y") // overwrite output
            .arg("-hide_banner")
            .arg("-i").arg(file)
            .arg("-threads").arg("0"); // auto-detect CPU threads for decode

        match op {
            PostprocessOp::ConvertCompatible => {
                if use_nvenc {
                    // NVENC H.264 — Premiere Pro compatible
                    cmd.args(["-c:v", "h264_nvenc"])
                        .args(["-preset", "p4"])       // balanced quality/speed
                        .args(["-rc", "vbr"])           // variable bitrate
                        .args(["-cq", "18"])            // quality level (visually lossless)
                        .args(["-b:v", "0"]);           // let CQ control quality
                } else {
                    // CPU fallback — libx264
                    cmd.args(["-c:v", "libx264"])
                        .args(["-preset", "medium"])
                        .args(["-crf", "18"]);
                }
                // Audio: AAC for Premiere compatibility
                cmd.args(["-c:a", "aac"])
                    .args(["-b:a", "192k"]);
                // Pixel format for maximum NLE compatibility
                cmd.args(["-pix_fmt", "yuv420p"]);
                // Faststart for streaming/NLE scrubbing
                cmd.args(["-movflags", "+faststart"]);
            }
            PostprocessOp::Compress => {
                if use_nvenc {
                    // NVENC HEVC — efficient compression
                    cmd.args(["-c:v", "hevc_nvenc"])
                        .args(["-preset", "p5"])       // higher quality preset
                        .args(["-rc", "vbr"])           // variable bitrate
                        .args(["-cq", "26"])            // good compression, reasonable quality
                        .args(["-b:v", "0"]);           // let CQ control
                } else {
                    // CPU fallback — libx265
                    cmd.args(["-c:v", "libx265"])
                        .args(["-preset", "medium"])
                        .args(["-crf", "26"]);
                }
                // Audio: AAC at slightly lower bitrate for storage
                cmd.args(["-c:a", "aac"])
                    .args(["-b:a", "128k"]);
                // Faststart
                cmd.args(["-movflags", "+faststart"]);
            }
        }

        // Progress output: FFmpeg writes progress to stderr with `-progress pipe:1`
        // But we can parse time= from stderr for more reliable progress
        cmd.arg(&output_path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        debug!("FFmpeg command: {:?}", cmd);

        let mut child = cmd
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| AppError::ProcessSpawnError {
                binary: ffmpeg.display().to_string(),
                source: e,
            })?;

        // Read stderr for progress
        let stderr = child.stderr.take().expect("stderr piped");
        let cancelled_clone = cancelled.clone();
        let duration_clone = duration;
        let file_name_clone = file_name.clone();
        let file_idx = i;
        let file_total = total;

        // We need to handle the progress callback in a way that works with the borrow checker
        // Parse stderr in current thread since we're processing files sequentially
        let mut reader = BufReader::new(stderr);
        let mut buf = Vec::new();
        loop {
            if cancelled_clone.load(Ordering::Relaxed) {
                info!("Cancellation requested; killing FFmpeg");
                let _ = child.kill();
                return Err(AppError::Cancelled);
            }
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => {
                    // EOF — also check for \r-delimited lines (FFmpeg uses \r for progress)
                    break;
                }
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    let line = line.trim_end_matches(&['\r', '\n'][..]);

                    if let Some(time_secs) = parse_ffmpeg_time(line) {
                        if let Some(dur) = duration_clone {
                            if dur > 0.0 {
                                let pct = (time_secs / dur * 100.0).min(100.0);
                                on_progress(file_idx, file_total, &file_name_clone, pct);
                            }
                        }
                    }
                    debug!("ffmpeg: {}", line);
                }
                Err(e) => {
                    warn!("FFmpeg stderr read error: {e}");
                    break;
                }
            }
        }

        // Also read any \r-delimited progress that BufReader missed
        // (FFmpeg uses \r without \n for progress updates)
        // The BufReader above handles \n-delimited lines; for \r we need the wait below.

        let status = child.wait()?;

        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        if status.success() {
            info!("Successfully processed: {}", file_name);
            on_progress(i, total, &file_name, 100.0);
            results.push((file.clone(), FileResult::Success));
        } else {
            let code = status.code().unwrap_or(-1);
            error!("FFmpeg failed for {} with exit code {}", file_name, code);
            results.push((file.clone(), FileResult::Failed(format!("FFmpeg exit code {}", code))));
        }
    }

    Ok(results)
}

/// Run a post-processing operation on a single video file.
///
/// Same logic as `run_postprocess` but for one file only.
/// The file's parent directory is used as the working directory.
pub fn run_postprocess_single(
    op: PostprocessOp,
    file: &Path,
    config: &Config,
    cancelled: Arc<AtomicBool>,
    on_progress: PostprocessCallback,
) -> Result<Vec<(PathBuf, FileResult)>, AppError> {
    let directory = file.parent().ok_or_else(|| {
        AppError::InvalidDirectory("Cannot determine parent directory of file".to_string())
    })?;

    let ffmpeg_dir = resolve_ffmpeg_dir(config)?;
    let ffmpeg = ffmpeg_dir.join("ffmpeg.exe");

    if !ffmpeg.exists() {
        return Err(AppError::MissingFfmpeg);
    }

    // Detect GPU encoder availability
    let (nvenc_encoder, cpu_encoder) = match op {
        PostprocessOp::ConvertCompatible => ("h264_nvenc", "libx264"),
        PostprocessOp::Compress => ("hevc_nvenc", "libx265"),
    };

    let use_nvenc = has_encoder(&ffmpeg, nvenc_encoder);
    if use_nvenc {
        info!("{} encoder available — using GPU acceleration", nvenc_encoder);
    } else {
        warn!("{} not available — falling back to CPU encoder {}", nvenc_encoder, cpu_encoder);
    }

    // For Compress, create the `small/` subfolder
    let output_dir = match op {
        PostprocessOp::Compress => {
            let small_dir = directory.join("small");
            std::fs::create_dir_all(&small_dir)?;
            small_dir
        }
        PostprocessOp::ConvertCompatible => directory.to_path_buf(),
    };

    let file_name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
    let stem = file.file_stem().unwrap_or_default().to_string_lossy().to_string();

    on_progress(0, 1, &file_name, 0.0);

    // Get duration for progress
    let duration = get_duration_secs(&ffmpeg_dir, file);

    // Build output path
    let output_path = match op {
        PostprocessOp::ConvertCompatible => {
            directory.join(format!("{}_edit.mp4", stem))
        }
        PostprocessOp::Compress => {
            output_dir.join(format!("{}.mp4", stem))
        }
    };

    info!("Processing single file: {} → {:?}", file_name, output_path);

    // Build FFmpeg command
    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y")
        .arg("-hide_banner")
        .arg("-i").arg(file)
        .arg("-threads").arg("0");

    match op {
        PostprocessOp::ConvertCompatible => {
            if use_nvenc {
                cmd.args(["-c:v", "h264_nvenc"])
                    .args(["-preset", "p4"])
                    .args(["-rc", "vbr"])
                    .args(["-cq", "18"])
                    .args(["-b:v", "0"]);
            } else {
                cmd.args(["-c:v", "libx264"])
                    .args(["-preset", "medium"])
                    .args(["-crf", "18"]);
            }
            cmd.args(["-c:a", "aac"])
                .args(["-b:a", "192k"])
                .args(["-pix_fmt", "yuv420p"])
                .args(["-movflags", "+faststart"]);
        }
        PostprocessOp::Compress => {
            if use_nvenc {
                cmd.args(["-c:v", "hevc_nvenc"])
                    .args(["-preset", "p5"])
                    .args(["-rc", "vbr"])
                    .args(["-cq", "26"])
                    .args(["-b:v", "0"]);
            } else {
                cmd.args(["-c:v", "libx265"])
                    .args(["-preset", "medium"])
                    .args(["-crf", "26"]);
            }
            cmd.args(["-c:a", "aac"])
                .args(["-b:a", "128k"])
                .args(["-movflags", "+faststart"]);
        }
    }

    cmd.arg(&output_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    debug!("FFmpeg command: {:?}", cmd);

    let mut child = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::ProcessSpawnError {
            binary: ffmpeg.display().to_string(),
            source: e,
        })?;

    let stderr = child.stderr.take().expect("stderr piped");
    let mut reader = BufReader::new(stderr);
    let mut buf = Vec::new();

    loop {
        if cancelled.load(Ordering::Relaxed) {
            info!("Cancellation requested; killing FFmpeg");
            let _ = child.kill();
            return Err(AppError::Cancelled);
        }
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf);
                let line = line.trim_end_matches(&['\r', '\n'][..]);
                if let Some(time_secs) = parse_ffmpeg_time(line) {
                    if let Some(dur) = duration {
                        if dur > 0.0 {
                            let pct = (time_secs / dur * 100.0).min(100.0);
                            on_progress(0, 1, &file_name, pct);
                        }
                    }
                }
                debug!("ffmpeg: {}", line);
            }
            Err(e) => {
                warn!("FFmpeg stderr read error: {e}");
                break;
            }
        }
    }

    let status = child.wait()?;

    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }

    let result = if status.success() {
        info!("Successfully processed: {}", file_name);
        on_progress(0, 1, &file_name, 100.0);
        FileResult::Success
    } else {
        let code = status.code().unwrap_or(-1);
        error!("FFmpeg failed for {} with exit code {}", file_name, code);
        FileResult::Failed(format!("FFmpeg exit code {}", code))
    };

    Ok(vec![(file.to_path_buf(), result)])
}
