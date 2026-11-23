// tests/cli_tests.rs
// Comprehensive CLI argument parsing tests.

use gfx_tools::cli::{parse_from, Command};

#[test]
fn cli_positional_download() {
    let cmd = parse_from(["D:\\Videos"]);
    assert_eq!(
        cmd,
        Command::Download {
            directory: "D:\\Videos".to_string()
        }
    );
}

#[test]
fn cli_explicit_download() {
    let cmd = parse_from(["--download", "C:\\Users\\User\\Downloads"]);
    assert_eq!(
        cmd,
        Command::Download {
            directory: "C:\\Users\\User\\Downloads".to_string()
        }
    );
}

#[test]
fn cli_download_images() {
    let cmd = parse_from(["--download-images", "D:\\Pictures"]);
    assert_eq!(
        cmd,
        Command::DownloadImages {
            directory: "D:\\Pictures".to_string()
        }
    );
}

#[test]
fn cli_convert_compatible() {
    let cmd = parse_from(["--convert-compatible", "D:\\Videos\\Raw"]);
    assert_eq!(
        cmd,
        Command::ConvertCompatible {
            directory: "D:\\Videos\\Raw".to_string()
        }
    );
}

#[test]
fn cli_compress() {
    let cmd = parse_from(["--compress", "D:\\Videos\\Archive"]);
    assert_eq!(
        cmd,
        Command::Compress {
            directory: "D:\\Videos\\Archive".to_string()
        }
    );
}

#[test]
fn cli_install_flags() {
    assert_eq!(parse_from(["--install"]), Command::Install);
    assert_eq!(parse_from(["install"]), Command::Install);
}

#[test]
fn cli_uninstall_flags() {
    assert_eq!(parse_from(["--uninstall"]), Command::Uninstall);
    assert_eq!(parse_from(["uninstall"]), Command::Uninstall);
}

#[test]
fn cli_diagnostics_flags() {
    assert_eq!(parse_from(["--diagnostics"]), Command::Diagnostics);
    assert_eq!(parse_from(["diagnostics"]), Command::Diagnostics);
}

#[test]
fn cli_settings_flags() {
    assert_eq!(parse_from(["--settings"]), Command::Settings);
    assert_eq!(parse_from(["settings"]), Command::Settings);
}

#[test]
fn cli_version_flags() {
    assert_eq!(parse_from(["--version"]), Command::Version);
    assert_eq!(parse_from(["-V"]), Command::Version);
}

#[test]
fn cli_empty_args_gives_usage() {
    let empty: [&str; 0] = [];
    assert_eq!(parse_from(empty), Command::Usage);
}

#[test]
fn cli_unknown_flag_gives_usage() {
    assert_eq!(parse_from(["--unknown"]), Command::Usage);
    assert_eq!(parse_from(["--download"]), Command::Usage); // Missing dir arg
    assert_eq!(parse_from(["--compress"]), Command::Usage); // Missing dir arg
    assert_eq!(parse_from(["--install", "extra_arg"]), Command::Usage);
}
