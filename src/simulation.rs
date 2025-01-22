//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use log::info;
use std::rc::Rc;

/// Get an iterator of active [`Asset`]s for the specified milestone year in a given region.
fn filter_assets<'a>(
    assets: &'a AssetPool,
    year: u32,
    region_id: &'a Rc<str>,
) -> impl Iterator<Item = &'a Asset> {
    assets
        .iter()
        .filter(move |asset| asset.commission_year >= year && asset.region_id == *region_id)
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
        for region_id in model.iter_regions() {
            info!("├── Region: {region_id}");
            for asset in filter_assets(assets, year, region_id) {
                info!(
                    "│   ├── Agent {} has asset {} (commissioned in {})",
                    asset.agent_id, asset.process.id, asset.commission_year
                );

                for flow in asset.process.flows.iter() {
                    info!("│   │   ├── Commodity: {}", flow.commodity.id);
                }
            }
        }
    }
}
