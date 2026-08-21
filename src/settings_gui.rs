// src/settings_gui.rs
//
// Native Win32 settings window for Paste Link Downloader.
// Requires: windows-rs 0.58 with Win32_UI_Controls, Win32_UI_WindowsAndMessaging,
//           Win32_Graphics_Gdi, Win32_System_LibraryLoader features.
//
// Opened by: `paste-link-downloader.exe --settings`

#![allow(non_snake_case, clippy::cast_sign_loss, clippy::cast_possible_truncation)]

use crate::config::Config;
use crate::error::AppError;

use std::ffi::OsStr;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::ptr::null_mut;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, SetBkColor, SetTextColor, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OPEN_FILENAME_FLAGS, OPENFILENAMEW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_AUTOCHECKBOX, BS_PUSHBUTTON, BM_GETCHECK, BM_SETCHECK,
    CBS_DROPDOWNLIST, CBS_HASSTRINGS,
    CB_ADDSTRING, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN, CB_SETCURSEL,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    CreateWindowExW, DefWindowProcW, DispatchMessageW,
    ES_AUTOHSCROLL,
    GetDlgItem, GetMessageW, GetWindowLongPtrW, GetWindowTextW,
    GWLP_USERDATA,
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW,
    MSG, PostQuitMessage, RegisterClassExW,
    SendMessageW, SetWindowLongPtrW, SetWindowTextW, ShowWindow, SW_SHOWNORMAL,
    TranslateMessage,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY,
    WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN, WS_OVERLAPPED, WS_SYSMENU,
    WS_TABSTOP, WS_VISIBLE,
    WS_EX_APPWINDOW, WS_EX_CLIENTEDGE,
    WINDOW_STYLE, HMENU,
};

// ── BST values ────────────────────────────────────────────────────────────────
const BST_CHECKED: usize = 1;
const BST_UNCHECKED: usize = 0;

// ── Control IDs ───────────────────────────────────────────────────────────────
const ID_EDIT_YTDLP: i32 = 101;
const ID_BTN_YTDLP: usize = 102;
const ID_EDIT_FFMPEG: i32 = 103;
const ID_BTN_FFMPEG: usize = 104;
const ID_EDIT_COOKIES_FILE: i32 = 105;
const ID_BTN_COOKIES_FILE: usize = 106;
const ID_COMBO_BROWSER: i32 = 107;
const ID_COMBO_FORMAT: i32 = 108;
const ID_COMBO_LOGLEVEL: i32 = 109;
const ID_CHECK_NOTIF: i32 = 110;
const ID_BTN_SAVE: usize = 111;
const ID_BTN_CANCEL: usize = 112;
const ID_BTN_OPEN_CONFIG: usize = 113;

// ── Colour helpers (COLORREF is 0x00BBGGRR) ───────────────────────────────────
fn cr(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}

fn solid(r: u8, g: u8, b: u8) -> HBRUSH {
    unsafe { CreateSolidBrush(cr(r, g, b)) }
}

// Catppuccin Mocha palette
const BG_R: u8 = 30;  const BG_G: u8 = 30;  const BG_B: u8 = 46;
const ED_R: u8 = 49;  const ED_G: u8 = 50;  const ED_B: u8 = 68;
const TX_R: u8 = 205; const TX_G: u8 = 214; const TX_B: u8 = 244;

// ── Window class name ─────────────────────────────────────────────────────────
const CLASS_NAME: &str = "PLD_SettingsWnd";

// ── UTF-16 helpers ────────────────────────────────────────────────────────────

