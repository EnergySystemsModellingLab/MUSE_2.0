//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use log::info;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: &Model, assets: &mut AssetPool) {
    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Commission new assets from user-supplied pool
        assets.commission_new(year);

        for asset in assets.iter() {
            info!(
                "├── Agent: {}; region: {}; process: {} (commissioned {})",
                asset.agent_id, asset.region_id, asset.process.id, asset.commission_year
            );

            for flow in asset.process.flows.iter() {
                info!("│   ├── Commodity: {}", flow.commodity.id);
            }
        }
    }
}
