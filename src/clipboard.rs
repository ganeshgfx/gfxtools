// src/clipboard.rs
// Reads text from the Windows clipboard using `arboard`.
//
// Security: clipboard contents are NEVER executed as a shell command.
// They are treated as untrusted user-supplied data and only passed as a
// literal argument to the yt-dlp child process.

use crate::error::AppError;
use arboard::Clipboard;
use tracing::debug;

/// Read and return the current clipboard text.
///
/// Returns:
/// - `Ok(text)` — trimmed, non-empty clipboard text.
/// - `Err(ClipboardEmpty)` — clipboard has no text.
/// - `Err(ClipboardError)` — OS-level clipboard error.
pub fn read_clipboard_text() -> Result<String, AppError> {
    let mut clipboard =
        Clipboard::new().map_err(|e| AppError::ClipboardError(e.to_string()))?;

    let text = clipboard
        .get_text()
        .map_err(|e| AppError::ClipboardError(e.to_string()))?;

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::ClipboardEmpty);
    }

    debug!("Clipboard contents (trimmed): {trimmed}");
    Ok(trimmed)
}
