//! High level functionality for launching a simulation.
use crate::settings::Settings;
use polars::prelude::*;

/// Run the simulation
///
/// # Arguments:
///
/// * `settings`: The model settings
pub fn run(settings: &Settings) {
    // Print the contents of settings
    // TODO: Remove this once we're actually doing something with the settings
    println!("{}", settings.process_info);
    // println!("Demand data: {:?}", settings.demand_data);
    // println!("Time slices: {:?}", settings.time_slices);
    // println!("Milestone years: {:?}", settings.milestone_years);

    let avail_for_windfarms = settings
        .process_info
        .availabilities
        .clone()
        .lazy()
        .filter(col("process_id").eq(lit("WNDFRM")))
        .select([col("timeslice")])
        .collect()
        .unwrap();

    println!("Result of query: {}", avail_for_windfarms);
}
