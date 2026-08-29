// src/download_gui.rs
// Native download-progress window using eframe (egui).
// Replaces the raw Win32 implementation.

use crate::config::Config;
use crate::downloader::{
    download, download_images, resolve_gallery_dl, AdvancedOptions, DownloadOptions, ImageDownloadOptions,
};
use crate::error::AppError;
use crate::platform::Platform;
use crate::progress::ProgressEvent;

use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::warn;
use url::Url;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Phase {
    Starting,
    YtDlp,
    GalleryDl,
    Done,
    Failed(String),
    Cancelled,
}

pub(crate) struct GuiState {
    pub phase: Phase,
    pub progress_pct: f64,
    pub status_text: String,
}

pub fn run_download_window(
    url: &Url,
    platform: &Platform,
    output_dir: PathBuf,
    config: Config,
    cancelled: Arc<AtomicBool>,
    advanced: Option<AdvancedOptions>,
) -> Result<(), AppError> {
    let url_str = url.to_string();
    let plat_str = platform.to_string();

    let state = Arc::new(Mutex::new(GuiState {
        phase: Phase::Starting,
        progress_pct: 0.0,
        status_text: "Starting...".to_string(),
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([520.0, 220.0])
            .with_title("Paste Link Downloader")
            .with_icon(std::sync::Arc::new(crate::app_icon::load_icon())),
        ..Default::default()
    };

    let app = DownloadApp {
        state: state.clone(),
        cancelled: cancelled.clone(),
        output_dir: output_dir.clone(),
        url_str: url_str.clone(),
        plat_str,
    };

    let app_state = state.clone();
    let app_cancelled = cancelled.clone();
    eframe::run_native(
        "Paste Link Downloader",
        options,
        Box::new(move |cc| {
            setup_custom_styles(&cc.egui_ctx);
            let ctx_clone = cc.egui_ctx.clone();
            
            // Spawn worker
            let sw = app_state;
            let cw = app_cancelled;
            let uw = url_str;
            let dw = output_dir;
            let cfg = config;
            let adv_w = advanced;
            std::thread::spawn(move || worker(uw, dw, cfg, sw, cw, ctx_clone, adv_w));
            
            Ok(Box::new(app))
        }),
    ).map_err(|e| AppError::ConfigError(e.to_string()))?;

    let final_phase = state.lock().unwrap().phase.clone();
    match final_phase {
        Phase::Failed(e) => Err(AppError::DownloadFailed(e)),
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
    ctx.set_style(style);
}

struct DownloadApp {
    state: Arc<Mutex<GuiState>>,
    cancelled: Arc<AtomicBool>,
    output_dir: PathBuf,
    url_str: String,
    plat_str: String,
}

impl eframe::App for DownloadApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.state.lock().unwrap();
        let phase = state.phase.clone();
        let pct = state.progress_pct as f32;
        let text = state.status_text.clone();
        drop(state);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
            
            ui.add_space(8.0);
            ui.label(format!("Platform: {}", self.plat_str));
            ui.label(format!("URL: {}", trunc(&self.url_str, 68)));
            ui.label(format!("Save: {}", trunc(&self.output_dir.display().to_string(), 68)));

            ui.add_space(4.0);
            
            let is_marquee = matches!(phase, Phase::Starting | Phase::GalleryDl);
            if is_marquee {
                ui.add(egui::ProgressBar::new(0.0).animate(true));
            } else {
                ui.add(egui::ProgressBar::new(pct / 100.0).show_percentage());
            }

            let color = match phase {
                Phase::Done => egui::Color32::from_rgb(180, 180, 180),
                Phase::Failed(_) => egui::Color32::from_rgb(110, 110, 110),
                _ => egui::Color32::from_rgb(210, 210, 210),
            };
            ui.label(egui::RichText::new(&text).color(color));

            ui.add_space(16.0);
            
            ui.horizontal(|ui| {
                let finished = matches!(phase, Phase::Done | Phase::Failed(_) | Phase::Cancelled);
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
                    if ui.add_enabled(matches!(phase, Phase::Done), egui::Button::new("Open Folder")).clicked() {
                        let _ = std::process::Command::new("explorer").arg(&self.output_dir).spawn();
                    }
                });
            });
        });
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() }
    else { format!("{}...", s.chars().take(n).collect::<String>()) }
}

fn worker(
    url: String,
    output_dir: PathBuf,
    config: Config,
    state: Arc<Mutex<GuiState>>,
    cancelled: Arc<AtomicBool>,
    ctx: egui::Context,
    advanced: Option<AdvancedOptions>,
) {
    {
        let mut s = state.lock().unwrap();
        s.phase = Phase::YtDlp;
        s.status_text = "Downloading via yt-dlp...".to_string();
    }
    ctx.request_repaint();

    let st2 = state.clone();
    let ctx2 = ctx.clone();
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
        ctx2.request_repaint();
    });

    let opts = DownloadOptions { url: url.clone(), output_dir: output_dir.clone(), format: config.preferred_format.clone(), advanced };
    let yt_err = match download(&opts, &config, cancelled.clone(), on_ytdlp) {
        Ok(()) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Done;
            s.progress_pct = 100.0;
            s.status_text = "Files saved successfully.".to_string();
            ctx.request_repaint();
            return;
        }
        Err(AppError::Cancelled) => {
            let mut s = state.lock().unwrap();
            s.phase = Phase::Cancelled;
            s.status_text = "Cancelled.".to_string();
            ctx.request_repaint();
            return;
        }
        Err(e) => { 
            // When advanced options are set (audio-only, resolution cap, etc.),
            // gallery-dl fallback makes no sense — it only downloads images.
            if opts.advanced.is_some() {
                let mut s = state.lock().unwrap();
                s.phase = Phase::Failed(e.to_string());
                s.status_text = format!("Download failed: {}", e);
                ctx.request_repaint();
                return;
            }
            warn!("yt-dlp failed ({e}), trying gallery-dl...");
            e
        }
    };

    if resolve_gallery_dl(&config).is_err() {
        let mut s = state.lock().unwrap();
        s.phase = Phase::Failed("yt-dlp failed. Install gallery-dl for image support.".to_string());
        s.status_text = "Download failed.".to_string();
        ctx.request_repaint();
        return;
    }

    {
        let mut s = state.lock().unwrap();
        s.phase = Phase::GalleryDl;
        s.status_text = "yt-dlp failed — trying gallery-dl...".to_string();
        s.progress_pct = 0.0;
    }
    ctx.request_repaint();

    let st3 = state.clone();
    let ctx3 = ctx.clone();
    let on_img: Box<dyn Fn(ProgressEvent) + Send + 'static> = Box::new(move |ev| {
        if let ProgressEvent::Other(line) = ev {
            if !line.is_empty() && !line.starts_with('#') {
                let mut s = st3.lock().unwrap();
                s.status_text = trunc(&line, 72);
                ctx3.request_repaint();
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
        Err(img_err) => {
            let msg = format!("yt-dlp: {}\ngallery-dl: {}", yt_err, img_err);
            let mut s = state.lock().unwrap();
            s.phase = Phase::Failed(msg);
            s.status_text = format!("Error: {}", yt_err);
        }
    }
    ctx.request_repaint();
}
