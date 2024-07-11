//! Provides the main entry point to the program.

mod demand;
mod input;
mod log;
mod settings;
mod simulation;
mod time_slices;

use crate::settings::SettingsReader;
use ::log::info;
use std::env;

/// The main entry point to the program
fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model configuration TOML file.");
    }

    // Read settings TOML file
    let reader = SettingsReader::from_path(&args[1])
        .unwrap_or_else(|err| panic!("Failed to parse TOML file: {}", err));

    // Set the program log level
    log::init(reader.log_level());
    log_panics::init(); // Write panic info to logger rather than stderr

    // Load settings from CSV files and verify
    let settings = reader
        .into_settings()
        .unwrap_or_else(|err| panic!("Failed to load settings: {}", err));

    info!("Settings loaded successfully.");

    // Run simulation
    simulation::run(&settings)
}
