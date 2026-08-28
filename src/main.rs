// src/main.rs
// Entry point for Paste Link Downloader.
#![windows_subsystem = "windows"]
//
// When launched from the Explorer context menu:
//   paste-link-downloader.exe "D:\Videos"
//
// The application:
//   1. Shows a native Win32 progress window (GUI).
//   2. Initialises logging.
//   3. Reads config.
//   4. Reads clipboard.
//   5. Validates URL and detects platform.
//   6. Runs yt-dlp (then gallery-dl fallback), streaming progress to the window.
//   7. Shows Open-Folder / error state in the window.


mod cli;
mod clipboard;
mod config;
mod context_menu;
mod download_gui;
mod downloader;
mod error;
mod logging;
mod notification;
mod platform;
mod progress;
mod settings_gui;
mod advanced_download_gui;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cli::{parse_args, print_usage, Command};
use config::Config;
use downloader::{download, download_images, run_diagnostics, DownloadOptions, ImageDownloadOptions};
use error::AppError;
use notification::{notify_cancelled, notify_error, notify_success};
use platform::validate_and_detect;
use progress::ProgressEvent;
use tracing::{error, info, warn};

fn main() {
    let command = parse_args();

    // Allocate a visible console only for CLI/diagnostic commands.
    // Download uses its own GUI window — no console needed.
    match &command {
        Command::DownloadImages { .. } | Command::Diagnostics | Command::Usage | Command::Version => {
            alloc_console();
        }
        Command::Download { .. } => {} // GUI window — no console
        Command::Settings => {}        // GUI window — no console
        Command::Install | Command::Uninstall => {}
    }

    // Load config (ignore errors — use defaults)
    let config = Config::load().unwrap_or_default();

    // Initialise logging
    if let Err(e) = logging::init(&config.log_level) {
        eprintln!("Warning: could not initialise logging: {e}");
    }

    let result = match command {
        Command::Version => {
            println!("paste-link-downloader {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }

        Command::Usage => {
            print_usage();
            Ok(())
        }

        Command::Diagnostics => {
            run_diagnostics(&config);
            Ok(())
        }

        Command::Install => run_install(),

        Command::Uninstall => run_uninstall(),

        Command::Settings => settings_gui::show_settings_window(&config),

        Command::Download { directory } => run_download_gui(directory, config.clone()),

        Command::DownloadImages { directory } => run_download_images(directory, config.clone()),
    };

    if let Err(e) = result {
        error!("Fatal error: {e}");
        if config.notifications {
            notify_error("Paste Link Downloader — Error", &e.to_string());
        } else {
            eprintln!("Error: {e}");
        }
        std::process::exit(1);
    }
}

// ── Install ──────────────────────────────────────────────────────────────────

fn run_install() -> Result<(), AppError> {
    let exe_path = std::env::current_exe()?;

    // Determine install target directory
    let install_dir = install_dir()?;
    let installed_exe = install_dir.join("paste-link-downloader.exe");

    // Create install directory
    std::fs::create_dir_all(&install_dir)?;

    // Copy executable if not already running from install dir
    if exe_path != installed_exe {
        info!("Copying {:?} → {:?}", exe_path, installed_exe);
        std::fs::copy(&exe_path, &installed_exe)?;
    }

    // Copy bin/ directory (yt-dlp, ffmpeg, ffprobe) if present next to source exe
    let source_bin = exe_path
        .parent()
        .map(|p| p.join("bin"))
        .unwrap_or_default();

    if source_bin.exists() {
        let dest_bin = install_dir.join("bin");
        copy_dir_all(&source_bin, &dest_bin)?;
        info!("Copied bin/ to {:?}", dest_bin);
    } else {
        warn!("bin/ directory not found next to executable; skipping binary copy");
        warn!("Place yt-dlp.exe, ffmpeg.exe, ffprobe.exe in {:?} after install", install_dir.join("bin"));
    }

    // Register context menu
    context_menu::install(&installed_exe)?;

    // Write default config if missing
    if Config::config_path().map(|p| !p.exists()).unwrap_or(false) {
        let _ = Config::write_default();
    }

    println!("✓ Paste Link Downloader installed to:");
    println!("    {}", install_dir.display());
    println!();
    println!("✓ Explorer context menu registered.");
    println!("  Right-click empty space in any folder → \"Paste link\"");
    println!("  (tries yt-dlp first; falls back to gallery-dl for images)");
    println!();
    println!("Next steps:");
    println!(
        "  1. Ensure yt-dlp.exe, ffmpeg.exe, ffprobe.exe are in:\n     {}",
        install_dir.join("bin").display()
    );
    println!(
        "  2. Optionally place gallery-dl.exe in:\n     {}\n     (enables fallback image downloading)",
        install_dir.join("bin").display()
    );

    info!("Installation complete");
    Ok(())
}

// ── Uninstall ────────────────────────────────────────────────────────────────

fn run_uninstall() -> Result<(), AppError> {
    context_menu::uninstall()?;

    println!("✓ Explorer context menu removed.");
    println!();
    println!("Application files remain at:");
    if let Ok(dir) = install_dir() {
        println!("    {}", dir.display());
    }
    println!("Delete that folder manually to remove all files.");
    println!("Your downloaded videos are NOT deleted.");

    info!("Uninstall complete");
    Ok(())
}

// ── Download (GUI window) ─────────────────────────────────────────────────────

fn run_download_gui(directory: String, config: Config) -> Result<(), AppError> {
    info!("Download (GUI) command. Directory: {directory}");

    let output_dir = PathBuf::from(&directory);
    if !output_dir.exists() {
        return Err(AppError::InvalidDirectory(directory));
    }
    if !output_dir.is_dir() {
        return Err(AppError::InvalidDirectory(format!("{directory} is not a directory")));
    }

    let raw_url = clipboard::read_clipboard_text()?;
    info!("Clipboard URL: {raw_url}");

    let (url, platform) = validate_and_detect(&raw_url)?;

    let cancelled = Arc::new(AtomicBool::new(false));

    // Check if Shift is held — if so, show advanced download options GUI first.
    let advanced = if is_shift_held() {
        info!("Shift key detected — opening advanced download options");
        match advanced_download_gui::show_advanced_options(&url) {
            Some(opts) => Some(opts),
            None => {
                info!("Advanced options cancelled by user");
                return Ok(());
            }
        }
    } else {
        None
    };

    download_gui::run_download_window(&url, &platform, output_dir, config, cancelled, advanced)
}

// ── Download (console fallback, used by --download-images CLI) ────────────────

fn run_download(directory: String, config: Config) -> Result<(), AppError> {
    info!("Download command. Directory: {directory}");

    // Validate the supplied directory
    let output_dir = PathBuf::from(&directory);
    if !output_dir.exists() {
        return Err(AppError::InvalidDirectory(directory));
    }
    if !output_dir.is_dir() {
        return Err(AppError::InvalidDirectory(format!(
            "{directory} is not a directory"
        )));
    }

    // Read clipboard
    let raw_url = clipboard::read_clipboard_text()?;
    info!("Clipboard URL: {raw_url}");

    // Validate URL and detect platform
    let (url, platform) = validate_and_detect(&raw_url)?;

    info!("Platform: {platform}");

    match &platform {
        platform::Platform::Unsupported(host) => {
            warn!("Platform '{host}' not in supported list; handing to yt-dlp anyway");
            println!("⚠  Platform '{host}' is not in the explicit supported list.");
            println!("   yt-dlp will try to download it anyway.");
        }
        p => println!("Platform: {p}"),
    }

    println!();
    println!("URL:    {url}");
    println!("Saving: {}", output_dir.display());
    println!();

    // Cancellation flag — future GUI can set this from a Cancel button
    let cancelled = Arc::new(AtomicBool::new(false));

    // Ctrl+C handler sets the cancellation flag
    let cancelled_ctrlc = cancelled.clone();
    let _ = ctrlc_handler(cancelled_ctrlc);

    // Progress callback — prints to the allocated console
    let on_progress: downloader::ProgressCallback = Box::new(move |event| {
        match &event {
            ProgressEvent::Percent(p) => {
                print!("\r  {p:.1}%                    ");
                let _ = flush_stdout();
            }
            ProgressEvent::Speed(summary) => {
                print!("\r  {summary}");
                let _ = flush_stdout();
            }
            ProgressEvent::Complete => {
                println!("\r  100.0% — Done!              ");
            }
            ProgressEvent::Merging(msg) => {
                println!("\n  Merging: {msg}");
            }
            ProgressEvent::Warning(w) => {
                println!("\n⚠  Warning: {w}");
            }
            ProgressEvent::Error(e) => {
                println!("\n✗  Error: {e}");
            }
            ProgressEvent::Other(_) | ProgressEvent::Eta(_) => {}
        }
    });

    let opts = DownloadOptions {
        url: url.to_string(),
        output_dir: output_dir.clone(),
        format: config.preferred_format.clone(),
        advanced: None,
    };

    println!("Downloading…");
    let result = download(&opts, &config, cancelled.clone(), on_progress);

    println!();

    match result {
        Ok(()) => {
            let msg = format!("Video saved to:\n{}", output_dir.display());
            println!("✓ {msg}");
            if config.notifications {
                notify_success("Download complete", &msg);
            }
            info!("Download complete. Output dir: {:?}", output_dir);
            Ok(())
        }
        Err(AppError::Cancelled) => {
            println!("⚠  Download cancelled.");
            if config.notifications {
                notify_cancelled();
            }
            info!("Download cancelled by user");
            Ok(()) // Not a fatal error
        }
        // yt-dlp failed — try gallery-dl as fallback (handles image galleries,
        // Pinterest boards, Instagram posts, Pixiv, Reddit, etc.)
        Err(yt_err) => {
            warn!("yt-dlp failed ({yt_err}); trying gallery-dl fallback…");
            println!("⚠  yt-dlp failed — trying gallery-dl…");
            println!();

            // Check gallery-dl is available before attempting
            match downloader::resolve_gallery_dl(&config) {
                Err(_) => {
                    // gallery-dl not installed — surface the original yt-dlp error
                    Err(yt_err)
                }
                Ok(_) => {
                    let img_opts = ImageDownloadOptions {
                        url: url.to_string(),
                        output_dir: output_dir.clone(),
                    };

                    let cancelled2 = Arc::new(AtomicBool::new(false));
                    let on_img_progress: downloader::ProgressCallback = Box::new(move |event| {
                        if let progress::ProgressEvent::Other(line) = event {
                            println!("  {line}");
                        }
                    });

                    println!("Downloading images…");
                    let img_result =
                        download_images(&img_opts, &config, cancelled2, on_img_progress);
                    println!();

                    match img_result {
                        Ok(()) => {
                            let msg = format!("Files saved to:\n{}", output_dir.display());
                            println!("✓ {msg}");
                            if config.notifications {
                                notify_success("Download complete", &msg);
                            }
                            info!("gallery-dl fallback complete. Output dir: {:?}", output_dir);
                            Ok(())
                        }
                        Err(AppError::Cancelled) => {
                            println!("⚠  Download cancelled.");
                            if config.notifications { notify_cancelled(); }
                            Ok(())
                        }
                        Err(img_err) => {
                            // Both failed — report yt-dlp error as primary, log gallery-dl
                            error!("gallery-dl also failed: {img_err}");
                            Err(yt_err)
                        }
                    }
                }
            }
        }
    }
}

// ── Download Images ───────────────────────────────────────────────────────────

fn run_download_images(directory: String, config: Config) -> Result<(), AppError> {
    info!("DownloadImages command. Directory: {directory}");

    let output_dir = PathBuf::from(&directory);
    if !output_dir.exists() {
        return Err(AppError::InvalidDirectory(directory));
    }
    if !output_dir.is_dir() {
        return Err(AppError::InvalidDirectory(format!(
            "{directory} is not a directory"
        )));
    }

    // Read clipboard
    let raw_url = clipboard::read_clipboard_text()?;
    info!("Clipboard URL: {raw_url}");

    // Validate URL (gallery-dl accepts any URL; we still validate scheme)
    let (url, platform) = validate_and_detect(&raw_url)?;
    info!("Platform: {platform}");

    println!();
    println!("URL:    {url}");
    println!("Saving: {}", output_dir.display());
    println!();

    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_ctrlc = cancelled.clone();
    let _ = ctrlc_handler(cancelled_ctrlc);

    // Progress callback — print each gallery-dl output line
    let on_progress: downloader::ProgressCallback = Box::new(move |event| {
        if let progress::ProgressEvent::Other(line) = event {
            println!("  {line}");
        }
    });

    let opts = ImageDownloadOptions {
        url: url.to_string(),
        output_dir: output_dir.clone(),
    };

    println!("Downloading images…");
    let result = download_images(&opts, &config, cancelled.clone(), on_progress);

    println!();

    match result {
        Ok(()) => {
            let msg = format!("Images saved to:\n{}", output_dir.display());
            println!("✓ {msg}");
            if config.notifications {
                notify_success("Images downloaded", &msg);
            }
            info!("Image download complete. Output dir: {:?}", output_dir);
            Ok(())
        }
        Err(AppError::Cancelled) => {
            println!("⚠  Download cancelled.");
            if config.notifications {
                notify_cancelled();
            }
            info!("Image download cancelled by user");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Returns the per-user installation directory:
///   %LOCALAPPDATA%\PasteLinkDownloader\
fn install_dir() -> Result<PathBuf, AppError> {
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("PasteLinkDownloader"))
        .ok_or_else(|| AppError::ConfigError("LOCALAPPDATA not set".to_string()))
}

/// Recursively copies a directory tree.
fn copy_dir_all(src: &PathBuf, dst: &PathBuf) -> Result<(), AppError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

/// Allocate a console window (no-op if one already exists).
#[cfg(target_os = "windows")]
fn alloc_console() {
    use windows::Win32::System::Console::AllocConsole;
    // SAFETY: AllocConsole is safe to call from any thread; fails silently
    // if a console already exists.
    unsafe { let _ = AllocConsole(); }
}

#[cfg(not(target_os = "windows"))]
fn alloc_console() {}

/// Check if the Shift key is currently held down.
///
/// Uses `GetAsyncKeyState(VK_SHIFT)` — returns `true` if Shift is pressed
/// at the instant of the call. When the user Shift+clicks "Paste link" in
/// Explorer, Shift is typically still held when the process starts (~100ms).
#[cfg(target_os = "windows")]
fn is_shift_held() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SHIFT};
    // SAFETY: GetAsyncKeyState is safe to call at any time.
    let state = unsafe { GetAsyncKeyState(VK_SHIFT.0 as i32) };
    // High bit set = key is currently down.
    (state as u16 & 0x8000) != 0
}

#[cfg(not(target_os = "windows"))]
fn is_shift_held() -> bool {
    false
}

/// Register Ctrl+C handler that sets the cancellation flag.
fn ctrlc_handler(cancelled: Arc<AtomicBool>) -> Result<(), ()> {
    // Use a simple thread-based approach compatible with any Rust target.
    // We spawn a thread that blocks reading from stdin; Ctrl+C on Windows
    // raises a console event, which we intercept via SetConsoleCtrlHandler.
    set_console_ctrl_handler(cancelled)
}

#[cfg(target_os = "windows")]
fn set_console_ctrl_handler(cancelled: Arc<AtomicBool>) -> Result<(), ()> {
    use std::sync::OnceLock;
    use windows::Win32::System::Console::{SetConsoleCtrlHandler, CTRL_C_EVENT};

    // Store the Arc in a static so the handler closure can access it.
    static CANCELLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    let _ = CANCELLED.set(cancelled);

    unsafe extern "system" fn handler(ctrl_type: u32) -> windows::Win32::Foundation::BOOL {
        if ctrl_type == CTRL_C_EVENT {
            if let Some(flag) = CANCELLED.get() {
                flag.store(true, Ordering::Relaxed);
            }
            windows::Win32::Foundation::TRUE
        } else {
            windows::Win32::Foundation::FALSE
        }
    }

    // SAFETY: handler is a valid extern "system" fn.
    unsafe {
        let _ = SetConsoleCtrlHandler(Some(handler), true);
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_console_ctrl_handler(_cancelled: Arc<AtomicBool>) -> Result<(), ()> {
    Ok(())
}

fn flush_stdout() -> std::io::Result<()> {
    use std::io::Write;
    std::io::stdout().flush()
}
