//! Code for performing agent investment.
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::model::Model;
use log::info;
use std::collections::HashSet;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `solution` - The solution to the dispatch optimisation
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
pub fn perform_agent_investment(
    _model: &Model,
    solution: &Solution,
    _prices: &CommodityPrices,
    assets: &mut AssetPool,
) {
    info!("Performing agent investment...");

    let mut assets_to_keep = HashSet::new();
    for (asset_id, _commodity_id, _time_slice, _flow) in solution.iter_commodity_flows_for_assets()
    {
        if assets.get(asset_id).is_none() {
            // Asset has been decommissioned
            continue;
        }

        // **TODO**: Implement agent investment. For now, just keep all assets.
        assets_to_keep.insert(asset_id);
    }

    // Decommission non-selected assets
    assets.retain(&assets_to_keep);
}
