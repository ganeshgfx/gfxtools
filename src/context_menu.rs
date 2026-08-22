// src/context_menu.rs
// Windows Registry context-menu integration.
//
// Registers / unregisters the "Paste link" entry under:
//   HKCU\Software\Classes\Directory\Background\shell\PasteLink
//
// Uses HKCU (HKEY_CURRENT_USER) — no administrator privileges required.
//
// After any change, notifies the shell via SHChangeNotify so Explorer
// picks up the change without needing to be restarted.

use crate::error::AppError;
use std::path::Path;
use tracing::{info, warn};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, KEY_READ, REG_CREATE_KEY_DISPOSITION,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows::Win32::UI::Shell::{SHChangeNotify, SHCNE_ASSOCCHANGED, SHCNF_IDLIST};

/// The registry key path under HKCU\Software\Classes
const SHELL_KEY: &str = r"Software\Classes\Directory\Background\shell\PasteLink";
const COMMAND_SUBKEY: &str = r"Software\Classes\Directory\Background\shell\PasteLink\command";
const MENU_LABEL: &str = "Paste link";

// ── Public API ───────────────────────────────────────────────────────────────

/// Register the context-menu entry.
///
/// `exe_path` should be the absolute path to the installed executable.
///
/// Registry layout:
/// ```text
/// HKCU\Software\Classes\Directory\Background\shell\PasteLink
///   (Default)  = "Paste link"
///   Icon       = "C:\...\paste-link-downloader.exe"
/// HKCU\Software\Classes\Directory\Background\shell\PasteLink\command
///   (Default)  = "\"C:\...\paste-link-downloader.exe\" \"%V\""
/// ```
pub fn install(exe_path: &Path) -> Result<(), AppError> {
    let exe_str = exe_path.to_str().ok_or_else(|| {
        AppError::RegistryError("Executable path is not valid UTF-8".to_string())
    })?;

    // Set label and icon on the main key
    let shell_key = create_or_open_key(SHELL_KEY)?;
    set_string_value(&shell_key, "", MENU_LABEL)?;
    set_string_value(&shell_key, "Icon", exe_str)?;
    close_key(shell_key);

    // Set command: "<exe>" "%V"  (%V = folder path from Explorer)
    let command = format!("\"{}\" \"%V\"", exe_str);
    let cmd_key = create_or_open_key(COMMAND_SUBKEY)?;
    set_string_value(&cmd_key, "", &command)?;
    close_key(cmd_key);

    notify_shell();
    info!("Context menu registered. Exe: {exe_str}");
    Ok(())
}

/// Remove the context-menu entries.
pub fn uninstall() -> Result<(), AppError> {
    let parent_path = r"Software\Classes\Directory\Background\shell";
    let parent_key = open_key_for_write(parent_path)?;

    // Remove "PasteLink"
    let subkey_w = to_wide("PasteLink");
    // SAFETY: HKEY is a valid handle returned from RegOpenKeyExW above.
    let result = unsafe {
        RegDeleteTreeW(parent_key, PCWSTR(subkey_w.as_ptr()))
    };
    if result.is_err() {
        warn!("RegDeleteTreeW(PasteLink) returned error (may not exist): {:?}", result);
    }

    // Remove "PasteLinkImages"
    let images_subkey_w = to_wide("PasteLinkImages");
    let result2 = unsafe {
        RegDeleteTreeW(parent_key, PCWSTR(images_subkey_w.as_ptr()))
    };
    if result2.is_err() {
        warn!("RegDeleteTreeW(PasteLinkImages) returned error (may not exist): {:?}", result2);
    }

    close_key(parent_key);

    notify_shell();
    info!("Context menus unregistered");
    Ok(())
}

/// Check whether the context-menu entry is currently installed.
pub fn is_installed() -> bool {
    let key_w = to_wide(SHELL_KEY);
    // SAFETY: read-only registry open
    let result = unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_w.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
    };
    result.is_ok()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create (or open) a registry key under HKCU for writing.
fn create_or_open_key(path: &str) -> Result<HKEY, AppError> {
    let path_w = to_wide(path);
    let mut hkey = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();

    // SAFETY: we supply valid null-terminated UTF-16 strings and a valid
    // pointer to receive the key handle.
    let result = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            0,
            PWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            Some(&mut disposition),
        )
    };

    result.ok().map_err(|e| AppError::RegistryError(format!("RegCreateKeyExW({path}): {e}")))?;
    Ok(hkey)
}

/// Open an existing registry key under HKCU for writing.
fn open_key_for_write(path: &str) -> Result<HKEY, AppError> {
    let path_w = to_wide(path);
    let mut hkey = HKEY::default();

    // SAFETY: valid UTF-16 path, valid output pointer
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path_w.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        )
    };

    result.ok().map_err(|e| AppError::RegistryError(format!("RegOpenKeyExW({path}): {e}")))?;
    Ok(hkey)
}

/// Write a REG_SZ (string) value under an already-open key.
///
/// `name` = "" sets the default (unnamed) value.
fn set_string_value(hkey: &HKEY, name: &str, value: &str) -> Result<(), AppError> {
    let name_w = to_wide(name);
    let value_w = to_wide(value);
    // REG_SZ data is the raw UTF-16 bytes (including null terminator)
    let data: &[u8] = unsafe {
        std::slice::from_raw_parts(
            value_w.as_ptr() as *const u8,
            value_w.len() * 2,
        )
    };

    // SAFETY: hkey is valid, name_w and data are valid UTF-16
    let result = unsafe {
        RegSetValueExW(
            *hkey,
            PCWSTR(name_w.as_ptr()),
            0,
            REG_SZ,
            Some(data),
        )
    };

    result.ok().map_err(|e| {
        AppError::RegistryError(format!("RegSetValueExW(name={name}, value={value}): {e}"))
    })
}

/// Close a registry key handle.
fn close_key(hkey: HKEY) {
    // SAFETY: hkey was returned from a successful RegCreate/OpenKeyExW call.
    let _ = unsafe { RegCloseKey(hkey) };
}

/// Notify the shell that file associations have changed so Explorer refreshes
/// its context menus without requiring a restart.
fn notify_shell() {
    // SAFETY: SHChangeNotify is safe to call with these flags at any time.
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}

/// Convert a Rust `&str` to a null-terminated UTF-16 Vec.
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
