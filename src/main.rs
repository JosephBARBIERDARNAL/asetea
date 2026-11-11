#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod ase;
mod canvas;
mod dialogs;
mod doc;
mod draw;
mod export;
mod palettes;
mod panels;
#[cfg(test)]
mod tests;

use eframe::egui;

fn main() -> eframe::Result {
    let files: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("asetea")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([760.0, 520.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("asetea", options, Box::new(|cc| Ok(Box::new(app::App::new(cc, files)))))
}
