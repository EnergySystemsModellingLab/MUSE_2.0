//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use crate::agent::{Agent, ObjectiveType};
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Capacity, Flow};
use anyhow::Result;
use indexmap::IndexSet;
use itertools::chain;
use log::info;
use std::collections::HashMap;

pub mod appraisal;
use appraisal::{appraise_investment, AppraisalOutput};

/// A map of demand across time slices for a specific commodity and region
type DemandMap = HashMap<TimeSliceID, Flow>;

/// Demand for a given combination of commodity, region and time slice
type AllDemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `flow_map` - Map of commodity flows
/// * `reduced_costs` - Reduced costs for assets
/// * `assets` - The asset pool
/// * `year` - Current milestone year
pub fn perform_agent_investment(
    model: &Model,
    flow_map: &FlowMap,
    reduced_costs: &ReducedCosts,
    assets: &mut AssetPool,
    year: u32,
) -> Result<()> {
    info!("Performing agent investment...");

    // Get all existing assets and clear pool
    let existing_assets = assets.take();

    // We consider SVD commodities first
    let commodities_of_interest = model
        .commodities
        .iter()
        .filter(|(_, commodity)| commodity.kind == CommodityType::ServiceDemand)
        .map(|(id, _)| id.clone())
        .collect();
    let demand = get_demand_profile(&commodities_of_interest, flow_map);

    for commodity_id in commodities_of_interest.iter() {
        let time_slice_level = model
            .commodities
            .get(commodity_id)
            .unwrap()
            .time_slice_level;
        for agent in get_responsible_agents(model.agents.values(), commodity_id, year) {
            let objective_type = agent.objectives.get(&year).unwrap();

            for region_id in agent.regions.iter() {
                // Maximum capacity for candidate assets
                let max_capacity =
                    get_maximum_candidate_capacity(model, &demand, commodity_id, region_id);

                // Existing and candidate assets from which to choose
                let opt_assets = get_asset_options(
                    &existing_assets,
                    agent,
                    commodity_id,
                    region_id,
                    year,
                    max_capacity,
                )
                .collect();

                let demand_for_commodity = get_demand_for_commodity(
                    &model.time_slice_info,
                    &demand,
                    commodity_id,
                    region_id,
                );

                // Choose assets from among existing pool and candidates
                let best_assets = select_best_assets(
                    opt_assets,
                    objective_type,
                    reduced_costs,
                    &demand_for_commodity,
                    &model.time_slice_info,
                    time_slice_level,
                )?;

                // Add assets to pool
                assets.extend(best_assets);
            }
        }
    }

    Ok(())
}

/// Get demand per time slice for specified commodities
fn get_demand_profile(commodities: &IndexSet<CommodityID>, flow_map: &FlowMap) -> AllDemandMap {
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

/// Get part of the demand profile for this commodity and region
fn get_demand_for_commodity(
    time_slice_info: &TimeSliceInfo,
    demand: &AllDemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
) -> DemandMap {
    time_slice_info
        .iter_ids()
        .map(|time_slice| {
            (
                time_slice.clone(),
                *demand
                    .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                    .unwrap(),
            )
        })
        .collect()
}

/// Get the agents responsible for a given commodity in a given year
fn get_responsible_agents<'a, I>(
    agents: I,
    commodity_id: &'a CommodityID,
    year: u32,
) -> impl Iterator<Item = &'a Agent>
where
    I: Iterator<Item = &'a Agent>,
{
    agents.filter(move |agent| {
        agent
            .commodity_portions
            .contains_key(&(commodity_id.clone(), year))
    })
}

/// Get the maximum candidate asset capacity
fn get_maximum_candidate_capacity(
    model: &Model,
    demand: &AllDemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
) -> Capacity {
    model.parameters.capacity_limit_factor
        * get_peak_demand(&model.time_slice_info, demand, commodity_id, region_id)
}

/// Get the peak demand for this commodity
fn get_peak_demand(
    time_slice_info: &TimeSliceInfo,
    demand: &AllDemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
) -> Flow {
    *time_slice_info
        .iter_ids()
        .map(|time_slice| {
            demand
                .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                .unwrap()
        })
        .max_by(|a, b| a.total_cmp(b))
        .unwrap()
}

/// Get options from existing and potential assets for the given parameters
fn get_asset_options<'a>(
    all_existing_assets: &'a [AssetRef],
    agent: &'a Agent,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> impl Iterator<Item = AssetRef> + 'a {
    // Get existing assets which produce the commodity of interest
    let existing_assets = all_existing_assets
        .iter()
        .filter_agent(&agent.id)
        .filter_region(region_id)
        .filter_primary_producers_of(commodity_id)
        .cloned();

    // Get candidates assets which produce the commodity of interest
    let candidate_assets = get_candidate_assets(
        agent,
        region_id,
        commodity_id,
        year,
        candidate_asset_capacity,
    );

    chain(existing_assets, candidate_assets)
}

/// Get candidate assets which produce a particular commodity for a given agent
fn get_candidate_assets<'a>(
    agent: &'a Agent,
    region_id: &'a RegionID,
    commodity_id: &'a CommodityID,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> impl Iterator<Item = AssetRef> + 'a {
    let flows_key = (region_id.clone(), year);

    // Get all the processes which produce the commodity in this year
    let producers = agent
        .search_space
        .get(&(commodity_id.clone(), year))
        .unwrap()
        .iter()
        .filter(move |process| {
            process
                .flows
                .get(&flows_key)
                .unwrap()
                .get(commodity_id)
                .unwrap()
                .is_output()
        });

    producers.map(move |process| {
        Asset::new(
            Some(agent.id.clone()),
            process.clone(),
            region_id.clone(),
            candidate_asset_capacity,
            year,
        )
        .unwrap()
        .into()
    })
}

/// Get the best assets for meeting demand for the given commodity
fn select_best_assets(
    opt_assets: Vec<AssetRef>,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<impl Iterator<Item = AssetRef>> {
    // **TODO:** Loop while demand is unmet
    let mut current_best: Option<AppraisalOutput> = None;
    for asset in opt_assets.iter() {
        // Investment appraisal
        let output = appraise_investment(
            asset,
            objective_type,
            reduced_costs,
            demand,
            time_slice_info,
            time_slice_level,
        )?;

        if current_best
            .as_ref()
            .is_none_or(|best_output| output.is_better_than(best_output))
        {
            current_best = Some(output);
        }
    }

    let best_output = current_best.expect("No assets given");
    let mut best_asset = best_output.asset;

    drop(opt_assets); // drop so there's (probably) only one reference to best_asset

    // If a candidate asset, we need to set the capacity
    if !best_asset.is_commissioned() {
        best_asset.make_mut().capacity = best_output.capacity;
    }

    // Just return this one asset for now
    Ok(std::iter::once(best_asset))
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
