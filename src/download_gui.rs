// src/download_gui.rs
// Native Win32 download-progress window.
//
// Replaces the bare console for context-menu downloads with a sleek UI:
//   * Platform / URL / destination labels
//   * Smooth progress bar (yt-dlp %) or marquee (gallery-dl / starting)
//   * Live status text (colour-coded: default / green / red)
//   * Cancel -> Close button, Open Folder button (enabled on success)
//
// Architecture:
//   GUI thread  -> Win32 message loop + WM_TIMER polling every 80 ms
//   Worker thread -> download(); updates Arc<Mutex<GuiState>>

#![allow(non_snake_case, clippy::cast_sign_loss, clippy::cast_possible_truncation)]

use crate::config::Config;
use crate::downloader::{
    download, download_images, resolve_gallery_dl, DownloadOptions, ImageDownloadOptions,
};
use crate::error::AppError;
use crate::platform::Platform;
use crate::progress::ProgressEvent;

use std::ffi::OsStr;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tracing::warn;
use url::Url;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, HDC, SetBkColor, SetTextColor, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_PROGRESS_CLASS, INITCOMMONCONTROLSEX,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CS_HREDRAW, CS_VREDRAW,
    CreateWindowExW, DefWindowProcW, DispatchMessageW,
    GetDlgItem, GetMessageW, GetWindowLongPtrW,
    GWLP_USERDATA, KillTimer, MSG, PostQuitMessage,
    RegisterClassExW, SendMessageW, SetTimer, SetWindowLongPtrW,
    SetWindowTextW, ShowWindow, SW_SHOWNORMAL, TranslateMessage,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_ENABLE,
    WM_DESTROY, WM_TIMER, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN,
    WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_EX_APPWINDOW,
    WS_EX_CLIENTEDGE, WINDOW_STYLE, HMENU,
};

// Window class name
const CLASS: &str = "PLD_DownloadWnd";

// Control IDs
const ID_LBL_PLATFORM: i32 = 201;
const ID_LBL_URL: i32      = 202;
const ID_LBL_DIR: i32      = 203;
const ID_PROGRESS: i32     = 204;
const ID_LBL_STATUS: i32   = 205;
const ID_BTN_CANCEL: usize = 206;
const ID_BTN_OPEN: usize   = 207;

// Timer
const TIMER_ID: usize = 42;
const TIMER_MS: u32   = 80;

// Progress bar raw Win32 messages / styles
const PBS_SMOOTH:      u32 = 0x01;
const PBS_MARQUEE:     u32 = 0x08;
const PBM_SETRANGE32:  u32 = 0x0406;
const PBM_SETPOS:      u32 = 0x0402;
const PBM_SETBARCOLOR: u32 = 0x0409;
const PBM_SETMARQUEE:  u32 = 0x040A;

// Catppuccin Mocha palette
const BG:    (u8, u8, u8) = (30,  30,  46);
const TX:    (u8, u8, u8) = (205, 214, 244);
const GREEN: (u8, u8, u8) = (166, 227, 161);
const RED:   (u8, u8, u8) = (243, 139, 168);
const BLUE:  (u8, u8, u8) = (137, 180, 250);

// Download phase
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Phase {
    Starting,
    YtDlp,
    GalleryDl,
    Done,
    Failed(String),
    Cancelled,
}

// Shared state (GUI thread reads, worker thread writes)
pub(crate) struct GuiState {
    pub phase:        Phase,
    pub progress_pct: f64,
    pub status_text:  String,
}

// Per-window data in GWLP_USERDATA (GUI thread only)
struct WindowData {
    state:        Arc<Mutex<GuiState>>,
    cancelled:    Arc<AtomicBool>,
    output_dir:   PathBuf,
    marquee_on:   bool,
    finished:     bool,
    status_color: (u8, u8, u8),
}

fn cr((r, g, b): (u8, u8, u8)) -> COLORREF {
    COLORREF(r as u32 | ((g as u32) << 8) | ((b as u32) << 16))
}
fn solid(c: (u8, u8, u8)) -> HBRUSH { unsafe { CreateSolidBrush(cr(c)) } }
fn w(s: &str) -> Vec<u16> { OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect() }
fn id_menu(id: usize) -> HMENU { HMENU(id as *mut _) }
fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() }
    else { format!("{}...", s.chars().take(n).collect::<String>()) }
}

