//! High level functionality for launching a simulation.
use std::path::Path;

/// Run the simulation
///
/// Arguments:
///
/// * `settings_file_path`: The path to the TOML file containing the model's configuration
pub fn run(settings_file_path: &Path) {
    // Placeholder code
    println!("Config file: {}", settings_file_path.to_str().unwrap())
}