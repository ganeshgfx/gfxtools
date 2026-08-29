// src/config.rs
// Loads optional per-user configuration from:
//   %APPDATA%\GFXTools\config.toml
//
// All fields have sensible defaults so the application works immediately
// after installation without any configuration.

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, warn};

/// Application configuration.
/// Stored as TOML at `%APPDATA%\GFXTools\config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Absolute path to yt-dlp.exe.
    /// Empty string = auto-detect (bundled → PATH).
    #[serde(default)]
    pub yt_dlp_path: String,

    /// Directory containing ffmpeg.exe and ffprobe.exe.
    /// Empty string = auto-detect (bundled → PATH).
    #[serde(default)]
    pub ffmpeg_dir: String,

    /// Absolute path to gallery-dl.exe (or gallery-dl on Linux/macOS).
    /// Empty string = auto-detect (bundled bin/ → PATH).
    #[serde(default)]
    pub gallery_dl_path: String,

    /// Show Windows notification dialogs on success / failure.
    #[serde(default = "default_true")]
    pub notifications: bool,

    /// Preferred output container format ("mp4", "mkv", etc.).
    #[serde(default = "default_format")]
    pub preferred_format: String,

    /// Log verbosity level ("error", "warn", "info", "debug", "trace").
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Browser to extract cookies from (passed to yt-dlp `--cookies-from-browser`).
    /// Supported values: "chrome", "firefox", "edge", "opera", "brave", "chromium".
    /// Empty string = disabled (no cookies passed).
    #[serde(default = "default_cookies_from_browser")]
    pub cookies_from_browser: String,

    /// Path to a Netscape-format cookies.txt file (passed to yt-dlp `--cookies`).
    /// Takes priority over `cookies_from_browser` when non-empty.
    /// Export from Chrome using the "Get cookies.txt LOCALLY" extension.
    /// Empty string = disabled.
    #[serde(default)]
    pub cookies_file: String,
}

fn default_true() -> bool {
    true
}

fn default_format() -> String {
    "mp4".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_cookies_from_browser() -> String {
    "edge".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            yt_dlp_path: String::new(),
            ffmpeg_dir: String::new(),
            gallery_dl_path: String::new(),
            notifications: true,
            preferred_format: default_format(),
            log_level: default_log_level(),
            cookies_from_browser: default_cookies_from_browser(),
            cookies_file: String::new(),
        }
    }
}

impl Config {
    /// Returns the config file path: `%APPDATA%\GFXTools\config.toml`
    pub fn config_path() -> Option<PathBuf> {
        dirs_from_env("APPDATA").map(|p| p.join("GFXTools").join("config.toml"))
    }

    /// Loads config from disk, returning defaults if the file doesn't exist.
    pub fn load() -> Result<Self, AppError> {
        let Some(path) = Self::config_path() else {
            warn!("APPDATA not set; using default configuration");
            return Ok(Self::default());
        };

        if !path.exists() {
            debug!("Config file not found at {path:?}; using defaults");
            return Ok(Self::default());
        }

        let contents = std::fs::read_to_string(&path)
            .map_err(|e| AppError::ConfigError(format!("Cannot read {path:?}: {e}")))?;

        let config: Config = toml::from_str(&contents)
            .map_err(|e| AppError::ConfigError(format!("Parse error in {path:?}: {e}")))?;

        debug!("Loaded config from {path:?}");
        Ok(config)
    }

    /// Writes the default config to disk (creates directories as needed).
    pub fn write_default() -> Result<(), AppError> {
        let Some(path) = Self::config_path() else {
            return Err(AppError::ConfigError("APPDATA not set".to_string()));
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(&Self::default())
            .map_err(|e| AppError::ConfigError(format!("Serialization error: {e}")))?;

        std::fs::write(&path, contents)?;
        Ok(())
    }

    /// Saves the current config to disk, creating the directory if needed.
    pub fn save(&self) -> Result<(), AppError> {
        let Some(path) = Self::config_path() else {
            return Err(AppError::ConfigError("APPDATA not set".to_string()));
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let contents = toml::to_string_pretty(self)
            .map_err(|e| AppError::ConfigError(format!("Serialization error: {e}")))?;

        std::fs::write(&path, &contents)
            .map_err(|e| AppError::ConfigError(format!("Cannot write {path:?}: {e}")))?;

        debug!("Saved config to {path:?}");
        Ok(())
    }

    /// Returns the config directory: `%APPDATA%\GFXTools`
    pub fn config_dir() -> Option<PathBuf> {
        dirs_from_env("APPDATA").map(|p| p.join("GFXTools"))
    }
}

/// Helper: read env var as PathBuf.
fn dirs_from_env(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}
