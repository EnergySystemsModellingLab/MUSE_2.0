use crate::model::Model;
use log::info;

pub fn run_dispatch_optimisation(model: &Model) {
    info!("Starting dispatch optimisation");

    for region_id in model.regions.keys() {
        info!("Looking at region {region_id}");

        for asset in model.assets_by_region.get(region_id).unwrap().iter() {
            info!(
                "Asset: process {}, agent {}, capacity {}, commission year {}",
                asset.process.id, asset.agent_id, asset.capacity, asset.commission_year
            );

            let pac = asset.process.pacs.first().unwrap();
            info!("First PAC: {pac}")
        }
    }
}
