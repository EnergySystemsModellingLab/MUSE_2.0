//! Provides the main entry point to the program.
use ::log::info;
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
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
    let settings = Settings::from_path(&model_dir);

    // Set the program log level
    log::init(settings.log_level.as_deref());
    log_panics::init(); // Write panic info to logger rather than stderr

    let model = Model::from_path(&model_dir);
    info!("Model loaded successfully.");

    // Run simulation
    muse2::run(&model)
}
