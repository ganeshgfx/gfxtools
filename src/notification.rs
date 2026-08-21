// src/notification.rs
// Windows user-facing notifications.
//
// v1 implementation: MessageBoxW via the `windows` crate.
//
// Architecture note: all public functions take plain Rust strings.
// The Windows API conversion (UTF-8 → UTF-16) is encapsulated here.
// Swap this module later to use Windows Toast (WinRT) without touching callers.

use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MB_TOPMOST,
};

/// Show a success dialog.
pub fn notify_success(title: &str, body: &str) {
    message_box(title, body, false);
}

/// Show an error dialog.
pub fn notify_error(title: &str, body: &str) {
    message_box(title, body, true);
}

/// Show a cancellation dialog.
pub fn notify_cancelled() {
    message_box("Download Cancelled", "The download was cancelled.", false);
}

fn message_box(title: &str, text: &str, is_error: bool) {
    let title_w = to_wide(title);
    let text_w = to_wide(text);

    let flags = if is_error {
        MB_OK | MB_ICONERROR | MB_TOPMOST
    } else {
        MB_OK | MB_ICONINFORMATION | MB_TOPMOST
    };

    // SAFETY: MessageBoxW is well-defined with null HWND (top-level),
    // and we supply valid null-terminated UTF-16 strings.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(text_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            flags,
        );
    }
}

/// Convert a Rust `&str` to a null-terminated UTF-16 Vec for Windows APIs.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
