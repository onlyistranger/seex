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

    ui::run();
}
