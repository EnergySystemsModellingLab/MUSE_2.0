//! High level functionality for launching the simulation.
#![warn(missing_docs)]
pub mod agent;
pub mod commands;
pub mod commodity;
pub mod demand;
pub mod input;
pub mod log;
pub mod model;
pub mod process;
pub mod region;
pub mod settings;
pub mod time_slice;

use crate::model::Model;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    // TODO: Remove this once we're actually doing something with these values
    println!("Commodities: {:?}", model.commodities);
    println!("Regions: {:?}", model.regions);
    println!("Processes: {:?}", model.processes);
    println!("Time slices: {:?}", model.time_slice_info);
    println!("Milestone years: {:?}", model.milestone_years);
}
