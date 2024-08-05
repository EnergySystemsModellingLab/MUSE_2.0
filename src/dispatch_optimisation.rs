use crate::model::Model;
use log::info;

pub fn run_dispatch_optimisation(model: &Model) {
    info!("Starting dispatch optimisation");

    for year in model.iter_years() {
        info!("Year: {year}");
        for time_slice in model.iter_time_slices() {
            info!("Time slice: {time_slice}");

            for region_id in model.regions.keys() {
                info!("Looking at region {region_id}");

                for asset in model.assets_by_region.get(region_id).unwrap().iter() {
                    if asset.commission_year < year {
                        // Asset hasn't been commissioned yet
                        continue;
                    }

                    info!(
                        "Asset: process {}, agent {}, capacity {}, commission year {}",
                        asset.process.id, asset.agent_id, asset.capacity, asset.commission_year
                    );

                    let pac = asset.process.pacs.first().unwrap();
                    info!("First PAC: {} ({})", pac.id, pac.description);
                }
            }
        }
    }
}
