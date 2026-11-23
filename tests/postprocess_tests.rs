// tests/postprocess_tests.rs
// Tests for postprocessing operations, time parsing, and video file discovery.

use gfx_tools::postprocess::{find_video_files, parse_out_time, PostprocessOp};
use std::fs::File;

#[test]
fn parse_out_time_valid_timestamps() {
    // 1 hour, 2 minutes, 3.5 seconds = 3600 + 120 + 3.5 = 3723.5
    let secs = parse_out_time("01:02:03.500000").expect("parsed valid timestamp");
    assert!((secs - 3723.5).abs() < 1e-6);

    let secs_zero = parse_out_time("00:00:00.000000").expect("parsed zero timestamp");
    assert!((secs_zero - 0.0).abs() < 1e-6);

    let secs_min = parse_out_time("00:01:30.00").expect("parsed minute timestamp");
    assert!((secs_min - 90.0).abs() < 1e-6);
}

#[test]
fn parse_out_time_invalid_or_special_inputs() {
    assert_eq!(parse_out_time("N/A"), None);
    assert_eq!(parse_out_time("-00:00:01.000000"), None);
    assert_eq!(parse_out_time("invalid"), None);
    assert_eq!(parse_out_time("00:10"), None);
    assert_eq!(parse_out_time(""), None);
}

#[test]
fn postprocess_op_display_names() {
    assert_eq!(
        PostprocessOp::ConvertCompatible.to_string(),
        "Convert to Compatible"
    );
    assert_eq!(PostprocessOp::Compress.to_string(), "Compress");
}

#[test]
fn find_video_files_filters_and_sorts_correctly() {
    let temp_dir = std::env::temp_dir().join(format!("gfx_test_videos_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("create temp dir");

    // Create mixed files
    let vid_b = temp_dir.join("b_clip.mp4");
    let vid_a = temp_dir.join("a_clip.MKV");
    let vid_c = temp_dir.join("c_clip.webm");
    let doc = temp_dir.join("readme.txt");
    let pic = temp_dir.join("photo.jpg");

    File::create(&vid_b).unwrap();
    File::create(&vid_a).unwrap();
    File::create(&vid_c).unwrap();
    File::create(&doc).unwrap();
    File::create(&pic).unwrap();

    let found = find_video_files(&temp_dir);

    // Clean up
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert_eq!(found.len(), 3);
    assert_eq!(found[0], vid_a);
    assert_eq!(found[1], vid_b);
    assert_eq!(found[2], vid_c);
}
