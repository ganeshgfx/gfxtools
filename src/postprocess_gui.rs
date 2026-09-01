// src/postprocess_gui.rs
// eframe/egui progress window for post-processing operations.
// Windows copy-dialog inspired UI with:
//   - Dynamic per-file + overall progress bars (custom drawn)
//   - Real-time speed graph (bar chart, newest = brightest)
//   - Expand / Compact view toggle
//   - Elapsed time + ETA + encoding speed display

use crate::config::Config;
use crate::error::AppError;
use crate::postprocess::{self, FileResult, FfmpegStats, PostprocessOp};

use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// ─── Constants ─────────────────────────────────────────────────────────────────

const COMPACT_SIZE: [f32; 2] = [580.0, 265.0];
const EXPANDED_SIZE: [f32; 2] = [580.0, 510.0];
const MAX_SPEED_HISTORY: usize = 60;

// ─── State ────────────────────────────────────────────────────────────────────

struct PostprocessState {
    phase: PostprocessPhase,
    current_idx: usize,
    total_files: usize,
    current_file: String,
    /// 0.0-100.0 for current file, negative = indeterminate
    file_progress: f64,
    status_text: String,
    results: Vec<(String, FileResult)>,
    /// Encoding speed multiplier (0 = unknown)
    speed: f64,
    /// Encoder fps (0 = unknown)
    fps: f64,
    /// Recent speed samples for graph (oldest first)
    speed_history: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
enum PostprocessPhase {
    Starting,
    Processing,
    Done,
    Failed(String),
    Cancelled,
}

// ─── App ─────────────────────────────────────────────────────────────────────

struct PostprocessApp {
    op: PostprocessOp,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    directory: PathBuf,
    /// Controls compact vs expanded view
    expanded: bool,
    /// Tracks last sent window size to avoid redundant viewport commands
    last_expanded: Option<bool>,
    /// Clock started when the window opens
    start_time: Instant,
}

// ─── Public entry points ──────────────────────────────────────────────────────

/// Launch the post-processing GUI window for all files in a directory.
pub fn run_postprocess_window(
    op: PostprocessOp,
    directory: PathBuf,
    config: Config,
) -> Result<(), AppError> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(make_state(0)));
    let title = format!("GFX Tools — {}", op);
    let options = make_options(&title);
    let app = make_app(op, state.clone(), cancelled.clone(), directory.clone());

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
            let dir = directory;
            let cfg = config;
            std::thread::spawn(move || worker(op, dir, cfg, sw, cw, ctx));
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    finish_check(state)
}

/// Launch the post-processing GUI window for an explicit file list.
pub fn run_postprocess_window_files(
    op: PostprocessOp,
    files: Vec<PathBuf>,
    directory: PathBuf,
    config: Config,
) -> Result<(), AppError> {
    let n = files.len();
    let cancelled = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(make_state(n)));
    let title = format!("GFX Tools — {}", op);
    let options = make_options(&title);
    let app = make_app(op, state.clone(), cancelled.clone(), directory.clone());

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
            let dir = directory;
            let cfg = config;
            std::thread::spawn(move || worker_files(op, files, dir, cfg, sw, cw, ctx));
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    finish_check(state)
}

/// Launch the post-processing GUI window for a single file.
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
    let state = Arc::new(Mutex::new(make_state(1)));
    let title = format!("GFX Tools — {}", op);
    let options = make_options(&title);
    let app = make_app(op, state.clone(), cancelled.clone(), directory.clone());

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
            std::thread::spawn(move || worker_file(op, file, cfg, sw, cw, ctx));
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| AppError::ConfigError(e.to_string()))?;

    finish_check(state)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_state(total: usize) -> PostprocessState {
    PostprocessState {
        phase: PostprocessPhase::Starting,
        current_idx: 0,
        total_files: total,
        current_file: String::new(),
        file_progress: -1.0,
        status_text: "Scanning for video files...".to_string(),
        results: Vec::new(),
        speed: 0.0,
        fps: 0.0,
        speed_history: Vec::new(),
    }
}

fn make_options(title: &str) -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(COMPACT_SIZE)
            .with_min_inner_size([400.0, 180.0])
            .with_title(title)
            .with_icon(std::sync::Arc::new(crate::app_icon::load_icon())),
        ..Default::default()
    }
}

fn make_app(
    op: PostprocessOp,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    directory: PathBuf,
) -> PostprocessApp {
    PostprocessApp {
        op,
        state,
        cancelled,
        directory,
        expanded: false,
        last_expanded: None,
        start_time: Instant::now(),
    }
}

fn finish_check(state: Arc<Mutex<PostprocessState>>) -> Result<(), AppError> {
    match state.lock().unwrap().phase.clone() {
        PostprocessPhase::Failed(e) => Err(AppError::DownloadFailed(e)),
        _ => Ok(()),
    }
}

