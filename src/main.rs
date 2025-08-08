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
pub mod session;

use std::env;
use uuid::Uuid;

fn main() -> eframe::Result {
    env_logger::init();
    
    // Set up panic handler to catch any issues
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("Location: {}:{}:{}", location.file(), location.line(), location.column());
        }
        if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
            eprintln!("Payload: {}", payload);
        }
        // Exit explicitly to avoid hanging
        std::process::exit(1);
    }));
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let mut attach_session_id: Option<Uuid> = None;
    
    // Check for --attach-session argument
    if args.len() >= 3 && args[1] == "--attach-session" {
        match Uuid::parse_str(&args[2]) {
            Ok(session_id) => {
                attach_session_id = Some(session_id);
                log::info!("Starting with attach to session: {:?}", session_id);
            }
            Err(e) => {
                log::error!("Invalid session ID: {}", e);
                std::process::exit(1);
            }
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0]),
        ..Default::default()
    };

    eframe::run_native(
        "TTerminal",
        native_options,
        Box::new(move |cc| Ok(Box::new(app::App::new_with_session(cc, attach_session_id)))),
    )
}
