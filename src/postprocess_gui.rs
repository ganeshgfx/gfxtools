// src/postprocess_gui.rs
// eframe/egui progress window for post-processing operations
// (Convert to Compatible, Compress).
//
// Shows: operation name, current file, per-file progress, overall progress,
// cancel button, and open-folder button when done.

use crate::config::Config;
use crate::error::AppError;
use crate::postprocess::{self, FileResult, PostprocessOp};

use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Shared state between the worker thread and the GUI.
struct PostprocessState {
    /// Current phase of the operation.
    phase: PostprocessPhase,
    /// Index of the file currently being processed (0-based).
    current_idx: usize,
    /// Total number of files.
    total_files: usize,
    /// Name of the file currently being processed.
    current_file: String,
    /// Progress percentage for the current file (0.0–100.0, negative = indeterminate).
    file_progress: f64,
    /// Summary text shown below the progress bar.
    status_text: String,
    /// Results per file (populated as processing completes).
    results: Vec<(String, FileResult)>,
}

#[derive(Debug, Clone, PartialEq)]
enum PostprocessPhase {
    Starting,
    Processing,
    Done,
    Failed(String),
    Cancelled,
}

/// Launch the post-processing GUI window (blocking).
///
/// Returns `Ok(())` on success or partial completion, `Err` on fatal errors.
pub fn run_postprocess_window(
    op: PostprocessOp,
    directory: PathBuf,
    config: Config,
) -> Result<(), AppError> {
    let cancelled = Arc::new(AtomicBool::new(false));

    let state = Arc::new(Mutex::new(PostprocessState {
        phase: PostprocessPhase::Starting,
        current_idx: 0,
        total_files: 0,
        current_file: String::new(),
        file_progress: -1.0,
        status_text: "Scanning for video files...".to_string(),
        results: Vec::new(),
    }));

    let title = format!("Paste Link Downloader — {}", op);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 300.0])
            .with_title(&title),
        ..Default::default()
    };

    let app = PostprocessApp {
        op,
        state: state.clone(),
        cancelled: cancelled.clone(),
        directory: directory.clone(),
    };

    let app_state = state.clone();
    let app_cancelled = cancelled.clone();

    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            setup_custom_styles(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();

            // Spawn worker thread
            let sw = app_state;
            let cw = app_cancelled;
            let dir = directory;
            let cfg = config;
            std::thread::spawn(move || worker(op, dir, cfg, sw, cw, ctx));

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    let final_phase = state.lock().unwrap().phase.clone();
    match final_phase {
        PostprocessPhase::Failed(e) => Err(AppError::DownloadFailed(e)),
        _ => Ok(()),
    }
}

fn setup_custom_styles(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    let bg = egui::Color32::from_rgb(28, 28, 28);
    let surface = egui::Color32::from_rgb(39, 39, 39);
    let surface2 = egui::Color32::from_rgb(49, 49, 49);
    let border = egui::Color32::from_rgb(64, 64, 64);
    let text = egui::Color32::from_rgb(216, 216, 216);
    let accent = egui::Color32::from_rgb(136, 136, 136);

    style.visuals.window_fill = bg;
    style.visuals.panel_fill = bg;
    style.visuals.faint_bg_color = surface;
    style.visuals.extreme_bg_color = surface;

    style.visuals.widgets.noninteractive.bg_fill = surface;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, border);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, text);
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.inactive.bg_fill = surface2;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, border);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, text);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.hovered.bg_fill = accent;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, accent);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.active.bg_fill = border;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, border);
    style.visuals.widgets.active.rounding = egui::Rounding::same(8.0);

    style.visuals.selection.bg_fill = accent;
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;

    ctx.set_style(style);
}

struct PostprocessApp {
    op: PostprocessOp,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    directory: PathBuf,
}

