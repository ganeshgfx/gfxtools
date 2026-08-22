// src/error.rs
// Central error type for the application.
// All public-facing errors are variants here so call sites can distinguish
// and display actionable messages to the user.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    // ── Clipboard ────────────────────────────────────────────────────────────
    #[error("Could not read the clipboard: {0}")]
    ClipboardError(String),

    #[error("Clipboard is empty")]
    ClipboardEmpty,

    // ── URL / Platform ───────────────────────────────────────────────────────
    #[error("Clipboard does not contain a valid URL: {0}")]
    InvalidUrl(String),

    #[error("URL scheme must be https (got: {0})")]
    InsecureUrl(String),

    #[error("URL host is not a supported platform: {0}")]
    UnsupportedPlatform(String),

    // ── Binaries ─────────────────────────────────────────────────────────────
    #[error(
        "yt-dlp executable not found.\n\
         Place yt-dlp.exe in the bin/ directory next to the application,\n\
         set yt_dlp_path in config.toml, or install yt-dlp on your PATH."
    )]
    MissingYtDlp,

    #[error(
        "FFmpeg executable not found.\n\
         Place ffmpeg.exe in the bin/ directory next to the application,\n\
         or set ffmpeg_dir in config.toml."
    )]
    MissingFfmpeg,

    #[error(
        "gallery-dl executable not found.\n\
         Place gallery-dl.exe in the bin/ directory next to the application,\n\
         set gallery_dl_path in config.toml, or install gallery-dl on your PATH."
    )]
    MissingGalleryDl,

    // ── Directory ────────────────────────────────────────────────────────────
    #[error("Directory argument is missing or invalid")]
    MissingDirectory,

    #[error("Directory does not exist or is not accessible: {0}")]
    InvalidDirectory(String),

    // ── Process ──────────────────────────────────────────────────────────────
    #[error("Failed to spawn process '{binary}': {source}")]
    ProcessSpawnError {
        binary: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("yt-dlp exited with code {0}")]
    YtDlpExitCode(i32),

    #[error("gallery-dl exited with code {0}")]
    GalleryDlExitCode(i32),

    // ── Cancellation ─────────────────────────────────────────────────────────
    #[error("Download was cancelled by the user")]
    Cancelled,

    // ── Registry ─────────────────────────────────────────────────────────────
    #[error("Registry operation failed: {0}")]
    RegistryError(String),

    // ── Config ───────────────────────────────────────────────────────────────
    #[error("Configuration error: {0}")]
    ConfigError(String),

    // ── I/O ──────────────────────────────────────────────────────────────────
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}
