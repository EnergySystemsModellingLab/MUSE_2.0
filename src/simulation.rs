//! High level functionality for launching a simulation.
use crate::model::Model;

/// Run the simulation
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    // TODO: Remove this once we're actually doing something with these values
    println!("Regions: {:?}", model.regions);
    println!("Demand data: {:?}", model.demand_data);
    println!("Processes: {:?}", model.processes);
    println!("Time slices: {:?}", model.time_slices);
    println!("Milestone years: {:?}", model.milestone_years);
}
