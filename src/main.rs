#![windows_subsystem = "windows"]
mod benchmark;
mod compression;
mod decompression;
mod ui;

use ui::FlxApp;

#[tokio::main]
async fn main() -> Result<(), eframe::Error> {
    // Récupération de la poignée du runtime Tokio actif
    let rt_handle = tokio::runtime::Handle::current();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([750.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "FLX Archiver",
        native_options,
        Box::new(move |cc| Ok(Box::new(FlxApp::new(cc, rt_handle)))),
    )
}