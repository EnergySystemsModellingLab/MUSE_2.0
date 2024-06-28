//! High level functionality for launching a simulation.
use crate::settings::read_settings;
use crate::time_slices::read_time_slices;
use std::path::Path;

/// Run the simulation
///
/// Arguments:
///
/// * `settings_file_path`: The path to the TOML file containing the model's configuration
/// * `time_slices_file_path`: The path to the time_slices.csv file
pub fn run(settings_file_path: &Path, time_slices_file_path: &Path) {
    // Read and process the settings file
    let settings = read_settings(settings_file_path)
        .unwrap_or_else(|err| panic!("Failed to load settings: {}", err));

    // Example usage: Accessing the milestone years
    println!("Milestone Years: {:?}", settings.milestone_years.years);

    // Use settings as needed for your simulation or other functionality
    dbg!(settings);

    // Placeholder code for time slices
    let time_slices = read_time_slices(time_slices_file_path)
        .unwrap_or_else(|err| {
            panic!(
                "Failed to read time slices file {:?}: {:?}",
                time_slices_file_path, err
            )
        })
        .unwrap();
    dbg!(time_slices);
}
