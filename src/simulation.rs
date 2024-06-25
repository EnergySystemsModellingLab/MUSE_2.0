//! High level functionality for launching a simulation.
use crate::time_slices::read_time_slices;
use std::path::Path;

/// Run the simulation
///
/// Arguments:
///
/// * `settings_file_path`: The path to the TOML file containing the model's configuration
/// * `time_slices_file_path`: The path to the time_slices.csv file
pub fn run(settings_file_path: &Path, time_slices_file_path: &Path) {
    // Placeholder code
    println!("Config file: {}", settings_file_path.to_str().unwrap());

    // Placeholder code for time slices
    let time_slices = read_time_slices(time_slices_file_path).unwrap_or_else(|err| {
        panic!(
            "Failed to read time slices file {:?}: {:?}",
            time_slices_file_path, err
        )
    });
    dbg!(time_slices);
}
