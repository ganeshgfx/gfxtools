// src/cli.rs
// Command-line argument parsing and dispatch.
//
// Supported invocations:
//
//   gfx-tools.exe "D:\Videos"        → download (Explorer context-menu)
//   gfx-tools.exe --download "D:\Videos"  → explicit download
//   gfx-tools.exe --install          → register context-menu entry
//   gfx-tools.exe --uninstall        → remove context-menu entry
//   gfx-tools.exe --diagnostics      → check yt-dlp / ffmpeg
//   gfx-tools.exe --version          → print version

/// Parsed command from CLI arguments.
#[derive(Debug)]
pub enum Command {
    /// Download video from clipboard into the given directory.
    Download { directory: String },
    /// Download images from clipboard URL into the given directory via gallery-dl.
    DownloadImages { directory: String },
    /// Convert videos in directory to Premiere Pro compatible format.
    ConvertCompatible { directory: String },
    /// Compress videos in directory for efficient storage.
    Compress { directory: String },
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
        [flag] if flag == "--install" || flag == "install" => Command::Install,
        [flag] if flag == "--uninstall" || flag == "uninstall" => Command::Uninstall,
        [flag] if flag == "--diagnostics" || flag == "diagnostics" => Command::Diagnostics,
        [flag] if flag == "--settings" || flag == "settings" => Command::Settings,
        [flag] if flag == "--version" || flag == "-V" => Command::Version,

        // Explicit --download <dir>
        [flag, dir] if flag == "--download" => Command::Download {
            directory: dir.clone(),
        },

        // Explicit --download-images <dir>  (fired by gallery-dl context menu)
        [flag, dir] if flag == "--download-images" => Command::DownloadImages {
            directory: dir.clone(),
        },

        // Explicit --convert-compatible <dir>  (fired by extended context menu)
        [flag, dir] if flag == "--convert-compatible" => Command::ConvertCompatible {
            directory: dir.clone(),
        },

        // Explicit --compress <dir>  (fired by extended context menu)
        [flag, dir] if flag == "--compress" => Command::Compress {
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
        r#"GFX Tools {version}

USAGE:
  gfx-tools.exe <directory>                 Download clipboard URL (video) into <directory>
  gfx-tools.exe --download-images <dir>     Download clipboard URL (images) via gallery-dl
  gfx-tools.exe --convert-compatible <dir>   Convert videos to Premiere Pro compatible
  gfx-tools.exe --compress <dir>             Compress videos for efficient storage
  gfx-tools.exe --install                    Register Explorer context-menu
  gfx-tools.exe --uninstall                  Remove  Explorer context-menu
  gfx-tools.exe --diagnostics                Check yt-dlp / FFmpeg / gallery-dl installation
  gfx-tools.exe --settings                   Open settings GUI
  gfx-tools.exe --version                    Print version

EXAMPLES:
  gfx-tools.exe "D:\Videos"
  gfx-tools.exe --download-images "D:\Pictures"
  gfx-tools.exe --convert-compatible "D:\Videos"
  gfx-tools.exe --compress "D:\Videos"
  gfx-tools.exe --install
  gfx-tools.exe --uninstall
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
