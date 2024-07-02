//! High level functionality for launching a simulation.
use crate::settings::read_settings;
use std::path::Path;

/// Run the simulation
///
/// # Arguments:
///
/// * `settings_file_path`: The path to the TOML file containing the model's configuration
pub fn run(settings_file_path: &Path) {
    println!("Loading settings from {:?}", settings_file_path);

    // Read and process the settings file
    let settings = read_settings(settings_file_path)
        .unwrap_or_else(|err| panic!("Failed to load settings: {}", err));

    // Print the contents of settings
    // TODO: Remove this once we're actually doing something with the settings
    println!("Time slices: {:?}", settings.time_slices);
    println!("Milestone years: {:?}", settings.milestone_years);
}