impl eframe::App for PostprocessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let s = self.state.lock().unwrap();
        let phase = s.phase.clone();
        let current_file = s.current_file.clone();
        let file_progress = s.file_progress;
        let current_idx = s.current_idx;
        let total_files = s.total_files;
        let status = s.status_text.clone();
        let results = s.results.clone();
        drop(s);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 10.0);

            ui.add_space(6.0);

            // Operation title
            ui.label(
                egui::RichText::new(format!("{}", self.op))
                    .size(16.0)
                    .strong()
                    .color(egui::Color32::from_rgb(220, 220, 220)),
            );

            // Directory
            ui.label(
                egui::RichText::new(trunc(&self.directory.display().to_string(), 72))
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );

            ui.separator();

            // Current file info
            if !current_file.is_empty() {
                let label = if total_files > 0 {
                    format!("[{}/{}] {}", current_idx + 1, total_files, current_file)
                } else {
                    current_file.clone()
                };
                ui.label(
                    egui::RichText::new(trunc(&label, 72))
                        .color(egui::Color32::from_rgb(200, 200, 200)),
                );
            }

            // Progress bar
            ui.add_space(2.0);
            let is_indeterminate = file_progress < 0.0
                || matches!(phase, PostprocessPhase::Starting);

            if is_indeterminate {
                ui.add(egui::ProgressBar::new(0.0).animate(true));
            } else {
                // Show overall progress: (completed_files + current_progress) / total
                let overall = if total_files > 0 {
                    let completed = current_idx as f64;
                    let current_frac = file_progress / 100.0;
                    ((completed + current_frac) / total_files as f64) as f32
                } else {
                    0.0
                };
                ui.add(
                    egui::ProgressBar::new(overall.clamp(0.0, 1.0))
                        .show_percentage()
                );
            }

            // Status text
            let color = match phase {
                PostprocessPhase::Done => egui::Color32::from_rgb(180, 180, 180),
                PostprocessPhase::Failed(_) => egui::Color32::from_rgb(180, 120, 120),
                _ => egui::Color32::from_rgb(210, 210, 210),
            };
            ui.label(egui::RichText::new(&status).color(color));

            // Results summary (when done)
            if matches!(phase, PostprocessPhase::Done) && !results.is_empty() {
                let success = results.iter().filter(|(_, r)| matches!(r, FileResult::Success)).count();
                let skipped = results.iter().filter(|(_, r)| matches!(r, FileResult::Skipped(_))).count();
                let failed = results.iter().filter(|(_, r)| matches!(r, FileResult::Failed(_))).count();

                let summary = format!(
                    "✓ {} succeeded  ⊘ {} skipped  ✗ {} failed",
                    success, skipped, failed
                );
                ui.label(
                    egui::RichText::new(summary)
                        .color(egui::Color32::from_rgb(160, 160, 160))
                        .small(),
                );
            }

            ui.add_space(8.0);

            // Buttons
            ui.horizontal(|ui| {
                let finished = matches!(
                    phase,
                    PostprocessPhase::Done | PostprocessPhase::Failed(_) | PostprocessPhase::Cancelled
                );

                let cancel_text = if finished { "Close" } else { "Cancel" };

                if ui.button(cancel_text).clicked() {
                    if finished {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.cancelled.store(true, Ordering::Relaxed);
                        let mut s = self.state.lock().unwrap();
                        s.status_text = "Cancelling...".to_string();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let open_dir = match self.op {
                        PostprocessOp::Compress => self.directory.join("small"),
                        PostprocessOp::ConvertCompatible => self.directory.clone(),
                    };

                    if ui
                        .add_enabled(
                            matches!(phase, PostprocessPhase::Done),
                            egui::Button::new("Open Folder"),
                        )
                        .clicked()
                    {
                        let _ = std::process::Command::new("explorer").arg(&open_dir).spawn();
                    }
                });
            });
        });
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n).collect::<String>())
    }
}