fn w(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

unsafe fn hwnd_text(hwnd: HWND) -> String {
    let mut buf = [0u16; 1024];
    let n = GetWindowTextW(hwnd, &mut buf);
    from_wide(&buf[..n as usize])
}

// ── HMENU from control id ─────────────────────────────────────────────────────
fn id_menu(id: usize) -> HMENU {
    HMENU(id as *mut _)
}

// ── Combo helpers ─────────────────────────────────────────────────────────────

unsafe fn combo_add(hwnd: HWND, s: &str) {
    let ws = w(s);
    SendMessageW(hwnd, CB_ADDSTRING, WPARAM(0), LPARAM(ws.as_ptr() as isize));
}

unsafe fn combo_set(hwnd: HWND, items: &[&str], cur: &str) {
    for item in items { combo_add(hwnd, item); }
    let idx = items.iter().position(|&i| i == cur).unwrap_or(0);
    SendMessageW(hwnd, CB_SETCURSEL, WPARAM(idx), LPARAM(0));
}

unsafe fn combo_get(hwnd: HWND) -> String {
    let idx = SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
    if idx < 0 { return String::new(); }
    let len = SendMessageW(hwnd, CB_GETLBTEXTLEN, WPARAM(idx as usize), LPARAM(0)).0;
    if len < 0 { return String::new(); }
    let mut buf = vec![0u16; len as usize + 2];
    SendMessageW(hwnd, CB_GETLBTEXT, WPARAM(idx as usize), LPARAM(buf.as_mut_ptr() as isize));
    from_wide(&buf)
}

// ── File picker ───────────────────────────────────────────────────────────────

unsafe fn pick_file(owner: HWND, title: &str, filter: &str) -> Option<String> {
    let fw: Vec<u16> = filter.encode_utf16().chain(std::iter::once(0)).collect();
    let tw = w(title);
    let mut buf = [0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(fw.as_ptr()),
        lpstrFile: windows::core::PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(tw.as_ptr()),
        Flags: OFN_PATHMUSTEXIST | OFN_FILEMUSTEXIST,
        ..Default::default()
    };
    if GetOpenFileNameW(&mut ofn).as_bool() {
        Some(from_wide(&buf))
    } else {
        None
    }
}

/// Pick a folder using GetOpenFileNameW with OFN_PICKFOLDERS (0x20000).
unsafe fn pick_folder(owner: HWND, title: &str) -> Option<String> {
    const OFN_PICKFOLDERS: u32 = 0x0002_0000;
    let fw: Vec<u16> = "Folder\0\\\0\0".encode_utf16().chain(std::iter::once(0)).collect();
    let tw = w(title);
    let mut buf = [0u16; 1024];
    let mut ofn = OPENFILENAMEW {
        lStructSize: mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner,
        lpstrFilter: PCWSTR(fw.as_ptr()),
        lpstrFile: windows::core::PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(tw.as_ptr()),
        Flags: OFN_PATHMUSTEXIST | OPEN_FILENAME_FLAGS(OFN_PICKFOLDERS),
        ..Default::default()
    };
    if GetOpenFileNameW(&mut ofn).as_bool() {
        Some(from_wide(&buf))
    } else {
        None
    }
}

// ── WndProc ───────────────────────────────────────────────────────────────────

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            // lp is a pointer to CREATESTRUCTW; its lpCreateParams holds our Config ptr.
            let cs = lp.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW;
            let cfg_ptr = (*cs).lpCreateParams as *const Config;
            // Store for later use by on_command (browse callbacks, etc.)
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cfg_ptr as isize);
            create_controls(hwnd);
            LRESULT(0)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
            let hdc = windows::Win32::Graphics::Gdi::HDC(wp.0 as *mut _);
            SetTextColor(hdc, cr(TX_R, TX_G, TX_B));
            SetBkColor(hdc, cr(ED_R, ED_G, ED_B));
            LRESULT(solid(ED_R, ED_G, ED_B).0 as isize)
        }
        WM_COMMAND => {
            on_command(hwnd, wp.0 & 0xFFFF);
            LRESULT(0)
        }
        WM_CLOSE | WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

// ── Helper to unwrap GetDlgItem ───────────────────────────────────────────────

unsafe fn dlg(hwnd: HWND, id: i32) -> HWND {
    GetDlgItem(hwnd, id).unwrap_or_default()
}

unsafe fn set_edit(hwnd: HWND, s: &str) {
    let ws = w(s);
    let _ = SetWindowTextW(hwnd, PCWSTR(ws.as_ptr()));
}

// ── Command handler ───────────────────────────────────────────────────────────

unsafe fn on_command(hwnd: HWND, id: usize) {
    match id {
        ID_BTN_YTDLP => {
            if let Some(p) = pick_file(hwnd, "Select yt-dlp executable",
                "Executable\0yt-dlp.exe\0All files\0*.*\0\0") {
                set_edit(dlg(hwnd, ID_EDIT_YTDLP), &p);
            }
        }
        ID_BTN_FFMPEG => {
            if let Some(d) = pick_folder(hwnd, "Select FFmpeg directory (contains ffmpeg.exe)") {
                set_edit(dlg(hwnd, ID_EDIT_FFMPEG), &d);
            }
        }
        ID_BTN_COOKIES_FILE => {
            if let Some(p) = pick_file(hwnd, "Select cookies.txt",
                "Text files\0*.txt\0All files\0*.*\0\0") {
                set_edit(dlg(hwnd, ID_EDIT_COOKIES_FILE), &p);
            }
        }
        ID_BTN_OPEN_CONFIG => {
            if let Some(dir) = Config::config_dir() {
                let _ = std::process::Command::new("explorer")
                    .arg(dir.to_string_lossy().as_ref())
                    .spawn();
            }
        }
        ID_BTN_SAVE   => save_config(hwnd),
        ID_BTN_CANCEL => { PostQuitMessage(0); }
        _ => {}
    }
}

