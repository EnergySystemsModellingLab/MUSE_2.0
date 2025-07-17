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

pub mod appraisal;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{Asset, AssetRef};
    use crate::commodity::CommodityID;
    use crate::fixture::{asset, commodity_id, region_id, time_slice};
    use crate::units::Flow;
    use rstest::rstest;
    use std::collections::HashMap;

    #[rstest]
    fn test_get_demand_profile(
        commodity_id: CommodityID,
        region_id: RegionID,
        time_slice: TimeSliceID,
        asset: Asset,
    ) {
        // Setup test commodities
        let mut commodities = IndexSet::new();
        commodities.insert(commodity_id.clone());

        // Setup test asset and AssetRef
        let asset_ref1 = AssetRef::from(asset.clone());
        // Create a second asset with the same region, commodity, and time_slice
        let mut asset2 = asset.clone();
        asset2.id = None; // Ensure it's treated as a different asset
        asset2.commission_year += 1; // Make it unique
        let asset_ref2 = AssetRef::from(asset2);

        let mut flow_map = FlowMap::new();
        flow_map.insert(
            (asset_ref1.clone(), commodity_id.clone(), time_slice.clone()),
            Flow(10.0),
        );
        flow_map.insert(
            (asset_ref2.clone(), commodity_id.clone(), time_slice.clone()),
            Flow(7.0),
        );
        flow_map.insert(
            (
                asset_ref1.clone(),
                CommodityID("C2".to_string().into()),
                time_slice.clone(),
            ),
            Flow(5.0),
        ); // Should be ignored
        flow_map.insert(
            (
                asset_ref1.clone(),
                commodity_id.clone(),
                crate::time_slice::TimeSliceID {
                    season: "summer".into(),
                    time_of_day: "night".into(),
                },
            ),
            Flow(0.0),
        ); // Should be ignored

        // Call get_demand_profile
        let result = get_demand_profile(&commodities, &flow_map);

        // Check result
        let mut expected = HashMap::new();
        expected.insert(
            (commodity_id.clone(), region_id.clone(), time_slice.clone()),
            Flow(17.0),
        );
        assert_eq!(result, expected);
    }
}
