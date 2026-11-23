// tests/error_tests.rs
// Tests for AppError variants and user-facing error messages.

use gfx_tools::error::AppError;

#[test]
fn error_display_formatting() {
    let err = AppError::ClipboardEmpty;
    assert!(err.to_string().contains("empty"));

    let err = AppError::ClipboardError("access denied".to_string());
    assert!(err.to_string().contains("access denied"));

    let err = AppError::InvalidUrl("ftp://bad".to_string());
    assert!(err.to_string().contains("ftp://bad"));

    let err = AppError::InsecureUrl("http://example.com".to_string());
    assert!(err.to_string().contains("https"));

    let err = AppError::MissingYtDlp;
    assert!(err.to_string().contains("yt-dlp"));

    let err = AppError::MissingFfmpeg;
    assert!(err.to_string().contains("FFmpeg"));

    let err = AppError::MissingGalleryDl;
    assert!(err.to_string().contains("gallery-dl"));

    let err = AppError::Cancelled;
    assert!(err.to_string().contains("cancelled"));

    let err = AppError::ConfigError("syntax bad".to_string());
    assert!(err.to_string().contains("syntax bad"));
}
