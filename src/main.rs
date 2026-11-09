//! SeEx: the clipboard monitor and EDA library exporter.
//!
//! The window is drawn with GPUI; everything under `src/ui/` is the front end,
//! and the pipeline modules below do the EasyEDA/LCSC → KiCad/Altium work.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod altium;
mod batch;
mod bom;
mod cadence;
mod cli;
mod config;
mod easyeda;
mod error;
mod exporter;
mod extract;
mod kicad;
mod lceda;
mod lcsc;
mod metadata;
mod monitor;
mod progress;
pub mod server;
mod step_cleaner;
mod step_logo;
mod step_model;
mod ui;
mod util;

fn main() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(
            windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
        );
    }

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        if let Err(err) = cli::run_cli() {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(target_os = "linux")]
    if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
        println!("No graphical display detected ($DISPLAY / $WAYLAND_DISPLAY not set).");
        println!("Starting SeEx in Web Server mode on http://0.0.0.0:8080 ...");
        if let Err(e) = server::run_server("0.0.0.0".to_string(), 8080, false, None) {
            eprintln!("Server error: {e}");
            std::process::exit(1);
        }
        return;
    }

    ui::run();
}
