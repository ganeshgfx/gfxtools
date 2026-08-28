// src/advanced_download_gui.rs
//
// Advanced download options window, shown when the user holds Shift while
// clicking "Paste link" in Explorer. Lets the user choose:
//   - Video / Audio stream toggles
//   - Audio format (when audio-only)
//   - Resolution cap
//   - Audio bitrate
//   - Start / End time (trim)
//
// Returns `Some(AdvancedOptions)` on Download, or `None` on Cancel / close.

use crate::downloader::AdvancedOptions;
use eframe::egui;
use std::sync::{Arc, Mutex};
use url::Url;

/// Internal state for the options form.
struct OptionsForm {
    video: bool,
    audio: bool,
    audio_format_idx: usize,
    resolution_idx: usize,
    audio_bitrate_idx: usize,
    start_time: String,
    end_time: String,
    /// `None` = window closed / cancelled. `Some(opts)` = user clicked Download.
    result: Option<AdvancedOptions>,
    submitted: bool,
    cancelled: bool,
}

const AUDIO_FORMATS: &[&str] = &["mp3", "m4a", "opus", "flac", "wav"];
const RESOLUTIONS: &[(&str, Option<u32>)] = &[
    ("Best", None),
    ("2160p (4K)", Some(2160)),
    ("1440p (2K)", Some(1440)),
    ("1080p", Some(1080)),
    ("720p", Some(720)),
    ("480p", Some(480)),
    ("360p", Some(360)),
];
const AUDIO_BITRATES: &[(&str, Option<&str>)] = &[
    ("Best (default)", None),
    ("320k", Some("320k")),
    ("256k", Some("256k")),
    ("192k", Some("192k")),
    ("128k", Some("128k")),
    ("96k", Some("96k")),
];

/// Show the advanced download options window (blocking).
///
/// Returns `Some(AdvancedOptions)` if the user clicked Download,
/// or `None` if they cancelled / closed the window.
pub fn show_advanced_options(url: &Url) -> Option<AdvancedOptions> {
    let form = Arc::new(Mutex::new(OptionsForm {
        video: true,
        audio: true,
        audio_format_idx: 0,    // mp3
        resolution_idx: 0,      // Best
        audio_bitrate_idx: 0,   // Best
        start_time: String::new(),
        end_time: String::new(),
        result: None,
        submitted: false,
        cancelled: false,
    }));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 400.0])
            .with_title("Download Options"),
        ..Default::default()
    };

    let url_display = url.to_string();
    let app = AdvancedOptionsApp {
        form: form.clone(),
        url_display,
    };

    let _ = eframe::run_native(
        "Download Options",
        options,
        Box::new(|cc| {
            setup_custom_styles(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    );

    let f = form.lock().unwrap();
    f.result.clone()
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

struct AdvancedOptionsApp {
    form: Arc<Mutex<OptionsForm>>,
    url_display: String,
}

impl eframe::App for AdvancedOptionsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut f = self.form.lock().unwrap();

        let mut download_clicked = false;
        let mut cancel_clicked = false;

        let is_audio_only = !f.video && f.audio;
        let is_video_only = f.video && !f.audio;
        let neither = !f.video && !f.audio;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 10.0);

            // URL display
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(trunc(&self.url_display, 62))
                    .color(egui::Color32::from_rgb(150, 150, 150)),
            );

            ui.separator();

            // ── Download Type ───────────────────────────────────────
            ui.label(egui::RichText::new("Download").strong());
            ui.horizontal(|ui| {
                ui.checkbox(&mut f.video, "Video");
                ui.add_space(16.0);
                ui.checkbox(&mut f.audio, "Audio");
            });

            ui.add_space(4.0);

            // ── Audio format (only when audio-only) ─────────────────
            if is_audio_only {
                ui.horizontal(|ui| {
                    ui.label("Audio format:");
                    egui::ComboBox::from_id_source("audio_fmt")
                        .width(140.0)
                        .selected_text(AUDIO_FORMATS[f.audio_format_idx])
                        .show_ui(ui, |ui| {
                            for (i, fmt) in AUDIO_FORMATS.iter().enumerate() {
                                ui.selectable_value(&mut f.audio_format_idx, i, *fmt);
                            }
                        });
                });
                ui.add_space(4.0);
            }

            // ── Quality ─────────────────────────────────────────────
            ui.label(egui::RichText::new("Quality").strong());

            egui::Grid::new("quality_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    // Resolution (only if video is included)
                    if !is_audio_only {
                        ui.label("Resolution:");
                        egui::ComboBox::from_id_source("resolution")
                            .width(160.0)
                            .selected_text(RESOLUTIONS[f.resolution_idx].0)
                            .show_ui(ui, |ui| {
                                for (i, (label, _)) in RESOLUTIONS.iter().enumerate() {
                                    ui.selectable_value(&mut f.resolution_idx, i, *label);
                                }
                            });
                        ui.end_row();
                    }

                    // Audio bitrate (only if audio is included)
                    if !is_video_only {
                        ui.label("Audio quality:");
                        egui::ComboBox::from_id_source("audio_br")
                            .width(160.0)
                            .selected_text(AUDIO_BITRATES[f.audio_bitrate_idx].0)
                            .show_ui(ui, |ui| {
                                for (i, (label, _)) in AUDIO_BITRATES.iter().enumerate() {
                                    ui.selectable_value(&mut f.audio_bitrate_idx, i, *label);
                                }
                            });
                        ui.end_row();
                    }
                });

            ui.add_space(4.0);

            // ── Trim (In / Out) ─────────────────────────────────────
            ui.label(egui::RichText::new("Trim").strong());
            egui::Grid::new("trim_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Start time:");
                    ui.add(
                        egui::TextEdit::singleline(&mut f.start_time)
                            .desired_width(160.0)
                            .hint_text("HH:MM:SS or seconds"),
                    );
                    ui.end_row();

                    ui.label("End time:");
                    ui.add(
                        egui::TextEdit::singleline(&mut f.end_time)
                            .desired_width(160.0)
                            .hint_text("HH:MM:SS or seconds"),
                    );
                    ui.end_row();
                });

            ui.add_space(16.0);

            // ── Buttons ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    cancel_clicked = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_download = !neither;
                    if ui
                        .add_enabled(can_download, egui::Button::new("Download"))
                        .clicked()
                    {
                        download_clicked = true;
                    }
                    if neither {
                        ui.label(
                            egui::RichText::new("Select at least one stream")
                                .color(egui::Color32::from_rgb(180, 100, 100))
                                .small(),
                        );
                    }
                });
            });
        });

        if download_clicked {
            f.result = Some(AdvancedOptions {
                video: f.video,
                audio: f.audio,
                audio_format: if is_audio_only {
                    Some(AUDIO_FORMATS[f.audio_format_idx].to_string())
                } else {
                    None
                },
                max_resolution: RESOLUTIONS[f.resolution_idx].1,
                audio_bitrate: AUDIO_BITRATES[f.audio_bitrate_idx]
                    .1
                    .map(|s| s.to_string()),
                start_time: if f.start_time.trim().is_empty() {
                    None
                } else {
                    Some(f.start_time.trim().to_string())
                },
                end_time: if f.end_time.trim().is_empty() {
                    None
                } else {
                    Some(f.end_time.trim().to_string())
                },
            });
            f.submitted = true;
            drop(f);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if cancel_clicked {
            let mut f2 = self.form.lock().unwrap();
            f2.cancelled = true;
            drop(f2);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn trunc(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(n).collect::<String>())
    }
}