// ─── Styles ───────────────────────────────────────────────────────────────────

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
    style.spacing.button_padding = egui::vec2(12.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);

    // Increase font sizes — egui's built-in Hack font renders small by default
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Small,    FontId::proportional(13.0)),
        (TextStyle::Body,     FontId::proportional(15.0)),
        (TextStyle::Monospace,FontId::monospace(14.0)),
        (TextStyle::Button,   FontId::proportional(15.0)),
        (TextStyle::Heading,  FontId::proportional(19.0)),
    ].into();

    ctx.set_style(style);
}

// ─── eframe::App ─────────────────────────────────────────────────────────────

impl eframe::App for PostprocessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Keep the UI alive even when worker is quiet (updates elapsed/ETA display)
        ctx.request_repaint_after(std::time::Duration::from_millis(250));

        // Resize window when expand/collapse toggled
        if self.last_expanded != Some(self.expanded) {
            self.last_expanded = Some(self.expanded);
            let size = if self.expanded { EXPANDED_SIZE } else { COMPACT_SIZE };
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(size[0], size[1])));
        }

        // ── Read shared state ──────────────────────────────────────────
        let s = self.state.lock().unwrap();
        let phase = s.phase.clone();
        let current_file = s.current_file.clone();
        let file_progress = s.file_progress;
        let current_idx = s.current_idx;
        let total_files = s.total_files;
        let status = s.status_text.clone();
        let results = s.results.clone();
        let speed = s.speed;
        let speed_history = s.speed_history.clone();
        drop(s);

        let finished = matches!(
            phase,
            PostprocessPhase::Done | PostprocessPhase::Failed(_) | PostprocessPhase::Cancelled
        );

        // ── Compute derived values ─────────────────────────────────────
        let overall = if finished {
            1.0f32
        } else if total_files > 0 && file_progress >= 0.0 {
            let completed = current_idx as f64;
            let frac = file_progress / 100.0;
            ((completed + frac) / total_files as f64).clamp(0.0, 1.0) as f32
        } else {
            0.0f32
        };

        let elapsed_secs = self.start_time.elapsed().as_secs_f64();
        let eta_text: Option<String> = if !finished && overall > 0.02 {
            let remaining = elapsed_secs * (1.0 - overall as f64) / overall as f64;
            Some(format_dur(remaining))
        } else {
            None
        };

        // ── Palette ───────────────────────────────────────────────────
        let blue = egui::Color32::from_rgb(40, 130, 255);
        let green = egui::Color32::from_rgb(80, 200, 100);
        let track = egui::Color32::from_rgb(52, 52, 52);
        let dim = egui::Color32::from_rgb(120, 120, 120);
        let bright = egui::Color32::from_rgb(210, 210, 210);
        let red = egui::Color32::from_rgb(200, 90, 90);

        // ── UI ────────────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);

            // Header row: title + expand button
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{}", self.op))
                        .size(17.0)
                        .strong()
                        .color(bright),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_label = if self.expanded { "[^] Less" } else { "[v] Details" };
                    if ui.small_button(btn_label).clicked() {
                        self.expanded = !self.expanded;
                    }
                });
            });

            // Directory subtitle
            ui.label(
                egui::RichText::new(trunc(&self.directory.display().to_string(), 72))
                    .color(dim)
                    .small(),
            );

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);

            // Current file name
            if !current_file.is_empty() {
                let label = if total_files > 0 {
                    format!("[{}/{}]  {}", current_idx + 1, total_files, current_file)
                } else {
                    current_file.clone()
                };
                ui.label(egui::RichText::new(trunc(&label, 72)).color(bright));
            } else {
                ui.label(egui::RichText::new(&status).color(dim).small());
            }

            ui.add_space(4.0);

            // ── Per-file progress bar (thin, 7px) ─────────────────────
            let avail_w = ui.available_width();
            {
                let bar_h = 7.0;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(avail_w, bar_h),
                    egui::Sense::hover(),
                );
                let p = ui.painter().with_clip_rect(rect);
                p.rect_filled(rect, egui::Rounding::same(3.5), track);

                let is_indet = file_progress < 0.0
                    || matches!(phase, PostprocessPhase::Starting);

                if finished {
                    p.rect_filled(rect, egui::Rounding::same(3.5), green);
                } else if is_indet {
                    // Sliding indeterminate segment
                    let t = (ctx.input(|i| i.time) % 1.6) / 1.6;
                    let seg_w = avail_w * 0.28;
                    let x = rect.left() + (avail_w + seg_w) * t as f32 - seg_w;
                    let seg = egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(seg_w, bar_h),
                    );
                    let seg = seg.intersect(rect);
                    if seg.is_positive() {
                        p.rect_filled(seg, egui::Rounding::same(3.5),
                            egui::Color32::from_rgba_premultiplied(40, 130, 255, 160));
                    }
                    ctx.request_repaint(); // keep animating
                } else {
                    let fw = avail_w * (file_progress / 100.0).clamp(0.0, 1.0) as f32;
                    let fill = egui::Rect::from_min_size(rect.min, egui::vec2(fw, bar_h));
                    p.rect_filled(fill, egui::Rounding::same(3.5), blue);
                }
            }

            ui.add_space(4.0);

            // ── Overall progress bar (thick, 18px) with % label ────────
            {
                let bar_h = 18.0;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(avail_w, bar_h),
                    egui::Sense::hover(),
                );
                let p = ui.painter().with_clip_rect(rect);
                p.rect_filled(rect, egui::Rounding::same(5.0), track);

                let fill_color = if finished { green } else { blue };
                let fw = avail_w * overall;
                if fw > 0.0 {
                    let fill = egui::Rect::from_min_size(rect.min, egui::vec2(fw, bar_h));
                    p.rect_filled(fill, egui::Rounding::same(5.0), fill_color);
                }

                // Percentage text overlaid on bar
                let pct_str = format!("{:.0}%", overall * 100.0);
                let text_color = if overall > 0.48 {
                    egui::Color32::from_rgb(240, 240, 240)
                } else {
                    egui::Color32::from_rgb(160, 160, 160)
                };
                p.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &pct_str,
                    egui::FontId::proportional(13.0),
                    text_color,
                );
            }

            ui.add_space(4.0);

            // ── Stats row: elapsed | speed | ETA ─────────────────────
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Elapsed: {}", format_dur(elapsed_secs)))
                        .color(dim)
                        .small(),
                );
                if speed > 0.01 && !finished {
                    ui.label(
                        egui::RichText::new(format!("  ·  {:.2}× speed", speed))
                            .color(dim)
                            .small(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(eta) = &eta_text {
                        ui.label(
                            egui::RichText::new(format!("ETA: {}", eta))
                                .color(dim)
                                .small(),
                        );
                    }
                });
            });

            // ── Status text ────────────────────────────────────────────
            let sc = match &phase {
                PostprocessPhase::Done => green,
                PostprocessPhase::Failed(_) => red,
                PostprocessPhase::Cancelled => dim,
                _ => egui::Color32::from_rgb(185, 185, 185),
            };
            ui.label(egui::RichText::new(trunc(&status, 80)).color(sc).small());

            // ── Expanded section ──────────────────────────────────────
            if self.expanded {
                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // Speed graph header
                let graph_label = if !speed_history.is_empty() {
                    let max = speed_history.iter().cloned().fold(0.0f32, f32::max);
                    format!("Encoding speed  (peak {:.2}×)", max)
                } else {
                    "Encoding speed".to_string()
                };
                ui.label(egui::RichText::new(&graph_label).color(dim).small());
                ui.add_space(2.0);

                // Speed graph (40px tall bar chart)
                {
                    let graph_h = 44.0;
                    let avail = ui.available_width();
                    let (graph_rect, _) = ui.allocate_exact_size(
                        egui::vec2(avail, graph_h),
                        egui::Sense::hover(),
                    );
                    let p = ui.painter().with_clip_rect(graph_rect);
                    // Background
                    p.rect_filled(
                        graph_rect,
                        egui::Rounding::same(4.0),
                        egui::Color32::from_rgb(36, 36, 36),
                    );

                    if !speed_history.is_empty() {
                        let n = speed_history.len();
                        let bar_w = (avail / n as f32).max(2.0);
                        let max_s = speed_history.iter().cloned().fold(0.1f32, f32::max);
                        for (i, &s) in speed_history.iter().enumerate() {
                            let age = i as f32 / n as f32; // 0=oldest 1=newest
                            let bh = ((s / max_s) * (graph_h - 4.0)).max(2.0);
                            let x = graph_rect.left() + i as f32 * bar_w;
                            let br = egui::Rect::from_min_size(
                                egui::pos2(x, graph_rect.bottom() - bh - 2.0),
                                egui::vec2((bar_w - 1.0).max(1.5), bh),
                            );
                            // Newer bars are brighter blue
                            let a = (80.0 + 175.0 * age) as u8;
                            p.rect_filled(
                                br,
                                egui::Rounding::same(1.5),
                                egui::Color32::from_rgba_premultiplied(30, 110, 220, a),
                            );
                        }
                    } else {
                        // No data yet — show placeholder text
                        p.text(
                            graph_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "Waiting for data...",
                            egui::FontId::proportional(12.0),
                            egui::Color32::from_rgb(70, 70, 70),
                        );
                    }

                    // X-axis baseline
                    p.line_segment(
                        [
                            egui::pos2(graph_rect.left(), graph_rect.bottom() - 2.0),
                            egui::pos2(graph_rect.right(), graph_rect.bottom() - 2.0),
                        ],
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 60, 60)),
                    );
                }

                ui.add_space(4.0);

                // Per-file results list
                if !results.is_empty() {
                    ui.label(egui::RichText::new("File results:").color(dim).small());
                    ui.add_space(2.0);
                    let list_h = (results.len() as f32 * 19.0).min(110.0);
                    egui::ScrollArea::vertical()
                        .max_height(list_h)
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            for (name, result) in &results {
                                let (icon, color) = match result {
                                    FileResult::Success => ("[OK]", green),
                                    FileResult::Skipped(_) => ("[--]", dim),
                                    FileResult::Failed(_) => ("[XX]", red),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(icon).color(color).small());
                                    ui.label(
                                        egui::RichText::new(trunc(name, 62))
                                            .color(bright)
                                            .small(),
                                    );
                                });
                            }
                        });
                }

                // Done summary
                if matches!(phase, PostprocessPhase::Done) && !results.is_empty() {
                    let ok = results.iter().filter(|(_, r)| matches!(r, FileResult::Success)).count();
                    let sk = results.iter().filter(|(_, r)| matches!(r, FileResult::Skipped(_))).count();
                    let fail = results.iter().filter(|(_, r)| matches!(r, FileResult::Failed(_))).count();
                    let summary = format!("OK: {}   Skipped: {}   Failed: {}", ok, sk, fail);
                    ui.label(egui::RichText::new(summary).color(dim).small());
                }
            }

            // ── Buttons ────────────────────────────────────────────────
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let cancel_label = if finished { "Close" } else { "Cancel" };
                if ui.button(cancel_label).clicked() {
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
                        let _ = std::process::Command::new("explorer")
                            .arg(&open_dir)
                            .spawn();
                    }
                });
            });
        });
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n).collect::<String>())
    }
}

