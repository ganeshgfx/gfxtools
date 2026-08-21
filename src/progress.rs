// src/progress.rs
// Parses yt-dlp's stdout/stderr output into typed progress events.
//
// yt-dlp is invoked with --newline so each progress update is on its own line.
//
// Example lines:
//   [download]  45.2% of ~128.00MiB at 12.34MiB/s ETA 00:21
//   [download] 100% of 128.00MiB in 00:10 at 12.80MiB/s
//   [Merger] Merging formats into "video.mp4"
//   ERROR: Video unavailable
//   WARNING: Some content unavailable

use regex::Regex;
use std::sync::OnceLock;
use tracing::{debug, warn};

/// A parsed event from yt-dlp's output stream.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Download percentage (0.0 – 100.0).
    Percent(f64),
    /// Download speed string as reported by yt-dlp (e.g., "12.34MiB/s").
    Speed(String),
    /// ETA string as reported by yt-dlp (e.g., "00:21").
    Eta(String),
    /// yt-dlp is merging streams.
    Merging(String),
    /// yt-dlp emitted a WARNING: line.
    Warning(String),
    /// yt-dlp emitted an ERROR: line.
    Error(String),
    /// Download reported 100% complete.
    Complete,
    /// An output line that didn't match any known pattern.
    Other(String),
}

/// Regex patterns compiled once at first use.
/// Matches the percentage from a `[download]` line.
fn pct_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%")
            .expect("pct regex is valid")
    })
}

/// Extracts the speed from a download line (e.g. `at 12.34MiB/s`).
fn speed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\bat\s+(\S+)").expect("speed regex is valid")
    })
}

/// Extracts the ETA from a download line (e.g. `ETA 00:21`).
fn eta_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\bETA\s+(\S+)").expect("eta regex is valid")
    })
}

fn merger_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[(?:Merger|ffmpeg|ExtractAudio)\].*").expect("merger regex is valid")
    })
}

/// Parse a single line of yt-dlp output into a `ProgressEvent`.
pub fn parse_line(line: &str) -> ProgressEvent {
    let trimmed = line.trim();

    // ── Error / Warning ──────────────────────────────────────────────────────
    if let Some(msg) = trimmed.strip_prefix("ERROR:") {
        warn!("yt-dlp ERROR: {}", msg.trim());
        return ProgressEvent::Error(msg.trim().to_string());
    }
    if let Some(msg) = trimmed.strip_prefix("WARNING:") {
        warn!("yt-dlp WARNING: {}", msg.trim());
        return ProgressEvent::Warning(msg.trim().to_string());
    }

    // ── Merger / FFmpeg ──────────────────────────────────────────────────────
    if merger_re().is_match(trimmed) {
        debug!("yt-dlp merge/post-process: {trimmed}");
        return ProgressEvent::Merging(trimmed.to_string());
    }

    // ── Download progress ────────────────────────────────────────────────────
    if let Some(pct_caps) = pct_re().captures(trimmed) {
        let pct: f64 = pct_caps
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0.0);

        debug!("yt-dlp progress: {pct:.1}%");

        if (pct - 100.0).abs() < f64::EPSILON {
            return ProgressEvent::Complete;
        }

        let speed = speed_re()
            .captures(trimmed)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        let eta = eta_re()
            .captures(trimmed)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();

        if !speed.is_empty() || !eta.is_empty() {
            let summary = format!("{pct:.1}%  {speed}  ETA {eta}");
            return ProgressEvent::Speed(summary);
        }

        return ProgressEvent::Percent(pct);
    }

    ProgressEvent::Other(trimmed.to_string())
}

/// Extract numeric percentage from a `ProgressEvent` if available.
pub fn event_percent(event: &ProgressEvent) -> Option<f64> {
    match event {
        ProgressEvent::Percent(p) => Some(*p),
        ProgressEvent::Complete => Some(100.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_percent() {
        let ev = parse_line("[download]  45.2% of ~128.00MiB at 12.34MiB/s ETA 00:21");
        // Should be Speed (has speed+eta)
        assert!(matches!(ev, ProgressEvent::Speed(_)));
    }

    #[test]
    fn parse_complete() {
        let ev = parse_line("[download] 100% of 128.00MiB in 00:10 at 12.80MiB/s");
        assert!(matches!(ev, ProgressEvent::Complete));
    }

    #[test]
    fn parse_error() {
        let ev = parse_line("ERROR: Video unavailable");
        assert!(matches!(ev, ProgressEvent::Error(_)));
    }

    #[test]
    fn parse_warning() {
        let ev = parse_line("WARNING: Some warning message");
        assert!(matches!(ev, ProgressEvent::Warning(_)));
    }

    #[test]
    fn parse_merger() {
        let ev = parse_line(r#"[Merger] Merging formats into "D:\Videos\video.mp4""#);
        assert!(matches!(ev, ProgressEvent::Merging(_)));
    }

    #[test]
    fn parse_other() {
        let ev = parse_line("[info] Writing video thumbnail");
        assert!(matches!(ev, ProgressEvent::Other(_)));
    }
}
