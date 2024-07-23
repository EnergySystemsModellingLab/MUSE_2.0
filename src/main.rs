//! Provides the main entry point to the program.

mod demand;
mod input;
mod log;
mod model;
mod process;
mod region;
mod settings;
mod simulation;
mod time_slice;

use ::log::info;
use model::Model;
use settings::Settings;
use std::env;
use std::path::PathBuf;

/// The main entry point to the program
fn main() {
    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model folder.");
    }
    let model_dir = PathBuf::from(args[1].as_str());

    // Read program settings
    let settings = Settings::from_path(&model_dir)
        .unwrap_or_else(|err| panic!("Failed to load program settings: {}", err));

    // Set the program log level
    log::init(settings.log_level.as_deref());
    log_panics::init(); // Write panic info to logger rather than stderr

    let model =
        Model::from_path(&model_dir).unwrap_or_else(|err| panic!("Failed to load model: {}", err));

    info!("Model loaded successfully.");

    // Run simulation
    simulation::run(&model)
}
