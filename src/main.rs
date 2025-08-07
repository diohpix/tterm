#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod types;
mod tab_manager;
mod split_manager;
mod grid_manager;
mod broadcast_manager;
mod input_handler;
mod ui_renderer;
mod ime;
pub mod ipc;

fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "full_screen_example",
        native_options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