fn format_dur(secs: f64) -> String {
    let s = secs as u64;
    if secs < 60.0 {
        format!("0:{:02}", s)
    } else if secs < 3600.0 {
        format!("{}:{:02}", s / 60, s % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    }
}

/// Update state from a FfmpegStats callback value.
fn apply_stats(s: &mut PostprocessState, stats: FfmpegStats) {
    s.file_progress = stats.pct;
    if stats.speed > 0.01 {
        s.speed = stats.speed;
        s.fps = stats.fps;
        if s.speed_history.len() >= MAX_SPEED_HISTORY {
            s.speed_history.remove(0);
        }
        s.speed_history.push(stats.speed as f32);
    }
}

// ─── Worker threads ───────────────────────────────────────────────────────────

fn worker(
    op: PostprocessOp,
    directory: PathBuf,
    config: Config,
    state: Arc<Mutex<PostprocessState>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
) {
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

    run_worker_common(op, move |cb| {
        postprocess::run_postprocess(op, &directory, &config, cancelled, cb)
    }, state, ctx);
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

    run_worker_common(op, move |cb| {
        postprocess::run_postprocess_files(op, &files, &directory, &config, cancelled, cb)
    }, state, ctx);
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

    run_worker_common(op, move |cb| {
        postprocess::run_postprocess_single(op, &file, &config, cancelled, cb)
    }, state, ctx);
}

/// Shared worker body: builds the progress callback, runs `work_fn`, updates state.
fn run_worker_common<F>(
    _op: PostprocessOp,
    work_fn: F,
    state: Arc<Mutex<PostprocessState>>,
    ctx: egui::Context,
) where
    F: FnOnce(postprocess::PostprocessCallback) -> Result<Vec<(PathBuf, FileResult)>, AppError>,
{
    let st = state.clone();
    let ctx2 = ctx.clone();

    let on_progress: postprocess::PostprocessCallback = Box::new(move |idx, total, name, stats| {
        let mut s = st.lock().unwrap();
        s.current_idx = idx;
        s.total_files = total;
        s.current_file = name.to_string();
        apply_stats(&mut s, stats);
        if stats.pct < 0.0 {
            s.status_text = format!("Skipping: {}", name);
        } else {
            s.status_text = format!(
                "Processing [{}/{}]: {} — {:.1}%",
                idx + 1,
                total,
                trunc(name, 40),
                stats.pct
            );
        }
        ctx2.request_repaint();
    });

    match work_fn(on_progress) {
        Ok(results) => {
            let mut s = state.lock().unwrap();
            s.phase = PostprocessPhase::Done;
            let ok = results.iter().filter(|(_, r)| matches!(r, FileResult::Success)).count();
            s.status_text = format!("Done — {} file(s) processed successfully.", ok);
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