unsafe fn get_data<'a>(hwnd: HWND) -> Option<&'a mut WindowData> {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if p == 0 { None } else { Some(&mut *(p as *mut WindowData)) }
}
unsafe fn dlg(hwnd: HWND, id: i32) -> HWND { GetDlgItem(hwnd, id).unwrap_or_default() }
unsafe fn set_lbl(hwnd: HWND, id: i32, s: &str) {
    let ws = w(s); let _ = SetWindowTextW(dlg(hwnd, id), PCWSTR(ws.as_ptr()));
}
unsafe fn pb_pos(hwnd: HWND, pct: f64) {
    let pb = dlg(hwnd, ID_PROGRESS);
    SendMessageW(pb, PBM_SETPOS, WPARAM((pct.clamp(0.0,100.0)*10.0) as usize), LPARAM(0));
}
unsafe fn pb_marquee(hwnd: HWND, on: bool) {
    let pb = dlg(hwnd, ID_PROGRESS);
    SendMessageW(pb, PBM_SETMARQUEE, WPARAM(usize::from(on)), LPARAM(40));
}

unsafe fn update_ui(hwnd: HWND, data: &mut WindowData) {
    let (phase, pct, text) = {
        let s = data.state.lock().unwrap();
        (s.phase.clone(), s.progress_pct, s.status_text.clone())
    };
    set_lbl(hwnd, ID_LBL_STATUS, &text);

    match &phase {
        Phase::Starting | Phase::GalleryDl => {
            if !data.marquee_on { pb_marquee(hwnd, true); data.marquee_on = true; }
            data.status_color = TX;
        }
        Phase::YtDlp => {
            if data.marquee_on { pb_marquee(hwnd, false); data.marquee_on = false; }
            pb_pos(hwnd, pct);
            data.status_color = TX;
        }
        Phase::Done => {
            if data.marquee_on { pb_marquee(hwnd, false); data.marquee_on = false; }
            pb_pos(hwnd, 100.0);
            data.status_color = GREEN;
            // Enable Open Folder button and rename Cancel -> Close
            // WM_ENABLE (0x000A) with WPARAM(1) enables the control
            SendMessageW(dlg(hwnd, ID_BTN_OPEN as i32), WM_ENABLE, WPARAM(1), LPARAM(0));
            let cw = w("Close");
            let _ = SetWindowTextW(dlg(hwnd, ID_BTN_CANCEL as i32), PCWSTR(cw.as_ptr()));
            data.finished = true;
        }
        Phase::Failed(_) => {
            if data.marquee_on { pb_marquee(hwnd, false); data.marquee_on = false; }
            data.status_color = RED;
            let cw = w("Close");
            let _ = SetWindowTextW(dlg(hwnd, ID_BTN_CANCEL as i32), PCWSTR(cw.as_ptr()));
            data.finished = true;
        }
        Phase::Cancelled => { data.status_color = TX; data.finished = true; }
    }

    use windows::Win32::Graphics::Gdi::InvalidateRect;
    InvalidateRect(dlg(hwnd, ID_LBL_STATUS), None, true);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lp.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            create_controls(hwnd);
            SetTimer(hwnd, TIMER_ID, TIMER_MS, None);
            LRESULT(0)
        }
        WM_TIMER => {
            if wp.0 == TIMER_ID {
                if let Some(d) = get_data(hwnd) {
                    if !d.finished {
                        update_ui(hwnd, d);
                        if d.finished { KillTimer(hwnd, TIMER_ID); }
                    }
                }
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wp.0 as *mut _);
            let ctrl = HWND(lp.0 as *mut _);
            let color = if ctrl == dlg(hwnd, ID_LBL_STATUS) {
                get_data(hwnd).map(|d| d.status_color).unwrap_or(TX)
            } else { TX };
            SetTextColor(hdc, cr(color));
            SetBkColor(hdc, cr(BG));
            LRESULT(solid(BG).0 as isize)
        }
        WM_COMMAND => {
            match wp.0 & 0xFFFF {
                x if x == ID_BTN_CANCEL => {
                    if let Some(d) = get_data(hwnd) {
                        if d.finished { PostQuitMessage(0); }
                        else {
                            d.cancelled.store(true, Ordering::Relaxed);
                            set_lbl(hwnd, ID_LBL_STATUS, "Cancelling...");
                        }
                    }
                }
                x if x == ID_BTN_OPEN => {
                    if let Some(d) = get_data(hwnd) {
                        let _ = std::process::Command::new("explorer").arg(&d.output_dir).spawn();
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if let Some(d) = get_data(hwnd) { d.cancelled.store(true, Ordering::Relaxed); }
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER_ID);
            let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if p != 0 {
                drop(Box::from_raw(p as *mut WindowData));
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

const WIN_W: i32  = 520;
const MARGIN: i32 = 14;
const LBL_H: i32  = 18;
const ROW_H: i32  = 26;
const BTN_H: i32  = 28;
const BTN_W: i32  = 120;

unsafe fn create_controls(hwnd: HWND) {
    let cw = WIN_W - MARGIN * 2;
    let mut y = MARGIN;

    macro_rules! lbl {
        ($id:expr, $text:expr) => {{
            let cn = w("STATIC"); let tw = w($text);
            let _ = CreateWindowExW(
                Default::default(), PCWSTR(cn.as_ptr()), PCWSTR(tw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
                MARGIN, y, cw, LBL_H,
                hwnd, HMENU($id as isize as *mut _), HINSTANCE(null_mut()), None,
            );
            y += ROW_H;
        }};
    }

    lbl!(ID_LBL_PLATFORM, "Platform: ...");
    lbl!(ID_LBL_URL,      "URL: ...");
    lbl!(ID_LBL_DIR,      "Save: ...");
    y += 4;

    // Progress bar
    let pb_cls = w("msctls_progress32");
    let pb = CreateWindowExW(
        WS_EX_CLIENTEDGE, PCWSTR(pb_cls.as_ptr()), PCWSTR::null(),
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | PBS_SMOOTH | PBS_MARQUEE),
        MARGIN, y, cw, 22,
        hwnd, id_menu(ID_PROGRESS as usize), HINSTANCE(null_mut()), None,
    ).unwrap_or_default();
    SendMessageW(pb, PBM_SETRANGE32, WPARAM(0), LPARAM(1000));
    SendMessageW(pb, PBM_SETBARCOLOR, WPARAM(0), LPARAM(cr(BLUE).0 as isize));
    SendMessageW(pb, PBM_SETMARQUEE, WPARAM(1), LPARAM(40));
    y += 22 + 10;

    // Status label
    {
        let cn = w("STATIC"); let tw = w("Starting...");
        let _ = CreateWindowExW(
            Default::default(), PCWSTR(cn.as_ptr()), PCWSTR(tw.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0),
            MARGIN, y, cw, LBL_H,
            hwnd, HMENU(ID_LBL_STATUS as isize as *mut _), HINSTANCE(null_mut()), None,
        );
        y += ROW_H + 8;
    }

    // Buttons
    macro_rules! btn {
        ($id:expr, $text:expr, $x:expr, $en:expr) => {{
            let cn = w("BUTTON"); let tw = w($text);
            let b = CreateWindowExW(
                Default::default(), PCWSTR(cn.as_ptr()), PCWSTR(tw.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32),
                $x, y, BTN_W, BTN_H,
                hwnd, id_menu($id), HINSTANCE(null_mut()), None,
            ).unwrap_or_default();
            if !$en {
                // Disable via WM_ENABLE
                SendMessageW(b, WM_ENABLE, WPARAM(0), LPARAM(0));
            }
        }};
    }
    btn!(ID_BTN_CANCEL, "Cancel",      MARGIN,               true);
    btn!(ID_BTN_OPEN,   "Open Folder", WIN_W - MARGIN - BTN_W, false);
}

fn worker(
    url: String,
    output_dir: PathBuf,
    config: Config,
    state: Arc<Mutex<GuiState>>,
    cancelled: Arc<AtomicBool>,
) {
    // Phase 1: yt-dlp
    {
        let mut s = state.lock().unwrap();
        s.phase = Phase::YtDlp;
        s.status_text = "Downloading via yt-dlp...".to_string();
    }

    let st2 = state.clone();
    let on_ytdlp: Box<dyn Fn(ProgressEvent) + Send + 'static> = Box::new(move |ev| {
        let mut s = st2.lock().unwrap();
        match ev {
            ProgressEvent::Percent(p)   => { s.progress_pct = p; s.status_text = format!("{p:.1}%"); }
            ProgressEvent::Speed(sum)   => { s.status_text = sum; }
            ProgressEvent::Complete     => { s.progress_pct = 100.0; s.status_text = "Finishing up...".to_string(); }
            ProgressEvent::Merging(m)   => { s.status_text = format!("Merging: {m}"); }
            ProgressEvent::Warning(w)   => { s.status_text = format!("Warning: {w}"); }
            ProgressEvent::Error(e)     => { s.status_text = format!("Error: {e}"); }
            _                           => {}
        }
    });

    let opts = DownloadOptions { url: url.clone(), output_dir: output_dir.clone(), format: config.preferred_format.clone() };
    match download(&opts, &config, cancelled.clone(), on_ytdlp) {
        Ok(()) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Done;
            s.progress_pct = 100.0;
            s.status_text = "Files saved successfully.".to_string();
            return;
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Cancelled;
            s.status_text = "Cancelled.".to_string();
            return;
        }
        Err(e) => { warn!("yt-dlp failed ({e}), trying gallery-dl..."); }
    }

    // Check gallery-dl available
    if resolve_gallery_dl(&config).is_err() {
        let mut s = state.lock().unwrap();
        s.phase = Phase::Failed("yt-dlp failed. Install gallery-dl for image support.".to_string());
        s.status_text = "Download failed.".to_string();
        return;
    }

    // Phase 2: gallery-dl
    {
        let mut s = state.lock().unwrap();
        s.phase = Phase::GalleryDl;
        s.status_text = "yt-dlp failed — trying gallery-dl...".to_string();
        s.progress_pct = 0.0;
    }

    let st3 = state.clone();
    let on_img: Box<dyn Fn(ProgressEvent) + Send + 'static> = Box::new(move |ev| {
        if let ProgressEvent::Other(line) = ev {
            if !line.is_empty() && !line.starts_with('#') {
                let mut s = st3.lock().unwrap();
                s.status_text = trunc(&line, 72);
            }
        }
    });

    let img_opts = ImageDownloadOptions { url, output_dir };
    match download_images(&img_opts, &config, cancelled, on_img) {
        Ok(()) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Done;
            s.status_text = "Files saved successfully.".to_string();
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Cancelled;
            s.status_text = "Cancelled.".to_string();
        }
        Err(e) => {
            let msg = e.to_string();
            let mut s = state.lock().unwrap();
            s.phase = Phase::Failed(msg.clone());
            s.status_text = format!("Error: {msg}");
        }
    }
}

