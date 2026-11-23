// tests/download_scenarios_test.rs
// Comprehensive test suite for all download scenarios:
// - Command construction across all option permutations (audio, video, resolution, trim, cookies)
// - Validation & error paths before process spawn
// - Real process execution with mock-yt-dlp (success, warning, error, cancellation)

use gfx_tools::config::Config;
use gfx_tools::downloader::{
    build_gallery_dl_command, build_yt_dlp_command, download, download_images,
    AdvancedOptions, DownloadOptions, ImageDownloadOptions,
};
use gfx_tools::error::AppError;
use gfx_tools::progress::ProgressEvent;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn base_options() -> DownloadOptions {
    DownloadOptions {
        url: "https://www.youtube.com/watch?v=mock-success".to_string(),
        output_dir: PathBuf::from(r"D:\Videos"),
        format: "mp4".to_string(),
        advanced: None,
    }
}

fn args_to_strings(cmd: &std::process::Command) -> Vec<String> {
    cmd.get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
}

// ── 1. Command construction scenarios ────────────────────────────────────────

#[test]
fn scenario_default_video_and_audio() {
    let opts = base_options();
    let config = Config::default();
    let yt_dlp = Path::new("yt-dlp.exe");
    let ffmpeg_dir = Path::new(r"C:\tools\ffmpeg\bin");

    let cmd = build_yt_dlp_command(&opts, &config, yt_dlp, ffmpeg_dir);
    let args = args_to_strings(&cmd);

    assert!(args.contains(&"--newline".to_string()));
    assert!(args.contains(&"--no-simulate".to_string()));
    assert!(args.contains(&"-f".to_string()));
    assert!(args.contains(&"--merge-output-format".to_string()));
    assert!(args.contains(&"mp4".to_string()));
    assert!(args.contains(&"--ffmpeg-location".to_string()));
    assert!(args.contains(&ffmpeg_dir.to_string_lossy().to_string()));
    assert!(args.contains(&opts.url));
}

#[test]
fn scenario_audio_only_all_formats() {
    let formats = ["mp3", "m4a", "opus", "flac", "wav"];
    let config = Config::default();

    for afmt in formats {
        let mut opts = base_options();
        opts.advanced = Some(AdvancedOptions {
            video: false,
            audio: true,
            audio_format: Some(afmt.to_string()),
            max_resolution: None,
            audio_bitrate: Some("320k".to_string()),
            start_time: None,
            end_time: None,
        });

        let cmd = build_yt_dlp_command(
            &opts,
            &config,
            Path::new("yt-dlp.exe"),
            Path::new(r"C:\ffmpeg"),
        );
        let args = args_to_strings(&cmd);

        assert!(args.contains(&"--extract-audio".to_string()));
        assert!(args.contains(&"--audio-format".to_string()));
        assert!(args.contains(&afmt.to_string()));
        assert!(args.contains(&"--audio-quality".to_string()));
        assert!(args.contains(&"320k".to_string()));
        // Audio-only must NOT specify --merge-output-format
        assert!(!args.contains(&"--merge-output-format".to_string()));
    }
}

