// src/cli.rs
// Command-line argument parsing and dispatch.
//
// Supported invocations:
//
//   paste-link-downloader.exe "D:\Videos"        → download (Explorer context-menu)
//   paste-link-downloader.exe --download "D:\Videos"  → explicit download
//   paste-link-downloader.exe --install          → register context-menu entry
//   paste-link-downloader.exe --uninstall        → remove context-menu entry
//   paste-link-downloader.exe --diagnostics      → check yt-dlp / ffmpeg
//   paste-link-downloader.exe --version          → print version

/// Parsed command from CLI arguments.
#[derive(Debug)]
pub enum Command {
    /// Download video from clipboard into the given directory.
    Download { directory: String },
    /// Register the Explorer context-menu entry.
    Install,
    /// Remove the Explorer context-menu entry.
    Uninstall,
    /// Print binary resolution info.
    Diagnostics,
    /// Open the settings GUI.
    Settings,
    /// Print version string.
    Version,
    /// No valid args supplied; print usage.
    Usage,
}

/// Parse `std::env::args()` into a `Command`.
pub fn parse_args() -> Command {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.as_slice() {
        // Explicit flags
        [flag] if flag == "--install" => Command::Install,
        [flag] if flag == "--uninstall" => Command::Uninstall,
        [flag] if flag == "--diagnostics" => Command::Diagnostics,
        [flag] if flag == "--settings" => Command::Settings,
        [flag] if flag == "--version" || flag == "-V" => Command::Version,

        // Explicit --download <dir>
        [flag, dir] if flag == "--download" => Command::Download {
            directory: dir.clone(),
        },

        // Bare positional argument → directory supplied by Explorer (%V)
        [dir] if !dir.starts_with("--") => Command::Download {
            directory: dir.clone(),
        },

        // Nothing
        [] => Command::Usage,

        // Unrecognised
        _ => {
            eprintln!("Unknown arguments: {:?}", args);
            Command::Usage
        }
    }
}

/// Print usage help.
pub fn print_usage() {
    println!(
        r#"Paste Link Downloader {version}

USAGE:
  paste-link-downloader.exe <directory>          Download clipboard URL into <directory>
  paste-link-downloader.exe --install            Register Explorer context-menu
  paste-link-downloader.exe --uninstall          Remove  Explorer context-menu
  paste-link-downloader.exe --diagnostics        Check yt-dlp / FFmpeg installation
  paste-link-downloader.exe --settings           Open settings GUI
  paste-link-downloader.exe --version            Print version

EXAMPLES:
  paste-link-downloader.exe "D:\Videos"
  paste-link-downloader.exe --install
  paste-link-downloader.exe --uninstall
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