// ── Save config ───────────────────────────────────────────────────────────────

unsafe fn save_config(hwnd: HWND) {
    let yt_dlp_path   = hwnd_text(dlg(hwnd, ID_EDIT_YTDLP));
    let ffmpeg_dir    = hwnd_text(dlg(hwnd, ID_EDIT_FFMPEG));
    let cookies_file  = hwnd_text(dlg(hwnd, ID_EDIT_COOKIES_FILE));
    let mut cookies_from_browser = combo_get(dlg(hwnd, ID_COMBO_BROWSER));
    let preferred_format = combo_get(dlg(hwnd, ID_COMBO_FORMAT));
    let log_level        = combo_get(dlg(hwnd, ID_COMBO_LOGLEVEL));
    let notif = SendMessageW(dlg(hwnd, ID_CHECK_NOTIF), BM_GETCHECK, WPARAM(0), LPARAM(0)).0;
    let notifications = notif == BST_CHECKED as isize;

    if cookies_from_browser == "disabled" { cookies_from_browser.clear(); }

    let cfg = Config { yt_dlp_path, ffmpeg_dir, cookies_file, cookies_from_browser,
                       preferred_format, log_level, notifications };

    match cfg.save() {
        Ok(()) => {
            let m = w("Settings saved!"); let t = w("Saved");
            MessageBoxW(hwnd, PCWSTR(m.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONINFORMATION);
            PostQuitMessage(0);
        }
        Err(e) => {
            let m = w(&format!("Save failed:\n{e}")); let t = w("Error");
            MessageBoxW(hwnd, PCWSTR(m.as_ptr()), PCWSTR(t.as_ptr()), MB_OK | MB_ICONERROR);
        }
    }
}

// ── Control creation ──────────────────────────────────────────────────────────

const LABEL_W: i32 = 135;
const EDIT_H: i32 = 22;
const COMBO_H: i32 = 160;
const BROWSE_W: i32 = 60;
const COMBO_W: i32 = 175;
const MARGIN: i32 = 12;
const ROW_H: i32 = 34;
const LABEL_H: i32 = 18;

unsafe fn create_controls(hwnd: HWND) {
    // Config was stored in USERDATA by WM_CREATE before this call.
    let cfg_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Config;
    let cfg = if cfg_ptr.is_null() { Config::default() } else { (*cfg_ptr).clone() };

    let win_w = 490;
    let ex = MARGIN + LABEL_W;
    let ew = win_w - MARGIN * 2 - LABEL_W - BROWSE_W - 6;

    macro_rules! static_lbl {
        ($text:expr, $x:expr, $y:expr) => {{
            let cw = w("STATIC"); let tw = w($text);
            let _ = CreateWindowExW(Default::default(), PCWSTR(cw.as_ptr()), PCWSTR(tw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0), $x, $y, LABEL_W, LABEL_H,
                hwnd, HMENU(null_mut()), HINSTANCE(null_mut()), None);
        }};
    }

    macro_rules! edit_ctrl {
        ($id:expr, $val:expr, $x:expr, $y:expr, $w:expr) => {{
            let cw = w("EDIT"); let vw = w($val);
            let _ = CreateWindowExW(WS_EX_CLIENTEDGE, PCWSTR(cw.as_ptr()), PCWSTR(vw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | ES_AUTOHSCROLL as u32),
                $x, $y, $w, EDIT_H,
                hwnd, id_menu($id as usize), HINSTANCE(null_mut()), None);
        }};
    }

    macro_rules! btn {
        ($id:expr, $text:expr, $x:expr, $y:expr, $w:expr, $h:expr) => {{
            let cw = w("BUTTON"); let tw = w($text);
            let _ = CreateWindowExW(Default::default(), PCWSTR(cw.as_ptr()), PCWSTR(tw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
                $x, $y, $w, $h,
                hwnd, id_menu($id), HINSTANCE(null_mut()), None);
        }};
    }

    macro_rules! combo {
        ($id:expr, $x:expr, $y:expr) => {{
            let cw = w("COMBOBOX"); let ew2 = w("");
            CreateWindowExW(Default::default(), PCWSTR(cw.as_ptr()), PCWSTR(ew2.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0
                    | CBS_DROPDOWNLIST as u32 | CBS_HASSTRINGS as u32),
                $x, $y, COMBO_W, COMBO_H,
                hwnd, id_menu($id as usize), HINSTANCE(null_mut()), None)
                .unwrap_or_default()
        }};
    }

    macro_rules! checkbox {
        ($id:expr, $text:expr, $x:expr, $y:expr, $w:expr) => {{
            let cw = w("BUTTON"); let tw = w($text);
            CreateWindowExW(Default::default(), PCWSTR(cw.as_ptr()), PCWSTR(tw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32),
                $x, $y, $w, EDIT_H,
                hwnd, id_menu($id as usize), HINSTANCE(null_mut()), None)
                .unwrap_or_default()
        }};
    }

    let mut y = MARGIN + 6;

    // yt-dlp path
    static_lbl!("yt-dlp path:", MARGIN, y + 3);
    edit_ctrl!(ID_EDIT_YTDLP, &cfg.yt_dlp_path, ex, y, ew);
    btn!(ID_BTN_YTDLP, "Browse…", ex + ew + 4, y, BROWSE_W, EDIT_H);
    y += ROW_H;

    // FFmpeg dir
    static_lbl!("FFmpeg dir:", MARGIN, y + 3);
    edit_ctrl!(ID_EDIT_FFMPEG, &cfg.ffmpeg_dir, ex, y, ew);
    btn!(ID_BTN_FFMPEG, "Browse…", ex + ew + 4, y, BROWSE_W, EDIT_H);
    y += ROW_H;

    // Cookies file
    static_lbl!("Cookies file:", MARGIN, y + 3);
    edit_ctrl!(ID_EDIT_COOKIES_FILE, &cfg.cookies_file, ex, y, ew);
    btn!(ID_BTN_COOKIES_FILE, "Browse…", ex + ew + 4, y, BROWSE_W, EDIT_H);
    y += ROW_H;

    // Cookie browser
    static_lbl!("Cookie browser:", MARGIN, y + 3);
    let cb_browser = combo!(ID_COMBO_BROWSER, ex, y);
    let browser = if cfg.cookies_from_browser.is_empty() { "disabled" } else { &cfg.cookies_from_browser };
    combo_set(cb_browser, &["disabled","edge","chrome","firefox","brave","opera","chromium"], browser);
    y += ROW_H;

    // Output format
    static_lbl!("Output format:", MARGIN, y + 3);
    let cb_fmt = combo!(ID_COMBO_FORMAT, ex, y);
    combo_set(cb_fmt, &["mp4","mkv","webm","mov","avi"], &cfg.preferred_format);
    y += ROW_H;

    // Log level
    static_lbl!("Log level:", MARGIN, y + 3);
    let cb_log = combo!(ID_COMBO_LOGLEVEL, ex, y);
    combo_set(cb_log, &["error","warn","info","debug","trace"], &cfg.log_level);
    y += ROW_H;

    // Notifications
    static_lbl!("Notifications:", MARGIN, y + 3);
    let chk = checkbox!(ID_CHECK_NOTIF, "Enable desktop notifications", ex, y, 240);
    SendMessageW(chk, BM_SETCHECK,
        WPARAM(if cfg.notifications { BST_CHECKED } else { BST_UNCHECKED }), LPARAM(0));
    y += ROW_H;

    // Hint label
    y += 4;
    {
        let cw = w("STATIC");
        let tw = w("Leave path fields empty to auto-detect bundled/PATH binaries.");
        let _ = CreateWindowExW(Default::default(), PCWSTR(cw.as_ptr()), PCWSTR(tw.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0), MARGIN, y, win_w - MARGIN * 2, LABEL_H,
            hwnd, HMENU(null_mut()), HINSTANCE(null_mut()), None);
    }
    y += 26;

    // Bottom buttons
    btn!(ID_BTN_OPEN_CONFIG, "Open Config Folder", MARGIN, y, 148, 26);
    btn!(ID_BTN_CANCEL, "Cancel", win_w - 172, y, 76, 26);
    btn!(ID_BTN_SAVE, "Save", win_w - 88, y, 76, 26);
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Open the settings GUI. Blocks until the window is closed.
pub fn show_settings_window(config: &Config) -> Result<(), AppError> {
    unsafe {
        let hmod = GetModuleHandleW(None)
            .map_err(|e| AppError::ConfigError(format!("GetModuleHandle: {e}")))?;
        let hinst = HINSTANCE(hmod.0);

        let class_w = w(CLASS_NAME);
        let bg_brush = solid(BG_R, BG_G, BG_B);

        let wc = WNDCLASSEXW {
            cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst,
            hbrBackground: bg_brush,
            lpszClassName: PCWSTR(class_w.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let title_w = w("Paste Link Downloader — Settings");
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, 510, 490,
            None, None, hinst,
            // Pass config ptr as lpCreateParams so WM_CREATE can read it
            // before SetWindowLongPtrW is called.
            Some(config as *const Config as *const std::ffi::c_void),
        ).map_err(|e| AppError::ConfigError(format!("CreateWindowExW: {e}")))?;

        // USERDATA already set in WM_CREATE; this line is kept only as a safety
        // net in case the class is reused across calls.
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, config as *const Config as isize);

        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        Ok(())
    }
}
