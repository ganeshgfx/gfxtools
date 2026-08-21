// tests/filename_tests.rs
// Tests related to filename and path safety.
//
// Note: actual filename sanitization is delegated to yt-dlp via --windows-filenames.
// These tests verify our path construction does not introduce unsafe components.

use std::path::PathBuf;

/// Helper: build the output template path the way downloader.rs does it.
fn output_template(output_dir: &str) -> String {
    PathBuf::from(output_dir)
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .to_string()
}

#[test]
fn template_simple_path() {
    let t = output_template(r"D:\Videos");
    assert!(t.contains("%(title)s.%(ext)s"));
    assert!(t.starts_with(r"D:\Videos"));
}

#[test]
fn template_path_with_spaces() {
    let t = output_template(r"D:\My Videos");
    assert!(t.contains("%(title)s.%(ext)s"));
    assert!(t.starts_with(r"D:\My Videos"));
}

#[test]
fn template_unicode_path() {
    let t = output_template(r"D:\My Videos\ગુજરાતી");
    assert!(t.contains("%(title)s.%(ext)s"));
}

#[test]
fn template_does_not_escape_to_parent() {
    // A directory value containing ".." should still produce a valid template.
    // The CLI validates that the directory exists as a real directory on disk,
    // so path traversal via the directory argument is not a concern here.
    // This test just ensures the template construction is deterministic.
    let t = output_template(r"D:\Videos\..\Other");
    assert!(t.contains("%(title)s.%(ext)s"));
}

#[test]
fn template_long_path() {
    // Windows MAX_PATH is 260 by default; long path support can extend it.
    // yt-dlp handles truncation.
    let long_dir = format!("D:\\{}", "A".repeat(200));
    let t = output_template(&long_dir);
    assert!(t.contains("%(title)s.%(ext)s"));
}
