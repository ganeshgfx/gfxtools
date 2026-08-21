// src/logging.rs
// Initialises tracing with a rolling file appender.
//
// Log location: %LOCALAPPDATA%\PasteLinkDownloader\logs\app.log
// Console output is also shown in debug builds.

use crate::error::AppError;
use std::path::PathBuf;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Returns the log directory path.
pub fn log_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("PasteLinkDownloader").join("logs"))
}

/// Initialise the global tracing subscriber.
///
/// - Always writes to a daily-rotating log file.
/// - In debug builds, also prints to stderr so developers can see output.
/// - `log_level` comes from the loaded `Config`.
pub fn init(log_level: &str) -> Result<(), AppError> {
    let dir = log_dir().ok_or_else(|| {
        AppError::ConfigError("LOCALAPPDATA env var not set; cannot initialise logging".to_string())
    })?;

    std::fs::create_dir_all(&dir)?;

    let file_appender = RollingFileAppender::new(Rotation::DAILY, &dir, "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Leak the guard so it lives for the entire process lifetime.
    // This is intentional — we need the background writer thread to stay alive.
    std::mem::forget(_guard);

    let filter = EnvFilter::try_new(log_level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);

    #[cfg(debug_assertions)]
    {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(true)
            .with_target(true);

        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .init();
    }

    info!("Logging initialised. Log dir: {:?}", dir);
    Ok(())
}