/// Open the progress window and run the download. Blocks until user closes.
pub fn run_download_window(
    url: &Url,
    platform: &Platform,
    output_dir: PathBuf,
    config: Config,
    cancelled: Arc<AtomicBool>,
) -> Result<(), AppError> {
    let url_str = url.to_string();
    let plat_str = platform.to_string();

    let state = Arc::new(Mutex::new(GuiState {
        phase: Phase::Starting,
        progress_pct: 0.0,
        status_text: "Starting...".to_string(),
    }));

    // Spawn worker
    {
        let sw = state.clone(); let cw = cancelled.clone();
        let uw = url_str.clone(); let dw = output_dir.clone(); let cfg = config.clone();
        std::thread::spawn(move || worker(uw, dw, cfg, sw, cw));
    }

    unsafe {
        let icc = INITCOMMONCONTROLSEX {
            dwSize: mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_PROGRESS_CLASS,
        };
        InitCommonControlsEx(&icc);

        let hmod = GetModuleHandleW(None)
            .map_err(|e| AppError::ConfigError(format!("GetModuleHandle: {e}")))?;
        let hinst = HINSTANCE(hmod.0);
        let bg_brush = solid(BG);
        let class_w = w(CLASS);

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

        let data = Box::new(WindowData {
            state: state.clone(),
            cancelled: cancelled.clone(),
            output_dir: output_dir.clone(),
            marquee_on: true,
            finished: false,
            status_color: TX,
        });
        let data_ptr = Box::into_raw(data);

        let title_w = w("Paste Link Downloader");
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            PCWSTR(class_w.as_ptr()),
            PCWSTR(title_w.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
            CW_USEDEFAULT, CW_USEDEFAULT, WIN_W, 220,
            None, None, hinst,
            Some(data_ptr as *const std::ffi::c_void),
        ).map_err(|e| AppError::ConfigError(format!("CreateWindowExW: {e}")))?;

        // Populate labels
        set_lbl(hwnd, ID_LBL_PLATFORM, &format!("Platform: {plat_str}"));
        set_lbl(hwnd, ID_LBL_URL,      &format!("URL:  {}", trunc(&url_str, 68)));
        set_lbl(hwnd, ID_LBL_DIR,      &format!("Save: {}", trunc(&output_dir.display().to_string(), 68)));

        ShowWindow(hwnd, SW_SHOWNORMAL);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let final_phase = state.lock().unwrap().phase.clone();
        match final_phase {
            Phase::Failed(e) => Err(AppError::DownloadFailed(e)),
            _ => Ok(()),
        }
    }
}
