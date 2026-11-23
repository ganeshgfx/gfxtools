// tests/downloader_tests.rs
// Tests for downloader argument construction and option configurations.

use gfx_tools::downloader::{build_yt_dlp_args, AdvancedOptions, DownloadOptions};
use std::path::PathBuf;

fn base_options() -> DownloadOptions {
    DownloadOptions {
        url: "https://www.youtube.com/watch?v=test12345".to_string(),
        output_dir: PathBuf::from(r"D:\Videos"),
        format: "mp4".to_string(),
        advanced: None,
    }
}

#[test]
fn default_options_generates_h264_priority_and_fast_postprocessor() {
    let opts = base_options();
    let (format_str, merge_fmt, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    assert_eq!(is_audio_only, false);
    assert_eq!(merge_fmt, "mp4");
    assert!(format_str.contains("vcodec^=avc1"));
    assert!(format_str.contains("vcodec^=h264"));
    assert!(pp_args.contains("-c:v libx264"));
    assert!(pp_args.contains("-c:a aac"));
    assert!(pp_args.contains("-b:a 192k"));
}

#[test]
fn audio_only_options_generates_bestaudio_and_sets_flag() {
    let mut opts = base_options();
    opts.advanced = Some(AdvancedOptions {
        video: false,
        audio: true,
        audio_format: Some("mp3".to_string()),
        max_resolution: None,
        audio_bitrate: Some("320k".to_string()),
        start_time: None,
        end_time: None,
    });

    let (format_str, _, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    assert_eq!(is_audio_only, true);
    assert_eq!(format_str, "bestaudio");
    // Codec postprocessing should be empty so yt-dlp --extract-audio handles it
    assert!(pp_args.is_empty());
}

#[test]
fn video_only_options_strips_audio_with_an_flag() {
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

    let (format_str, _, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    assert_eq!(is_audio_only, false);
    assert_eq!(format_str, "bestvideo");
    assert!(pp_args.contains("-an"));
}

#[test]
fn video_only_with_resolution_cap() {
    let mut opts = base_options();
    opts.advanced = Some(AdvancedOptions {
        video: true,
        audio: false,
        audio_format: None,
        max_resolution: Some(1080),
        audio_bitrate: None,
        start_time: None,
        end_time: None,
    });

    let (format_str, _, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    assert_eq!(is_audio_only, false);
    assert_eq!(format_str, "bestvideo[height<=1080]");
    assert!(pp_args.contains("-an"));
}

#[test]
fn video_and_audio_with_resolution_cap_and_custom_bitrate() {
    let mut opts = base_options();
    opts.advanced = Some(AdvancedOptions {
        video: true,
        audio: true,
        audio_format: None,
        max_resolution: Some(720),
        audio_bitrate: Some("320k".to_string()),
        start_time: None,
        end_time: None,
    });

    let (format_str, _, pp_args, is_audio_only) = build_yt_dlp_args(&opts);

    assert_eq!(is_audio_only, false);
    assert!(format_str.contains("height<=720"));
    assert!(pp_args.contains("-b:a 320k"));
}
