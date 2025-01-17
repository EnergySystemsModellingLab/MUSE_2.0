//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use log::info;

/// Get an iterator of active [`Asset`]s for the specified milestone year.
fn filter_assets(assets: &AssetPool, year: u32) -> impl Iterator<Item = &Asset> {
    assets
        .iter()
        .filter(move |asset| asset.commission_year >= year)
}

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: &Model, assets: &AssetPool) {
    for year in model.iter_years() {
        info!("Milestone year: {year}");
        for asset in filter_assets(assets, year) {
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
