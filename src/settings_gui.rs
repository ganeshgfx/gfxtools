// src/settings_gui.rs
//
// Native settings window for Paste Link Downloader, ported to eframe.

use crate::config::Config;
use crate::error::AppError;
use eframe::egui;

pub fn show_settings_window(config: &Config) -> Result<(), AppError> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([510.0, 524.0])
            .with_title("Paste Link Downloader — Settings"),
        ..Default::default()
    };

    let app = SettingsApp::new(config.clone());
    eframe::run_native(
        "Paste Link Downloader — Settings",
        options,
        Box::new(|cc| {
            setup_custom_styles(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    ).map_err(|e| AppError::ConfigError(e.to_string()))?;
    
    Ok(())
}

fn setup_custom_styles(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    
    // Base colors matching the plugin
    let bg = egui::Color32::from_rgb(28, 28, 28);
    let surface = egui::Color32::from_rgb(39, 39, 39);
    let surface2 = egui::Color32::from_rgb(49, 49, 49);
    let border = egui::Color32::from_rgb(64, 64, 64);
    let text = egui::Color32::from_rgb(216, 216, 216);
    let text_muted = egui::Color32::from_rgb(115, 115, 115);
    let accent = egui::Color32::from_rgb(136, 136, 136);

    style.visuals.window_fill = bg;
    style.visuals.panel_fill = bg;
    style.visuals.faint_bg_color = surface;
    style.visuals.extreme_bg_color = surface;
    
    // Widgets
    style.visuals.widgets.noninteractive.bg_fill = surface;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, border);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text);
    style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.inactive.bg_fill = surface2;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text);
    style.visuals.widgets.inactive.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.hovered.bg_fill = accent;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, accent);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(8.0);

    style.visuals.widgets.active.bg_fill = border;
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, border);
    style.visuals.widgets.active.rounding = egui::Rounding::same(8.0);

    style.visuals.selection.bg_fill = accent;

    // Add bit padding in buttons
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    // Make text edits a bit taller and padded
    style.spacing.interact_size.y = 28.0;

    ctx.set_style(style);
}

struct SettingsApp {
    config: Config,
    error_msg: Option<String>,
}

impl SettingsApp {
    fn new(config: Config) -> Self {
        Self { config, error_msg: None }
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);
            
            ui.add_space(8.0);
            
            let mut save_clicked = false;
            let mut cancel_clicked = false;
            
            egui::Grid::new("settings_grid")
                .num_columns(3)
                .spacing([12.0, 16.0])
                .min_col_width(100.0)
                .show(ui, |ui| {
                ui.label("yt-dlp path:");
                ui.add(egui::TextEdit::singleline(&mut self.config.yt_dlp_path).desired_width(280.0));
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Executable", &["exe"]).pick_file() {
                        self.config.yt_dlp_path = path.to_string_lossy().to_string();
                    }
                }
                ui.end_row();

                ui.label("FFmpeg dir:");
                ui.add(egui::TextEdit::singleline(&mut self.config.ffmpeg_dir).desired_width(280.0));
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.config.ffmpeg_dir = path.to_string_lossy().to_string();
                    }
                }
                ui.end_row();

                ui.label("gallery-dl path:");
                ui.add(egui::TextEdit::singleline(&mut self.config.gallery_dl_path).desired_width(280.0));
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Executable", &["exe"]).pick_file() {
                        self.config.gallery_dl_path = path.to_string_lossy().to_string();
                    }
                }
                ui.end_row();

                ui.label("Cookies file:");
                ui.add(egui::TextEdit::singleline(&mut self.config.cookies_file).desired_width(280.0));
                if ui.button("Browse…").clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Text files", &["txt"]).pick_file() {
                        self.config.cookies_file = path.to_string_lossy().to_string();
                    }
                }
                ui.end_row();
                
                ui.label("Cookie browser:");
                egui::ComboBox::from_id_source("browser")
                    .width(280.0)
                    .selected_text(if self.config.cookies_from_browser.is_empty() { "disabled" } else { &self.config.cookies_from_browser })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.cookies_from_browser, "".to_string(), "disabled");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "edge".to_string(), "edge");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "chrome".to_string(), "chrome");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "firefox".to_string(), "firefox");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "brave".to_string(), "brave");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "opera".to_string(), "opera");
                        ui.selectable_value(&mut self.config.cookies_from_browser, "chromium".to_string(), "chromium");
                    });
                ui.end_row();

                ui.label("Output format:");
                egui::ComboBox::from_id_source("format")
                    .width(280.0)
                    .selected_text(&self.config.preferred_format)
                    .show_ui(ui, |ui| {
                        for fmt in ["mp4", "mkv", "webm", "mov", "avi"] {
                            ui.selectable_value(&mut self.config.preferred_format, fmt.to_string(), fmt);
                        }
                    });
                ui.end_row();

                ui.label("Log level:");
                egui::ComboBox::from_id_source("loglevel")
                    .width(280.0)
                    .selected_text(&self.config.log_level)
                    .show_ui(ui, |ui| {
                        for lvl in ["error", "warn", "info", "debug", "trace"] {
                            ui.selectable_value(&mut self.config.log_level, lvl.to_string(), lvl);
                        }
                    });
                ui.end_row();
            });

            ui.add_space(8.0);
            ui.checkbox(&mut self.config.notifications, "Enable desktop notifications");
            
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Leave path fields empty to auto-detect bundled/PATH binaries.").color(egui::Color32::from_rgb(115, 115, 115)));
            
            if let Some(err) = &self.error_msg {
                ui.label(egui::RichText::new(err).color(egui::Color32::RED));
            }

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Open Config Folder").clicked() {
                    if let Some(dir) = Config::config_dir() {
                        let _ = std::process::Command::new("explorer")
                            .arg(dir.to_string_lossy().as_ref())
                            .spawn();
                    }
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save").clicked() { save_clicked = true; }
                    if ui.button("Cancel").clicked() { cancel_clicked = true; }
                });
            });

            if save_clicked {
                match self.config.save() {
                    Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.error_msg = Some(format!("Save failed: {}", e)),
                }
            }
            if cancel_clicked {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
