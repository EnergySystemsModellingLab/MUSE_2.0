//! High level functionality for launching a simulation.
use crate::settings::Settings;

/// Run the simulation
///
/// # Arguments:
///
/// * `settings`: The model settings
pub fn run(settings: &Settings) {
    // Print the contents of settings
    // TODO: Remove this once we're actually doing something with the settings
    println!("Regions: {:?}", settings.regions);
    println!("Demand data: {:?}", settings.demand_data);
    println!("Processes: {:?}", settings.processes);
    println!("Time slices: {:?}", settings.time_slices);
    println!("Milestone years: {:?}", settings.milestone_years);
}
