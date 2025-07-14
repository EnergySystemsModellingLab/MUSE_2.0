//! Code for performing agent investment.
use super::lcox::{calculate_lcox, AppraisalOutput};
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use crate::agent::{Agent, ObjectiveType};
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::simulation::optimisation::perform_dispatch_optimisation;
use crate::simulation::prices::get_prices_and_reduced_costs;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Capacity, Flow};
use anyhow::{ensure, Context, Result};
use indexmap::IndexSet;
use itertools::{chain, iproduct};
use log::info;
use std::collections::HashMap;

/// Demand for a given combination of commodity, region and time slice
type DemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `year` - Current milestone year
/// * `assets` - The asset pool
/// * `flow_map` - Map of commodity flows
/// * `reduced_costs` - Reduced costs for assets
pub fn perform_agent_investment(
    model: &Model,
    year: u32,
    assets: &mut AssetPool,
    flow_map: &mut FlowMap,
    reduced_costs: &mut ReducedCosts,
) -> Result<()> {
    info!("Performing agent investment...");

    // New asset pool
    let mut new_pool = Vec::new();

    // We consider SVD commodities first
    let mut commodities_of_interest: IndexSet<_> = model
        .commodities
        .iter()
        .filter(|(_, commodity)| commodity.kind == CommodityType::ServiceDemand)
        .map(|(id, _)| id.clone())
        .collect();
    let mut already_seen_commodities = commodities_of_interest.clone();

    loop {
        let demand = get_demand_profile(&commodities_of_interest, flow_map);

        // Select new/existing assets to meet demand for given commodities
        let chosen_assets = select_assets_producing_commodities(
            &commodities_of_interest,
            model,
            assets,
            reduced_costs,
            &demand,
            year,
        )?;

        // Get commodities of interest for next iteration
        commodities_of_interest = iter_commodities_consumed_by(&chosen_assets).collect();

        // Check that there are no dependency loops between commodities
        ensure!(
            commodities_of_interest.is_disjoint(&already_seen_commodities),
            "There is a demand loop between commodities. This is not permitted."
        );
        already_seen_commodities.extend(commodities_of_interest.iter().cloned());

        // Add chosen assets to new asset pool
        new_pool.extend(chosen_assets);

        // If there are no more commodities of interest, we've finished
        if commodities_of_interest.is_empty() {
            break;
        }

        // Perform dispatch optimisation with assets that have been selected so far
        let solution = perform_dispatch_optimisation(model, &new_pool, &[], year)?;
        *flow_map = solution.create_flow_map();
        let (_cur_prices, cur_reduced_costs) =
            get_prices_and_reduced_costs(model, &solution, &new_pool, year);
        *reduced_costs = cur_reduced_costs;
    }

    // Replace pool of active assets with the new one
    assets.replace_active_pool(new_pool);

    Ok(())
}

/// Get the commodities consumed by the specified assets
fn iter_commodities_consumed_by<'a>(
    assets: &'a [AssetRef],
) -> impl Iterator<Item = CommodityID> + 'a {
    assets.iter().flat_map(|asset| {
        asset
            .get_flows_map()
            .values()
            .filter_map(|flow| flow.is_input().then_some(flow.commodity.id.clone()))
    })
}

/// Get demand per time slice for specified commodities
fn get_demand_profile(commodities: &IndexSet<CommodityID>, flow_map: &FlowMap) -> DemandMap {
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
    demand: &DemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
) -> HashMap<TimeSliceID, Flow> {
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

/// Get new assets to meet demand for given commodities
fn select_assets_producing_commodities(
    commodities: &IndexSet<CommodityID>,
    model: &Model,
    assets: &AssetPool,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    year: u32,
) -> Result<Vec<AssetRef>> {
    let mut chosen_assets = Vec::new();
    for (commodity_id, region_id) in iproduct!(commodities.iter(), model.iter_regions()) {
        for agent in get_responsible_agents(model.agents.values(), commodity_id, region_id, year) {
            let objective_type = agent.objectives.get(&year).unwrap();

            // Maximum capacity for candidate assets
            let max_capacity =
                get_maximum_candidate_capacity(model, demand, commodity_id, region_id);

            // Existing and candidate assets from which to choose
            let opt_assets =
                get_asset_options(assets, agent, commodity_id, region_id, year, max_capacity)
                    .collect();

            let demand_for_commodity =
                get_demand_for_commodity(&model.time_slice_info, demand, commodity_id, region_id);

            // Choose assets from among existing pool and candidates
            let chosen_assets_for_agent = select_best_assets(
                reduced_costs,
                opt_assets,
                demand_for_commodity,
                objective_type,
            )
            .with_context(|| {
                format!(
                    "Failed to meet demand for commodity '{commodity_id}' in region '{region_id}'"
                )
            })?;

            chosen_assets.extend(chosen_assets_for_agent);
        }
    }

    Ok(chosen_assets)
}

/// Get the agents responsible for a given commodity in a given year
fn get_responsible_agents<'a, I>(
    agents: I,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
) -> impl Iterator<Item = &'a Agent>
where
    I: Iterator<Item = &'a Agent>,
{
    agents.filter(move |agent| {
        agent.regions.contains(region_id)
            && agent
                .commodity_portions
                .contains_key(&(commodity_id.clone(), year))
    })
}

