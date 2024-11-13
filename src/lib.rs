//! High level functionality for launching the simulation.
pub mod log;
pub mod model;
pub mod settings;

mod agent;
mod commodity;
mod demand;
mod input;
mod process;
mod region;
mod time_slice;

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
