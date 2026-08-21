// tests/platform_tests.rs
// Integration-style unit tests for URL validation + platform detection.
// No network access; all tests run offline.

// Bring in crate modules via a path — integration tests use the compiled crate.
// For tests in the tests/ directory, we reference the crate by name.

use paste_link_downloader::platform::{detect_platform, validate_url, Platform};

// ── Helper ───────────────────────────────────────────────────────────────────

fn platform_of(url: &str) -> Platform {
    let u = validate_url(url).expect("URL should be valid");
    detect_platform(&u).expect("detect should succeed")
}

// ── YouTube ───────────────────────────────────────────────────────────────────

#[test]
fn yt_watch_url() {
    assert_eq!(
        platform_of("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
        Platform::YouTube
    );
}

#[test]
fn yt_short_url() {
    assert_eq!(
        platform_of("https://youtu.be/dQw4w9WgXcQ"),
        Platform::YouTube
    );
}

#[test]
fn yt_mobile_url() {
    assert_eq!(
        platform_of("https://m.youtube.com/watch?v=abc123"),
        Platform::YouTube
    );
}

#[test]
fn yt_music_url() {
    assert_eq!(
        platform_of("https://music.youtube.com/watch?v=abc123"),
        Platform::YouTube
    );
}

#[test]
fn yt_no_www_url() {
    assert_eq!(
        platform_of("https://youtube.com/watch?v=abc123"),
        Platform::YouTube
    );
}

// ── Pinterest ─────────────────────────────────────────────────────────────────

#[test]
fn pinterest_pin_url() {
    assert_eq!(
        platform_of("https://www.pinterest.com/pin/1234567890/"),
        Platform::Pinterest
    );
}

#[test]
fn pinterest_short_url() {
    assert_eq!(
        platform_of("https://pin.it/AbCdEfG"),
        Platform::Pinterest
    );
}

#[test]
fn pinterest_uk_url() {
    assert_eq!(
        platform_of("https://www.pinterest.co.uk/pin/1234/"),
        Platform::Pinterest
    );
}

#[test]
fn pinterest_no_www_url() {
    assert_eq!(
        platform_of("https://pinterest.com/pin/1234/"),
        Platform::Pinterest
    );
}

// ── Instagram ─────────────────────────────────────────────────────────────────

#[test]
fn instagram_reel_url() {
    assert_eq!(
        platform_of("https://www.instagram.com/reel/ABC123def/"),
        Platform::Instagram
    );
}

#[test]
fn instagram_post_url() {
    assert_eq!(
        platform_of("https://www.instagram.com/p/ABC123def/"),
        Platform::Instagram
    );
}

#[test]
fn instagram_no_www_url() {
    assert_eq!(
        platform_of("https://instagram.com/reel/XYZ789/"),
        Platform::Instagram
    );
}

// ── Unsupported ───────────────────────────────────────────────────────────────

#[test]
fn unsupported_example_com() {
    let p = platform_of("https://example.com/video.mp4");
    assert!(matches!(p, Platform::Unsupported(_)));
}

#[test]
fn unsupported_tiktok() {
    let p = platform_of("https://www.tiktok.com/@user/video/123");
    assert!(matches!(p, Platform::Unsupported(_)));
}

#[test]
fn unsupported_vimeo() {
    let p = platform_of("https://vimeo.com/123456789");
    assert!(matches!(p, Platform::Unsupported(_)));
}

// ── Validation errors ─────────────────────────────────────────────────────────

#[test]
fn reject_http_scheme() {
    assert!(validate_url("http://www.youtube.com/watch?v=x").is_err());
}

#[test]
fn reject_ftp_scheme() {
    assert!(validate_url("ftp://example.com/file.mp4").is_err());
}

#[test]
fn reject_bare_string() {
    assert!(validate_url("not-a-url").is_err());
}

#[test]
fn reject_empty_string() {
    assert!(validate_url("").is_err());
}

#[test]
fn reject_javascript_injection() {
    assert!(validate_url("javascript:alert(1)").is_err());
}
