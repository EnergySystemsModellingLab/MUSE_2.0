//! High level functionality for launching the simulation.
pub mod log;
pub mod model;
pub mod settings;

mod asset;
mod commodity;
mod demand;
mod dispatch_optimisation;
mod input;
mod process;
mod region;
mod time_slice;

use dispatch_optimisation::run_dispatch_optimisation;

use crate::model::Model;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    // TODO: Remove this once we're actually doing something with these values
    // println!("Commodities: {:?}", model.commodities);
    // println!("Regions: {:?}", model.regions);
    // println!("Demand data: {:?}", model.demand_data);
    // println!("Processes: {:?}", model.processes);
    // println!("Assets: {:?}", model.assets_by_region);
    // println!("Time slices: {:?}", model.time_slices);
    // println!("Milestone years: {:?}", model.milestone_years);

    run_dispatch_optimisation(model);
}