/// Get the maximum candidate asset capacity
fn get_maximum_candidate_capacity(
    model: &Model,
    demand: &DemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
) -> Capacity {
    model.parameters.capacity_limit_factor
        * get_peak_demand(&model.time_slice_info, demand, commodity_id, region_id)
}

/// Get the peak demand for this commodity
fn get_peak_demand(
    time_slice_info: &TimeSliceInfo,
    demand: &DemandMap,
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
    assets: &'a AssetPool,
    agent: &'a Agent,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> impl Iterator<Item = AssetRef> + 'a {
    // Get existing assets which produce the commodity of interest
    let existing_assets = assets
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
    reduced_costs: &ReducedCosts,
    mut opt_assets: Vec<AssetRef>,
    mut demand: HashMap<TimeSliceID, Flow>,
    objective_type: &ObjectiveType,
) -> Option<Vec<AssetRef>> {
    let mut chosen_assets: Vec<AssetRef> = Vec::new();

    while is_remaining_unmet_demand(&demand) {
        // If there are no assets remaining, we were unable to meet the demand
        if opt_assets.is_empty() {
            return None;
        }

        let mut current_best: Option<(&AssetRef, AppraisalOutput)> = None;
        for asset in opt_assets.iter() {
            // Investment appraisal
            let output = appraise_investment(asset, reduced_costs, &demand, objective_type);

            if current_best
                .as_ref()
                .is_none_or(|(_, best_output)| output.cost_index < best_output.cost_index)
            {
                current_best = Some((asset, output));
            }
        }

        let (chosen_asset, chosen_output) = current_best.unwrap();
        demand = chosen_output.unmet_demand;

        update_assets(
            chosen_asset.clone(),
            chosen_output.capacity,
            &mut opt_assets,
            &mut chosen_assets,
        );
    }

    Some(chosen_assets)
}

/// Check whether there is any remaining demand that is unmet in any time slice
fn is_remaining_unmet_demand(demand: &HashMap<TimeSliceID, Flow>) -> bool {
    demand.values().any(|flow| *flow > Flow(0.0))
}

/// Update capacity of chosen asset, if needed, and update both asset options and chosen assets
fn update_assets(
    mut chosen_asset: AssetRef,
    new_capacity: Option<Capacity>,
    opt_assets: &mut Vec<AssetRef>,
    chosen_assets: &mut Vec<AssetRef>,
) {
    // New capacity given for candidates only
    if let Some(new_capacity) = new_capacity {
        // Get a reference to the copy of the asset in opt_assets
        let (old_idx, old) = opt_assets
            .iter_mut()
            .enumerate()
            .find(|(_, asset)| **asset == chosen_asset)
            .unwrap();

        // Remove this capacity from the available remaining capacity for this asset
        old.make_mut().capacity -= new_capacity;

        // If there's no capacity remaining, remove the asset from the options
        if old.capacity <= Capacity(0.0) {
            opt_assets.swap_remove(old_idx);
        }

        if let Some(existing_asset) = chosen_assets
            .iter_mut()
            .find(|asset| **asset == chosen_asset)
        {
            // Add the additional required capacity
            existing_asset.make_mut().capacity += new_capacity;
        } else {
            // Update the capacity of the chosen asset
            chosen_asset.make_mut().capacity = new_capacity;

            chosen_assets.push(chosen_asset);
        };
    } else {
        // Remove this asset from the options
        opt_assets.retain(|asset| *asset != chosen_asset);

        chosen_assets.push(chosen_asset);
    }
}

fn appraise_investment(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    objective_type: &ObjectiveType,
) -> AppraisalOutput {
    match objective_type {
        ObjectiveType::LevelisedCostOfX => calculate_lcox(asset, reduced_costs, demand),
    }
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
