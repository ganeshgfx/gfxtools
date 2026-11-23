// tests/config_tests.rs
// Configuration defaults, serialization and deserialization tests.

use gfx_tools::config::Config;

#[test]
fn default_config_has_sensible_values() {
    let cfg = Config::default();
    assert_eq!(cfg.yt_dlp_path, "");
    assert_eq!(cfg.ffmpeg_dir, "");
    assert_eq!(cfg.gallery_dl_path, "");
    assert!(cfg.notifications);
    assert_eq!(cfg.preferred_format, "mp4");
    assert_eq!(cfg.log_level, "info");
    assert_eq!(cfg.cookies_from_browser, "");
    assert_eq!(cfg.cookies_file, "");
}

#[test]
fn deserialize_empty_toml_uses_defaults() {
    let toml_str = "";
    let cfg: Config = toml::from_str(toml_str).expect("deserialize empty toml");
    assert!(cfg.notifications);
    assert_eq!(cfg.preferred_format, "mp4");
    assert_eq!(cfg.log_level, "info");
    assert_eq!(cfg.yt_dlp_path, "");
}

#[test]
fn deserialize_partial_toml() {
    let toml_str = r#"
        preferred_format = "mkv"
        notifications = false
        log_level = "debug"
    "#;
    let cfg: Config = toml::from_str(toml_str).expect("deserialize partial toml");
    assert!(!cfg.notifications);
    assert_eq!(cfg.preferred_format, "mkv");
    assert_eq!(cfg.log_level, "debug");
    assert_eq!(cfg.yt_dlp_path, "");
}

#[test]
fn deserialize_full_toml() {
    let toml_str = r#"
        yt_dlp_path = "C:\\tools\\yt-dlp.exe"
        ffmpeg_dir = "C:\\tools\\ffmpeg\\bin"
        gallery_dl_path = "C:\\tools\\gallery-dl.exe"
        notifications = false
        preferred_format = "webm"
        log_level = "trace"
        cookies_from_browser = "firefox"
        cookies_file = "C:\\path\\cookies.txt"
    "#;
    let cfg: Config = toml::from_str(toml_str).expect("deserialize full toml");
    assert_eq!(cfg.yt_dlp_path, "C:\\tools\\yt-dlp.exe");
    assert_eq!(cfg.ffmpeg_dir, "C:\\tools\\ffmpeg\\bin");
    assert_eq!(cfg.gallery_dl_path, "C:\\tools\\gallery-dl.exe");
    assert!(!cfg.notifications);
    assert_eq!(cfg.preferred_format, "webm");
    assert_eq!(cfg.log_level, "trace");
    assert_eq!(cfg.cookies_from_browser, "firefox");
    assert_eq!(cfg.cookies_file, "C:\\path\\cookies.txt");
}

#[test]
fn serialize_and_roundtrip_toml() {
    let original = Config {
        yt_dlp_path: "D:\\bin\\yt-dlp.exe".into(),
        ffmpeg_dir: "D:\\bin".into(),
        gallery_dl_path: "D:\\bin\\gallery-dl.exe".into(),
        notifications: true,
        preferred_format: "mp4".into(),
        log_level: "warn".into(),
        cookies_from_browser: "chrome".into(),
        cookies_file: "D:\\cookies.txt".into(),
    };

    let serialized = toml::to_string_pretty(&original).expect("serialize config");
    let deserialized: Config = toml::from_str(&serialized).expect("deserialize serialized config");

    assert_eq!(original.yt_dlp_path, deserialized.yt_dlp_path);
    assert_eq!(original.ffmpeg_dir, deserialized.ffmpeg_dir);
    assert_eq!(original.gallery_dl_path, deserialized.gallery_dl_path);
    assert_eq!(original.notifications, deserialized.notifications);
    assert_eq!(original.preferred_format, deserialized.preferred_format);
    assert_eq!(original.log_level, deserialized.log_level);
    assert_eq!(original.cookies_from_browser, deserialized.cookies_from_browser);
    assert_eq!(original.cookies_file, deserialized.cookies_file);
}
