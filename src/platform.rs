// src/platform.rs
// Detects which video platform a URL belongs to.
//
// Rules:
// - Scheme must be "https" (enforced in url_validator).
// - Host matching is case-insensitive and ignores the "www." prefix.
// - yt-dlp is the actual extractor; we only categorise for logging/UX.

use crate::error::AppError;
use url::Url;
use tracing::debug;

/// Supported video platforms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    YouTube,
    Pinterest,
    Instagram,
    /// A URL that passed validation but isn't one of the listed platforms.
    Unsupported(String),
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::YouTube => write!(f, "YouTube"),
            Platform::Pinterest => write!(f, "Pinterest"),
            Platform::Instagram => write!(f, "Instagram"),
            Platform::Unsupported(host) => write!(f, "Unsupported({host})"),
        }
    }
}

/// Validates that the URL uses https and returns a parsed `Url`.
pub fn validate_url(raw: &str) -> Result<Url, AppError> {
    let url = Url::parse(raw).map_err(|_| AppError::InvalidUrl(raw.to_string()))?;

    if url.scheme() != "https" {
        return Err(AppError::InsecureUrl(url.scheme().to_string()));
    }

    Ok(url)
}

/// Detects the platform from a pre-validated URL.
///
/// Returns `Platform::Unsupported` (not an error) so callers can decide whether
/// to proceed or reject. The downloader will still hand unsupported-platform
/// URLs to yt-dlp — yt-dlp decides the final "downloadable" truth.
pub fn detect_platform(url: &Url) -> Result<Platform, AppError> {
    let host = url
        .host_str()
        .ok_or_else(|| AppError::InvalidUrl("URL has no host".to_string()))?
        .to_lowercase();

    // Strip common prefix for simpler matching
    let host = host.strip_prefix("www.").unwrap_or(&host);

    let platform = match host {
        // ── YouTube ──────────────────────────────────────────────────────────
        "youtube.com"
        | "youtu.be"
        | "m.youtube.com"
        | "music.youtube.com"
        | "gaming.youtube.com" => Platform::YouTube,

        // ── Pinterest ────────────────────────────────────────────────────────
        "pinterest.com"
        | "pin.it"
        | "pinterest.co.uk"
        | "pinterest.ca"
        | "pinterest.com.au"
        | "pinterest.fr"
        | "pinterest.de"
        | "pinterest.es"
        | "pinterest.it"
        | "pinterest.jp"
        | "pinterest.pt"
        | "pinterest.ru"
        | "pinterest.nz"
        | "pinterest.ph"
        | "pinterest.cl"
        | "pinterest.mx"
        | "pinterest.at"
        | "pinterest.ch"
        | "pinterest.co"
        | "pinterest.dk"
        | "pinterest.ie"
        | "pinterest.in"
        | "pinterest.se"
        | "pinterest.no"
        | "pinterest.fi" => Platform::Pinterest,

        // ── Instagram ────────────────────────────────────────────────────────
        "instagram.com" | "instagr.am" => Platform::Instagram,

        // ── Everything else passes through to yt-dlp ─────────────────────────
        other => Platform::Unsupported(other.to_string()),
    };

    debug!("Detected platform: {platform} for host: {host}");
    Ok(platform)
}

/// Combined validate + detect helper.
pub fn validate_and_detect(raw: &str) -> Result<(Url, Platform), AppError> {
    let url = validate_url(raw)?;
    let platform = detect_platform(&url)?;
    Ok((url, platform))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(url: &str) -> Platform {
        let u = validate_url(url).expect("valid URL");
        detect_platform(&u).expect("detect ok")
    }

    // ── YouTube ──────────────────────────────────────────────────────────────
    #[test]
    fn youtube_watch() {
        assert_eq!(platform("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), Platform::YouTube);
    }

    #[test]
    fn youtube_short() {
        assert_eq!(platform("https://youtu.be/dQw4w9WgXcQ"), Platform::YouTube);
    }

    #[test]
    fn youtube_mobile() {
        assert_eq!(platform("https://m.youtube.com/watch?v=abc"), Platform::YouTube);
    }

    #[test]
    fn youtube_music() {
        assert_eq!(platform("https://music.youtube.com/watch?v=abc"), Platform::YouTube);
    }

    // ── Pinterest ────────────────────────────────────────────────────────────
    #[test]
    fn pinterest_main() {
        assert_eq!(platform("https://www.pinterest.com/pin/12345/"), Platform::Pinterest);
    }

    #[test]
    fn pinterest_short() {
        assert_eq!(platform("https://pin.it/abc123"), Platform::Pinterest);
    }

    #[test]
    fn pinterest_uk() {
        assert_eq!(platform("https://www.pinterest.co.uk/pin/12345/"), Platform::Pinterest);
    }

    // ── Instagram ────────────────────────────────────────────────────────────
    #[test]
    fn instagram_reel() {
        assert_eq!(
            platform("https://www.instagram.com/reel/ABC123/"),
            Platform::Instagram
        );
    }

    #[test]
    fn instagram_post() {
        assert_eq!(
            platform("https://www.instagram.com/p/ABC123/"),
            Platform::Instagram
        );
    }

    // ── Unsupported ──────────────────────────────────────────────────────────
    #[test]
    fn unsupported_example() {
        let p = platform("https://example.com/video");
        assert!(matches!(p, Platform::Unsupported(_)));
    }

    // ── Validation errors ────────────────────────────────────────────────────
    #[test]
    fn reject_http() {
        assert!(validate_url("http://youtube.com/watch?v=x").is_err());
    }

    #[test]
    fn reject_not_a_url() {
        assert!(validate_url("not-a-url").is_err());
    }

    #[test]
    fn reject_empty() {
        assert!(validate_url("").is_err());
    }
}
