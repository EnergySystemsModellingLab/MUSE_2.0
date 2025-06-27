//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::model::Model;
use log::info;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
pub fn perform_agent_investment(
    _model: &Model,
    _flow_map: &FlowMap,
    _prices: &CommodityPrices,
    _assets: &mut AssetPool,
) {
    info!("Performing agent investment...");

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}
