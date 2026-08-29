// src/app_icon.rs
// Loads the embedded application icon for eframe/egui windows.

use eframe::egui;

/// Return the application icon data for use with `ViewportBuilder::with_icon()`.
///
/// Embeds the 32×32 PNG at compile time and decodes it on first call.
pub fn load_icon() -> egui::IconData {
    let png_bytes = include_bytes!("../ico/32.png");
    let img = image::load_from_memory(png_bytes)
        .expect("Failed to decode embedded icon PNG")
        .into_rgba8();
    let (w, h) = img.dimensions();
    egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    }
}