fn worker(
    op: PostprocessOp,
    directory: PathBuf,
    config: Config,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    // Count files first
    let files = postprocess::find_video_files(&directory);
    {
        let mut s = state.lock().unwrap();
        s.total_files = files.len();
        if files.is_empty() {
            s.phase = PostprocessPhase::Done;
            s.status_text = "No video files found in this folder.".to_string();
            ctx.request_repaint();
            return;
        }
        s.phase = PostprocessPhase::Processing;
        s.status_text = format!("Processing {} files...", files.len());
    }
    ctx.request_repaint();

    let st = state.clone();
    let ctx2 = ctx.clone();
    let on_progress: postprocess::PostprocessCallback = Box::new(move |idx, total, name, pct| {
        let mut s = st.lock().unwrap();
        s.current_idx = idx;
        s.total_files = total;
        s.current_file = name.to_string();
        s.file_progress = pct;
        if pct < 0.0 {
            s.status_text = format!("Skipping: {}", name);
        } else {
            s.status_text = format!(
                "Processing [{}/{}]: {} — {:.1}%",
                idx + 1,
                total,
                trunc(name, 40),
                pct
            );
        }
        ctx2.request_repaint();
    });

    match postprocess::run_postprocess(op, &directory, &config, cancelled, on_progress) {
        Ok(results) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Done;
            let success_count = results.iter().filter(|(_, r)| matches!(r, FileResult::Success)).count();
            s.status_text = format!("Done — {} files processed successfully.", success_count);
            s.results = results
                .into_iter()
                .map(|(p, r)| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    (name, r)
                })
                .collect();
            s.file_progress = 100.0;
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Cancelled;
            s.status_text = "Cancelled.".to_string();
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Failed(e.to_string());
            s.status_text = format!("Error: {}", e);
        }
    }
    ctx.request_repaint();
}

/// Launch the post-processing GUI window for an explicit list of files (blocking).
///
/// All files are processed sequentially in a single window.
pub fn run_postprocess_window_files(
    op: PostprocessOp,
    files: Vec<PathBuf>,
    directory: PathBuf,
    config: Config,
) -> Result<(), AppError> {
    let cancelled = Arc::new(AtomicBool::new(false));

    let state = Arc::new(Mutex::new(PostprocessState {
        phase: PostprocessPhase::Starting,
        current_idx: 0,
        total_files: files.len(),
        current_file: String::new(),
        file_progress: -1.0,
        status_text: format!("Processing {} files...", files.len()),
        results: Vec::new(),
    }));

    let title = format!("Paste Link Downloader — {}", op);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 300.0])
            .with_title(&title),
        ..Default::default()
    };

    let app = PostprocessApp {
        op,
        state: state.clone(),
        cancelled: cancelled.clone(),
        directory: directory.clone(),
    };

    let app_state = state.clone();
    let app_cancelled = cancelled.clone();

    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            setup_custom_styles(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();

            let sw = app_state;
            let cw = app_cancelled;
            let cfg = config;
            let dir = directory;
            std::thread::spawn(move || worker_files(op, files, dir, cfg, sw, cw, ctx));

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    let final_phase = state.lock().unwrap().phase.clone();
    match final_phase {
        PostprocessPhase::Failed(e) => Err(AppError::DownloadFailed(e)),
        _ => Ok(()),
    }
}

