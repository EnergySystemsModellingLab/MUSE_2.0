//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use super::CommodityPrices;
use crate::asset::AssetPool;
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::Flow;
use indexmap::IndexSet;
use log::info;
use std::collections::HashMap;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `assets` - The asset pool
/// * `year` - Current milestone year
pub fn perform_agent_investment(
    model: &Model,
    flow_map: &FlowMap,
    _prices: &CommodityPrices,
    _reduced_costs: &ReducedCosts,
    _assets: &AssetPool,
    _year: u32,
) {
    info!("Performing agent investment...");

    // We consider SVD commodities first
    let commodities_of_interest = model
        .commodities
        .iter()
        .filter(|(_, commodity)| commodity.kind == CommodityType::ServiceDemand)
        .map(|(id, _)| id.clone())
        .collect();
    let _demand = get_demand_profile(&commodities_of_interest, flow_map);

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
}

/// Get demand per time slice for specified commodities
pub fn get_demand_profile(
    commodities: &IndexSet<CommodityID>,
    flow_map: &FlowMap,
) -> HashMap<(CommodityID, RegionID, TimeSliceID), Flow> {
    let mut map = HashMap::new();
    for ((asset, commodity_id, time_slice), &flow) in flow_map.iter() {
        if commodities.contains(commodity_id) && flow > Flow(0.0) {
            map.entry((
                commodity_id.clone(),
                asset.region_id.clone(),
                time_slice.clone(),
            ))
            .and_modify(|value| *value += flow)
            .or_insert(flow);
        }
    }

    map
}
