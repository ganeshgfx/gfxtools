// src/lib.rs
// Public API surface for integration tests.
// The binary crate (main.rs) uses modules directly via `mod`.
// Integration tests (tests/*.rs) import through this lib crate.

pub mod clipboard;
pub mod config;
pub mod context_menu;
pub mod downloader;
pub mod error;
pub mod logging;
pub mod notification;
pub mod platform;
pub mod postprocess;
pub mod postprocess_gui;
pub mod progress;
pub mod cli;
pub mod advanced_download_gui;
