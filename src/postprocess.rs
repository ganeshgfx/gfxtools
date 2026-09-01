// src/postprocess.rs
// FFmpeg-based post-processing operations for existing video files.
//
// Two operations:
//   1. Convert to Compatible -- re-encode to H.264+AAC MP4 for Adobe Premiere Pro
//   2. Compress -- re-encode to HEVC+AAC MP4 for efficient storage
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

/// FFmpeg encoding statistics emitted via the progress callback.
/// Uses FFmpeg's `-progress pipe:1` machine-readable output format.
#[derive(Debug, Clone, Copy, Default)]
pub struct FfmpegStats {
    /// Progress percentage 0.0-100.0 for the current file; negative = indeterminate.
    pub pct: f64,
    /// Encoding speed relative to real-time (e.g. 2.5 = 2.5x faster than realtime).
    pub speed: f64,
    /// Encoder output frames per second.
    pub fps: f64,
}

/// Progress callback for post-processing.
/// Called with (current_file_index, total_files, file_name, stats).
pub type PostprocessCallback = Box<dyn Fn(usize, usize, &str, FfmpegStats) + Send + 'static>;

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

/// Get the video duration in seconds by probing with FFprobe.
fn get_duration_secs(ffmpeg_dir: &Path, file: &Path) -> Option<f64> {
    let ffprobe = ffmpeg_dir.join("ffprobe.exe");
    if !ffprobe.exists() {
        return None;
    }

    let output = Command::new(&ffprobe)
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
    stdout.trim().parse::<f64>().ok()
}

/// Parse `out_time` from FFmpeg -progress output: "HH:MM:SS.uuuuuu" -> seconds.
fn parse_out_time(value: &str) -> Option<f64> {
    // Format: HH:MM:SS.uuuuuu  (may also be N/A or negative)
    if value.starts_with('-') || value == "N/A" {
        return None;
    }
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() == 3 {
        let h: f64 = parts[0].parse().ok()?;
        let m: f64 = parts[1].parse().ok()?;
        let s: f64 = parts[2].parse().ok()?;
        Some(h * 3600.0 + m * 60.0 + s)
    } else {
        None
    }
}