#[test]
fn scenario_video_only_without_audio() {
    let mut opts = base_options();
    opts.advanced = Some(AdvancedOptions {
        video: true,
        audio: false,
        audio_format: None,
        max_resolution: None,
        audio_bitrate: None,
        start_time: None,
        end_time: None,
    });
    let config = Config::default();

    let cmd = build_yt_dlp_command(
        &opts,
        &config,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args = args_to_strings(&cmd);

    assert!(args.contains(&"--postprocessor-args".to_string()));
    let pp_idx = args
        .iter()
        .position(|a| a == "--postprocessor-args")
        .unwrap();
    assert!(args[pp_idx + 1].contains("-an"));
}

#[test]
fn scenario_resolution_caps() {
    let resolutions = [480, 720, 1080, 1440, 2160];
    let config = Config::default();

    for res in resolutions {
        let mut opts = base_options();
        opts.advanced = Some(AdvancedOptions {
            video: true,
            audio: true,
            audio_format: None,
            max_resolution: Some(res),
            audio_bitrate: None,
            start_time: None,
            end_time: None,
        });

        let cmd = build_yt_dlp_command(
            &opts,
            &config,
            Path::new("yt-dlp.exe"),
            Path::new(r"C:\ffmpeg"),
        );
        let args = args_to_strings(&cmd);
        let f_idx = args.iter().position(|a| a == "-f").unwrap();
        assert!(args[f_idx + 1].contains(&format!("height<={}", res)));
    }
}

#[test]
fn scenario_trim_permutations() {
    let config = Config::default();

    // Start only
    let mut opts1 = base_options();
    opts1.advanced = Some(AdvancedOptions {
        video: true,
        audio: true,
        audio_format: None,
        max_resolution: None,
        audio_bitrate: None,
        start_time: Some("00:01:30".to_string()),
        end_time: None,
    });
    let cmd1 = build_yt_dlp_command(
        &opts1,
        &config,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args1 = args_to_strings(&cmd1);
    assert!(args1.contains(&"--download-sections".to_string()));
    assert!(args1.contains(&"*00:01:30-inf".to_string()));
    assert!(args1.contains(&"--force-keyframes-at-cuts".to_string()));

    // End only
    let mut opts2 = base_options();
    opts2.advanced = Some(AdvancedOptions {
        video: true,
        audio: true,
        audio_format: None,
        max_resolution: None,
        audio_bitrate: None,
        start_time: None,
        end_time: Some("00:05:00".to_string()),
    });
    let cmd2 = build_yt_dlp_command(
        &opts2,
        &config,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args2 = args_to_strings(&cmd2);
    assert!(args2.contains(&"*0-00:05:00".to_string()));

    // Both start and end
    let mut opts3 = base_options();
    opts3.advanced = Some(AdvancedOptions {
        video: true,
        audio: true,
        audio_format: None,
        max_resolution: None,
        audio_bitrate: None,
        start_time: Some("10".to_string()),
        end_time: Some("45".to_string()),
    });
    let cmd3 = build_yt_dlp_command(
        &opts3,
        &config,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args3 = args_to_strings(&cmd3);
    assert!(args3.contains(&"*10-45".to_string()));
}

#[test]
fn scenario_cookie_rules_youtube_vs_other() {
    let mut cfg = Config::default();
    cfg.cookies_from_browser = "chrome".to_string();

    // YouTube URL: cookies_from_browser MUST be skipped (avoid SABR lockout)
    let opts_yt = base_options();
    let cmd_yt = build_yt_dlp_command(
        &opts_yt,
        &cfg,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args_yt = args_to_strings(&cmd_yt);
    assert!(!args_yt.contains(&"--cookies-from-browser".to_string()));

    // Non-YouTube URL (Instagram): cookies_from_browser MUST be passed
    let mut opts_ig = base_options();
    opts_ig.url = "https://www.instagram.com/p/C-example/".to_string();
    let cmd_ig = build_yt_dlp_command(
        &opts_ig,
        &cfg,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args_ig = args_to_strings(&cmd_ig);
    assert!(args_ig.contains(&"--cookies-from-browser".to_string()));
    assert!(args_ig.contains(&"chrome".to_string()));

    // Cookies file takes precedence over browser cookies even on YouTube
    cfg.cookies_file = r"C:\path\cookies.txt".to_string();
    let cmd_cookie_file = build_yt_dlp_command(
        &opts_yt,
        &cfg,
        Path::new("yt-dlp.exe"),
        Path::new(r"C:\ffmpeg"),
    );
    let args_cookie_file = args_to_strings(&cmd_cookie_file);
    assert!(args_cookie_file.contains(&"--cookies".to_string()));
    assert!(args_cookie_file.contains(&r"C:\path\cookies.txt".to_string()));
    assert!(!args_cookie_file.contains(&"--cookies-from-browser".to_string()));
}

#[test]
fn scenario_gallery_dl_command_construction() {
    let mut cfg = Config::default();
    cfg.cookies_file = r"C:\cookies.txt".to_string();

    let opts = ImageDownloadOptions {
        url: "https://www.pinterest.com/pin/12345/".to_string(),
        output_dir: PathBuf::from(r"D:\Pictures"),
    };

    let cmd = build_gallery_dl_command(&opts, &cfg, Path::new("gallery-dl.exe"));
    let args = args_to_strings(&cmd);

    assert!(args.contains(&"--verbose".to_string()));
    assert!(args.contains(&"--directory".to_string()));
    assert!(args.contains(&r"D:\Pictures".to_string()));
    assert!(args.contains(&"--cookies".to_string()));
    assert!(args.contains(&r"C:\cookies.txt".to_string()));
    assert!(args.contains(&opts.url));
}

// ── 2. Error & validation scenarios ──────────────────────────────────────────

#[test]
fn scenario_invalid_output_directory_fails_immediately() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock-yt-dlp"));
    let temp_dir = std::env::temp_dir().join(format!("gfx_mock_dummy_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    File::create(temp_dir.join("ffmpeg.exe")).unwrap();

    let mut opts = base_options();
    opts.output_dir = PathBuf::from(r"Z:\nonexistent_drive_987654321");

    let mut cfg = Config::default();
    cfg.yt_dlp_path = mock_bin.to_string_lossy().to_string();
    cfg.ffmpeg_dir = temp_dir.to_string_lossy().to_string();

    let cancelled = Arc::new(AtomicBool::new(false));

    let res = download(
        &opts,
        &cfg,
        cancelled,
        Box::new(|_| {}),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(matches!(res, Err(AppError::InvalidDirectory(_))), "expected InvalidDirectory, got: {:?}", res);
}

#[test]
fn scenario_missing_yt_dlp_fails_gracefully() {
    let temp_dir = std::env::temp_dir().join(format!("gfx_test_dl_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let mut opts = base_options();
    opts.output_dir = temp_dir.clone();

    let mut cfg = Config::default();
    cfg.yt_dlp_path = r"Z:\nonexistent\yt-dlp.exe".to_string();

    let cancelled = Arc::new(AtomicBool::new(false));

    let res = download(
        &opts,
        &cfg,
        cancelled,
        Box::new(|_| {}),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
    assert!(res.is_err());
}

// ── 3. Real process execution with mock-yt-dlp ────────────────────────────────

#[test]
fn scenario_real_download_success_progress_events() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock-yt-dlp"));
    assert!(mock_bin.exists(), "mock-yt-dlp binary must exist");

    let temp_dir = std::env::temp_dir().join(format!("gfx_mock_test_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    // Create dummy ffmpeg.exe in temp_dir so resolve_ffmpeg_dir succeeds
    File::create(temp_dir.join("ffmpeg.exe")).unwrap();

    let mut opts = base_options();
    opts.url = "https://www.youtube.com/watch?v=mock-success".to_string();
    opts.output_dir = temp_dir.clone();

    let mut cfg = Config::default();
    cfg.yt_dlp_path = mock_bin.to_string_lossy().to_string();
    cfg.ffmpeg_dir = temp_dir.to_string_lossy().to_string();

    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let events_cb = events.clone();

    let cancelled = Arc::new(AtomicBool::new(false));

    let res = download(
        &opts,
        &cfg,
        cancelled,
        Box::new(move |ev| {
            let mut list = events_cb.lock().unwrap();
            match ev {
                ProgressEvent::Percent(p) => list.push(format!("pct:{p}")),
                ProgressEvent::Speed(s) => list.push(format!("speed:{s}")),
                ProgressEvent::Merging(m) => list.push(format!("merge:{m}")),
                ProgressEvent::Complete => list.push("complete".to_string()),
                ProgressEvent::Warning(w) => list.push(format!("warn:{w}")),
                ProgressEvent::Error(e) => list.push(format!("err:{e}")),
                ProgressEvent::Other(o) => list.push(format!("other:{o}")),
                ProgressEvent::Eta(eta) => list.push(format!("eta:{eta}")),
            }
        }),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(res.is_ok(), "download should succeed with mock binary");
    let recorded = events.lock().unwrap();
    assert!(recorded.contains(&"complete".to_string()), "must receive complete event, got: {:?}", recorded);
    assert!(recorded.iter().any(|e| e.starts_with("merge:")));
}

#[test]
fn scenario_real_download_error_exit_code() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock-yt-dlp"));

    let temp_dir = std::env::temp_dir().join(format!("gfx_mock_err_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    File::create(temp_dir.join("ffmpeg.exe")).unwrap();

    let mut opts = base_options();
    opts.url = "https://www.youtube.com/watch?v=mock-error".to_string();
    opts.output_dir = temp_dir.clone();

    let mut cfg = Config::default();
    cfg.yt_dlp_path = mock_bin.to_string_lossy().to_string();
    cfg.ffmpeg_dir = temp_dir.to_string_lossy().to_string();

    let error_received = Arc::new(AtomicBool::new(false));
    let error_flag = error_received.clone();

    let cancelled = Arc::new(AtomicBool::new(false));

    let res = download(
        &opts,
        &cfg,
        cancelled,
        Box::new(move |ev| {
            if matches!(ev, ProgressEvent::Error(_)) {
                error_flag.store(true, Ordering::SeqCst);
            }
        }),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(res.is_err(), "non-zero exit must return Err");
    assert!(error_received.load(Ordering::SeqCst), "stderr error must be forwarded to progress");
}

#[test]
fn scenario_real_download_cancellation() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock-yt-dlp"));

    let temp_dir = std::env::temp_dir().join(format!("gfx_mock_cancel_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    File::create(temp_dir.join("ffmpeg.exe")).unwrap();

    let mut opts = base_options();
    opts.url = "https://www.youtube.com/watch?v=mock-hang".to_string();
    opts.output_dir = temp_dir.clone();

    let mut cfg = Config::default();
    cfg.yt_dlp_path = mock_bin.to_string_lossy().to_string();
    cfg.ffmpeg_dir = temp_dir.to_string_lossy().to_string();

    let cancelled = Arc::new(AtomicBool::new(false));
    let cancel_trigger = cancelled.clone();

    // Trigger cancellation after 150ms while child process is sleeping
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(150));
        cancel_trigger.store(true, Ordering::SeqCst);
    });

    let res = download(
        &opts,
        &cfg,
        cancelled,
        Box::new(|_| {}),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(matches!(res, Err(AppError::Cancelled)), "must return AppError::Cancelled");
}

#[test]
fn scenario_real_download_images_success() {
    let mock_bin = PathBuf::from(env!("CARGO_BIN_EXE_mock-yt-dlp"));

    let temp_dir = std::env::temp_dir().join(format!("gfx_mock_img_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let opts = ImageDownloadOptions {
        url: "https://www.pinterest.com/pin/mock-success/".to_string(),
        output_dir: temp_dir.clone(),
    };

    let mut cfg = Config::default();
    cfg.gallery_dl_path = mock_bin.to_string_lossy().to_string();

    let cancelled = Arc::new(AtomicBool::new(false));

    let res = download_images(
        &opts,
        &cfg,
        cancelled,
        Box::new(|_| {}),
    );

    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(res.is_ok(), "download_images should succeed with mock binary");
}
