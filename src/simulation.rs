//! Functionality for running the MUSE 2.0 simulation.
use crate::model::Model;
use log::info;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    for year in model.iter_years() {
        info!("* Milestone year: {year}");
        perform_dispatch(model, year);
    }
}

fn perform_dispatch(model: &Model, year: u32) {
    info!("(Performing dispatch...)");

    for region_id in model.iter_regions() {
        info!("** Region: {region_id}");
        for asset in model.get_assets(year, region_id) {
            info!(
                "*** Agent {} has asset {} (commissioned in {})",
                asset.agent_id, asset.process.id, asset.commission_year
            );

            for flow in asset.process.flows.iter() {
                info!("**** Commodity: {}", flow.commodity.id);
            }
        }
    }
}
