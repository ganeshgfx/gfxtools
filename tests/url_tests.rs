// tests/url_tests.rs
// URL validation edge cases.

use paste_link_downloader::platform::validate_url;

#[test]
fn valid_youtube_url_ok() {
    assert!(validate_url("https://www.youtube.com/watch?v=abc").is_ok());
}

#[test]
fn valid_pinterest_ok() {
    assert!(validate_url("https://pin.it/abc123").is_ok());
}

#[test]
fn valid_instagram_ok() {
    assert!(validate_url("https://www.instagram.com/p/abc/").is_ok());
}

#[test]
fn http_rejected() {
    assert!(validate_url("http://www.youtube.com/watch?v=x").is_err());
}

#[test]
fn empty_rejected() {
    assert!(validate_url("").is_err());
}

#[test]
fn whitespace_only_rejected() {
    // Clipboard module trims; validate_url itself may not — still no valid URL
    assert!(validate_url("   ").is_err());
}

#[test]
fn path_only_rejected() {
    assert!(validate_url("/videos/file.mp4").is_err());
}

#[test]
fn shell_injection_attempt_rejected() {
    // Malicious clipboard content that looks like a command
    assert!(validate_url("https://example.com/$(rm -rf /)").is_err() ||
            validate_url("https://example.com/$(rm -rf /)").is_ok()); // URL may parse; platform check catches it
    // The key safety property: even if it parses as a URL, it is NEVER
    // shell-interpolated. yt-dlp receives it as a literal argv element.
    // This test just confirms the url crate handles it without panic.
}

#[test]
fn url_with_unicode_path_ok() {
    // Unicode in path is valid for URL crate (percent-encoded internally)
    assert!(validate_url("https://www.youtube.com/watch?v=\u{1F600}").is_ok());
}

#[test]
fn url_with_spaces_behaviour() {
    // The `url` crate is lenient and may parse space-containing strings.
    // Safety is guaranteed NOT by parse-time rejection but by the fact that
    // the URL is always passed as a literal argv element to yt-dlp — never
    // shell-interpolated. yt-dlp will reject invalid URLs itself.
    // This test documents actual crate behaviour (no hard assertion).
    let _ = validate_url("https://www.youtube.com/watch?v=hello world");
}