/// Run a post-processing operation on all video files in a directory.
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
        info!("{} encoder available -- using GPU acceleration", nvenc_encoder);
    } else {
        warn!("{} not available -- falling back to CPU encoder {}", nvenc_encoder, cpu_encoder);
    }

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
                    on_progress(i, total, &file_name, FfmpegStats { pct: -1.0, ..Default::default() });
                    results.push((file.clone(), FileResult::Skipped("Already converted (_edit suffix)".to_string())));
                    continue;
                }
            }
            PostprocessOp::Compress => {
                let out_path = output_dir.join(&file_name);
                if out_path.exists() {
                    info!("Skipping already-compressed file: {}", file_name);
                    on_progress(i, total, &file_name, FfmpegStats { pct: -1.0, ..Default::default() });
                    results.push((file.clone(), FileResult::Skipped("Already exists in small/".to_string())));
                    continue;
                }
            }
        }

        // Signal start (0% or indeterminate if duration unknown)
        let duration = get_duration_secs(&ffmpeg_dir, file);
        let start_stats = if duration.is_some() {
            FfmpegStats::default() // pct=0.0
        } else {
            FfmpegStats { pct: -1.0, ..Default::default() }
        };
        on_progress(i, total, &file_name, start_stats);

        // Build output path
        let output_path = match op {
            PostprocessOp::ConvertCompatible => {
                directory.join(format!("{}_edit.mp4", stem))
            }
            PostprocessOp::Compress => {
                output_dir.join(format!("{}.mp4", stem))
            }
        };

        info!("Processing [{}/{}]: {} -> {:?}", i + 1, total, file_name, output_path);

        // Build FFmpeg command
        let mut cmd = Command::new(&ffmpeg);
        cmd.arg("-y")
            .arg("-hide_banner")
            .arg("-loglevel").arg("error")  // suppress informational stderr
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

        // Use FFmpeg's structured progress output on stdout (newline-delimited key=value).
        // This avoids the \r-delimited stats that BufReader::read_until(b'\n') can't parse.
        cmd.args(["-progress", "pipe:1"])
            .arg(&output_path)
            .stdout(Stdio::piped())   // capture structured progress
            .stderr(Stdio::null());   // suppress noisy stats

        debug!("FFmpeg command: {:?}", cmd);

        let mut child = cmd
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| AppError::ProcessSpawnError {
                binary: ffmpeg.display().to_string(),
                source: e,
            })?;

        // Read stdout for machine-readable progress key=value pairs
        let stdout = child.stdout.take().expect("stdout piped");
        let cancelled_clone = cancelled.clone();
        let duration_clone = duration;
        let file_name_clone = file_name.clone();
        let file_idx = i;
        let file_total = total;

        let mut reader = BufReader::new(stdout);
        let mut current_time_secs: Option<f64> = None;
        let mut current_speed: f64 = 0.0;
        let mut current_fps: f64 = 0.0;

        loop {
            if cancelled_clone.load(Ordering::Relaxed) {
                info!("Cancellation requested; killing FFmpeg");
                let _ = child.kill();
                return Err(AppError::Cancelled);
            }
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break, // EOF -- FFmpeg finished
                Ok(_) => {
                    let line = line.trim_end_matches(&['\r', '\n'][..]);
                    if let Some(eq) = line.find('=') {
                        let key = &line[..eq];
                        let value = &line[eq + 1..];
                        match key {
                            "out_time" => {
                                current_time_secs = parse_out_time(value);
                            }
                            "fps" => {
                                current_fps = value.parse().unwrap_or(0.0);
                            }
                            "speed" => {
                                // "2.50x" or "N/A"
                                let v = value.trim_end_matches('x');
                                current_speed = if v == "N/A" { 0.0 } else { v.parse().unwrap_or(0.0) };
                            }
                            "progress" => {
                                // Emit one update per FFmpeg progress block
                                let pct = match (current_time_secs, duration_clone) {
                                    (Some(t), Some(d)) if d > 0.0 => (t / d * 100.0).clamp(0.0, 99.5),
                                    _ => -1.0, // duration unknown -- indeterminate
                                };
                                on_progress(file_idx, file_total, &file_name_clone, FfmpegStats {
                                    pct,
                                    speed: current_speed,
                                    fps: current_fps,
                                });
                            }
                            _ => {}
                        }
                    }
                    debug!("ffmpeg: {}", line);
                }
                Err(e) => {
                    warn!("FFmpeg stdout read error: {e}");
                    break;
                }
            }
        }

        let status = child.wait()?;

        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }

        if status.success() {
            info!("Successfully processed: {}", file_name);
            on_progress(i, total, &file_name, FfmpegStats { pct: 100.0, speed: current_speed, fps: current_fps });
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

    let (nvenc_encoder, cpu_encoder) = match op {
        PostprocessOp::ConvertCompatible => ("h264_nvenc", "libx264"),
        PostprocessOp::Compress => ("hevc_nvenc", "libx265"),
    };

    let use_nvenc = has_encoder(&ffmpeg, nvenc_encoder);
    if use_nvenc {
        info!("{} encoder available -- using GPU acceleration", nvenc_encoder);
    } else {
        warn!("{} not available -- falling back to CPU encoder {}", nvenc_encoder, cpu_encoder);
    }

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

    let duration = get_duration_secs(&ffmpeg_dir, file);
    let start_stats = if duration.is_some() {
        FfmpegStats::default()
    } else {
        FfmpegStats { pct: -1.0, ..Default::default() }
    };
    on_progress(0, 1, &file_name, start_stats);

    let output_path = match op {
        PostprocessOp::ConvertCompatible => {
            directory.join(format!("{}_edit.mp4", stem))
        }
        PostprocessOp::Compress => {
            output_dir.join(format!("{}.mp4", stem))
        }
    };

    info!("Processing single file: {} -> {:?}", file_name, output_path);

    let mut cmd = Command::new(&ffmpeg);
    cmd.arg("-y")
        .arg("-hide_banner")
        .arg("-loglevel").arg("error")
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

    cmd.args(["-progress", "pipe:1"])
        .arg(&output_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    debug!("FFmpeg command: {:?}", cmd);

    let mut child = cmd
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| AppError::ProcessSpawnError {
            binary: ffmpeg.display().to_string(),
            source: e,
        })?;

    let stdout = child.stdout.take().expect("stdout piped");
    let mut reader = BufReader::new(stdout);
    let mut current_time_secs: Option<f64> = None;
    let mut current_speed: f64 = 0.0;
    let mut current_fps: f64 = 0.0;

    loop {
        if cancelled.load(Ordering::Relaxed) {
            info!("Cancellation requested; killing FFmpeg");
            let _ = child.kill();
            return Err(AppError::Cancelled);
        }
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let line = line.trim_end_matches(&['\r', '\n'][..]);
                if let Some(eq) = line.find('=') {
                    let key = &line[..eq];
                    let value = &line[eq + 1..];
                    match key {
                        "out_time" => {
                            current_time_secs = parse_out_time(value);
                        }
                        "fps" => {
                            current_fps = value.parse().unwrap_or(0.0);
                        }
                        "speed" => {
                            let v = value.trim_end_matches('x');
                            current_speed = if v == "N/A" { 0.0 } else { v.parse().unwrap_or(0.0) };
                        }
                        "progress" => {
                            let pct = match (current_time_secs, duration) {
                                (Some(t), Some(d)) if d > 0.0 => (t / d * 100.0).clamp(0.0, 99.5),
                                _ => -1.0,
                            };
                            on_progress(0, 1, &file_name, FfmpegStats { pct, speed: current_speed, fps: current_fps });
                        }
                        _ => {}
                    }
                }
                debug!("ffmpeg: {}", line);
            }
            Err(e) => {
                warn!("FFmpeg stdout read error: {e}");
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
        on_progress(0, 1, &file_name, FfmpegStats { pct: 100.0, speed: current_speed, fps: current_fps });
        FileResult::Success
    } else {
        let code = status.code().unwrap_or(-1);
        error!("FFmpeg failed for {} with exit code {}", file_name, code);
        FileResult::Failed(format!("FFmpeg exit code {}", code))
    };

    Ok(vec![(file.to_path_buf(), result)])
}