fn worker_files(
    op: PostprocessOp,
    files: Vec<PathBuf>,
    directory: PathBuf,
    config: Config,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    {
        let mut s = state.lock().unwrap();
        s.total_files = files.len();
        if files.is_empty() {
            s.phase = PostprocessPhase::Done;
            s.status_text = "No video files to process.".to_string();
            ctx.request_repaint();
            return;
        }
        s.phase = PostprocessPhase::Processing;
        s.status_text = format!("Processing {} files...", files.len());
    }
    ctx.request_repaint();

    let st = state.clone();
    let ctx2 = ctx.clone();
    let on_progress: postprocess::PostprocessCallback = Box::new(move |idx, total, name, pct| {
        let mut s = st.lock().unwrap();
        s.current_idx = idx;
        s.total_files = total;
        s.current_file = name.to_string();
        s.file_progress = pct;
        if pct < 0.0 {
            s.status_text = format!("Skipping: {}", name);
        } else {
            s.status_text = format!(
                "Processing [{}/{}]: {} — {:.1}%",
                idx + 1,
                total,
                trunc(name, 40),
                pct
            );
        }
        ctx2.request_repaint();
    });

    match postprocess::run_postprocess_files(op, &files, &directory, &config, cancelled, on_progress) {
        Ok(results) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Done;
            let success_count = results.iter().filter(|(_, r)| matches!(r, FileResult::Success)).count();
            s.status_text = format!("Done — {} files processed successfully.", success_count);
            s.results = results
                .into_iter()
                .map(|(p, r)| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    (name, r)
                })
                .collect();
            s.file_progress = 100.0;
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Cancelled;
            s.status_text = "Cancelled.".to_string();
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Failed(e.to_string());
            s.status_text = format!("Error: {}", e);
        }
    }
    ctx.request_repaint();
}

/// Launch the post-processing GUI window for a single file (blocking).
pub fn run_postprocess_window_file(
    op: PostprocessOp,
    file: PathBuf,
    config: Config,
) -> Result<(), AppError> {
    let cancelled = Arc::new(AtomicBool::new(false));

    let directory = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| file.clone());

    let file_name = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let state = Arc::new(Mutex::new(PostprocessState {
        phase: PostprocessPhase::Starting,
        current_idx: 0,
        total_files: 1,
        current_file: file_name,
        file_progress: 0.0,
        status_text: "Starting...".to_string(),
        results: Vec::new(),
    }));

    let title = format!("Paste Link Downloader — {}", op);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 300.0])
            .with_title(&title),
        ..Default::default()
    };

    let app = PostprocessApp {
        op,
        state: state.clone(),
        cancelled: cancelled.clone(),
        directory: directory.clone(),
    };

    let app_state = state.clone();
    let app_cancelled = cancelled.clone();

    eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            setup_custom_styles(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();

            let sw = app_state;
            let cw = app_cancelled;
            let f = file;
            let cfg = config;
            std::thread::spawn(move || worker_file(op, f, cfg, sw, cw, ctx));

            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    let final_phase = state.lock().unwrap().phase.clone();
    match final_phase {
        PostprocessPhase::Failed(e) => Err(AppError::DownloadFailed(e)),
        _ => Ok(()),
    }
}

fn worker_file(
    op: PostprocessOp,
    file: PathBuf,
    config: Config,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
) {
    {
        let mut s = state.lock().unwrap();
        s.phase = PostprocessPhase::Processing;
        s.status_text = format!(
            "Processing: {}",
            file.file_name().unwrap_or_default().to_string_lossy()
        );
    }
    ctx.request_repaint();

    let st = state.clone();
    let ctx2 = ctx.clone();
    let on_progress: postprocess::PostprocessCallback = Box::new(move |_idx, _total, name, pct| {
        let mut s = st.lock().unwrap();
        s.current_file = name.to_string();
        s.file_progress = pct;
        s.status_text = format!("Processing: {} — {:.1}%", trunc(name, 48), pct);
        ctx2.request_repaint();
    });

    match postprocess::run_postprocess_single(op, &file, &config, cancelled, on_progress) {
        Ok(results) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Done;
            let success = results.iter().any(|(_, r)| matches!(r, FileResult::Success));
            s.status_text = if success {
                "Done — file processed successfully.".to_string()
            } else {
                "Processing failed.".to_string()
            };
            s.results = results
                .into_iter()
                .map(|(p, r)| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    (name, r)
                })
                .collect();
            s.file_progress = 100.0;
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Cancelled;
            s.status_text = "Cancelled.".to_string();
        }
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Failed(e.to_string());
            s.status_text = format!("Error: {}", e);
        }
    }
    ctx.request_repaint();
}
