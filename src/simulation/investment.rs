//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use crate::agent::{Agent, ObjectiveType};
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{Commodity, CommodityID};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Capacity, Dimensionless, Flow, FlowPerCapacity};
use anyhow::{ensure, Result};
use itertools::chain;
use log::{debug, info};
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

    // Demand profile for commodities
    let demand = get_demand_profile(flow_map);

    // We consider SVD commodities first
    for region_id in model.iter_regions() {
        for commodity_id in model.commodity_order[&(region_id.clone(), year)].iter() {
            let commodity = &model.commodities[commodity_id];
            for (agent, commodity_portion) in
                get_responsible_agents(model.agents.values(), commodity_id, region_id, year)
            {
                debug!(
                    "Running investment for agent '{}' with commodity '{}' in region '{}'",
                    &agent.id, commodity_id, region_id
                );

                let demand_for_commodity = get_demand_portion_for_commodity(
                    &model.time_slice_info,
                    &demand,
                    commodity_id,
                    region_id,
                    commodity_portion,
                );

                // Existing and candidate assets from which to choose
                let opt_assets = get_asset_options(
                    &model.time_slice_info,
                    &existing_assets,
                    &demand_for_commodity,
                    agent,
                    commodity_id,
                    region_id,
                    year,
                )
                .collect();

                // Choose assets from among existing pool and candidates
                let best_assets = select_best_assets(
                    model,
                    opt_assets,
                    commodity,
                    &agent.objectives[&year],
                    reduced_costs,
                    demand_for_commodity,
                )?;

                // Add assets to pool
                assets.extend(best_assets);
            }
        }
    }

    // Decommission non-selected assets
    assets.decommission_if_not_active(existing_assets, year);

    Ok(())
}

/// Get demand per time slice for every commodity
fn get_demand_profile(flow_map: &FlowMap) -> AllDemandMap {
    let mut map = HashMap::new();
    for ((asset, commodity_id, time_slice), &flow) in flow_map.iter() {
        if flow > Flow(0.0) {
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

/// Get a portion of the demand profile for this commodity and region
fn get_demand_portion_for_commodity(
    time_slice_info: &TimeSliceInfo,
    demand: &AllDemandMap,
    commodity_id: &CommodityID,
    region_id: &RegionID,
    commodity_portion: Dimensionless,
) -> DemandMap {
    time_slice_info
        .iter_ids()
        .map(|time_slice| {
            (
                time_slice.clone(),
                commodity_portion
                    * *demand
                        .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
                        .unwrap_or(&Flow(0.0)),
            )
        })
        .collect()
}

/// Get the agents responsible for a given commodity in a given year along with the commodity
/// portion for which they are responsible
fn get_responsible_agents<'a, I>(
    agents: I,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
) -> impl Iterator<Item = (&'a Agent, Dimensionless)>
where
    I: Iterator<Item = &'a Agent>,
{
    agents.filter_map(move |agent| {
        if !agent.regions.contains(region_id) {
            return None;
        }
        let portion = agent
            .commodity_portions
            .get(&(commodity_id.clone(), year))?;

        Some((agent, *portion))
    })
}

/// Get the maximum required capacity across time slices
fn get_demand_limiting_capacity(
    time_slice_info: &TimeSliceInfo,
    asset: &Asset,
    commodity_id: &CommodityID,
    demand: &DemandMap,
) -> Capacity {
    let coeff = asset.get_flow(commodity_id).unwrap().coeff;
    let mut capacity = Capacity(0.0);
    for time_slice in time_slice_info.iter_ids() {
        let max_flow_per_cap = *asset.get_activity_per_capacity_limits(time_slice).end() * coeff;
        if max_flow_per_cap == FlowPerCapacity(0.0) {
            continue;
        }
        capacity = capacity.max(demand[time_slice] / max_flow_per_cap);
    }
    capacity
}

/// Get options from existing and potential assets for the given parameters
fn get_asset_options<'a>(
    time_slice_info: &'a TimeSliceInfo,
    all_existing_assets: &'a [AssetRef],
    demand: &'a DemandMap,
    agent: &'a Agent,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
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
        time_slice_info,
        demand,
        agent,
        region_id,
        commodity_id,
        year,
    );

    chain(existing_assets, candidate_assets)
}

/// Get candidate assets which produce a particular commodity for a given agent
fn get_candidate_assets<'a>(
    time_slice_info: &'a TimeSliceInfo,
    demand: &'a DemandMap,
    agent: &'a Agent,
    region_id: &'a RegionID,
    commodity_id: &'a CommodityID,
    year: u32,
) -> impl Iterator<Item = AssetRef> + 'a {
    agent
        .iter_possible_producers_of(region_id, commodity_id, year)
        .map(move |process| {
            let mut asset = Asset::new_without_capacity(
                Some(agent.id.clone()),
                process.clone(),
                region_id.clone(),
                year,
            )
            .unwrap();
            asset.capacity =
                get_demand_limiting_capacity(time_slice_info, &asset, commodity_id, demand);

            asset.into()
        })
}

/// Get the best assets for meeting demand for the given commodity
fn select_best_assets(
    model: &Model,
    mut opt_assets: Vec<AssetRef>,
    commodity: &Commodity,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    mut demand: DemandMap,
) -> Result<Vec<AssetRef>> {
    let mut best_assets: Vec<AssetRef> = Vec::new();

    let mut remaining_candidate_capacity = HashMap::from_iter(
        opt_assets
            .iter()
            .filter(|asset| !asset.is_commissioned())
            .map(|asset| (asset.clone(), asset.capacity)),
    );
    while is_any_remaining_demand(&demand) {
        ensure!(
            !opt_assets.is_empty(),
            "Failed to meet demand for commodity '{}' with provided assets",
            &commodity.id
        );

        let mut current_best: Option<AppraisalOutput> = None;
        for asset in opt_assets.iter() {
            let max_capacity = (!asset.is_commissioned()).then(|| {
                let max_capacity = model.parameters.capacity_limit_factor * asset.capacity;
                let remaining_capacity = remaining_candidate_capacity[asset];
                max_capacity.min(remaining_capacity)
            });

            let output = appraise_investment(
                model,
                asset,
                max_capacity,
                commodity,
                objective_type,
                reduced_costs,
                &demand,
            )?;

            if current_best
                .as_ref()
                .is_none_or(|best_output| output.metric < best_output.metric)
            {
                // Sanity check. We currently have no good way to handle this scenario and it can
                // cause an infinite loop.
                assert!(
                    output.capacity > Capacity(0.0),
                    "Attempted to select asset '{}' with zero capacity.\nSee: \
                    https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/716",
                    &output.asset.process.id
                );

                current_best = Some(output);
            }
        }

        let best_output = current_best.expect("No assets given");
        let asset = best_output.asset;
        let capacity = best_output.capacity;
        demand = best_output.unmet_demand;

        let commissioned_txt = if asset.is_commissioned() {
            "existing"
        } else {
            "candidate"
        };
        debug!(
            "Selected {} asset '{}' (capacity: {})",
            commissioned_txt, &asset.process.id, capacity
        );

        update_assets(
            asset,
            capacity,
            &mut opt_assets,
            &mut remaining_candidate_capacity,
            &mut best_assets,
        );
    }

    Ok(best_assets)
}

/// Check whether there is any remaining demand that is unmet in any time slice
fn is_any_remaining_demand(demand: &HashMap<TimeSliceID, Flow>) -> bool {
    demand.values().any(|flow| *flow > Flow(0.0))
}

/// Update capacity of chosen asset, if needed, and update both asset options and chosen assets
fn update_assets(
    mut best_asset: AssetRef,
    capacity: Capacity,
    opt_assets: &mut Vec<AssetRef>,
    remaining_candidate_capacity: &mut HashMap<AssetRef, Capacity>,
    best_assets: &mut Vec<AssetRef>,
) {
    // New capacity given for candidates only
    if !best_asset.is_commissioned() {
        // Remove this capacity from the available remaining capacity for this asset
        let remaining_capacity = remaining_candidate_capacity.get_mut(&best_asset).unwrap();
        *remaining_capacity -= capacity;

        // If there's no capacity remaining, remove the asset from the options
        if *remaining_capacity <= Capacity(0.0) {
            let old_idx = opt_assets
                .iter()
                .position(|asset| *asset == best_asset)
                .unwrap();
            opt_assets.swap_remove(old_idx);
            remaining_candidate_capacity.remove(&best_asset);
        }

        if let Some(existing_asset) = best_assets.iter_mut().find(|asset| **asset == best_asset) {
            // Add the additional required capacity
            existing_asset.make_mut().capacity += capacity;
        } else {
            // Update the capacity of the chosen asset
            best_asset.make_mut().capacity = capacity;

            best_assets.push(best_asset);
        };
    } else {
        // Remove this asset from the options
        opt_assets.retain(|asset| *asset != best_asset);

        best_assets.push(best_asset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::{Asset, AssetRef};
    use crate::commodity::{Commodity, CommodityID};
    use crate::fixture::{
        asset, commodity_id, process, process_parameter_map, region_id, svd_commodity, time_slice,
        time_slice_info, time_slice_info2,
    };
    use crate::process::{FlowType, ProcessFlow, ProcessParameter};
    use crate::region::RegionID;
    use crate::time_slice::{TimeSliceID, TimeSliceInfo};
    use crate::units::{
        ActivityPerCapacity, Dimensionless, Flow, FlowPerActivity, MoneyPerActivity,
        MoneyPerCapacity, MoneyPerCapacityPerYear, MoneyPerFlow,
    };
    use itertools::Itertools;
    use rstest::rstest;
    use std::collections::HashMap;
    use std::rc::Rc;

    /// Custom fixture for process parameters with non-zero capacity_to_activity
    fn process_parameter_with_capacity_to_activity() -> Rc<ProcessParameter> {
        Rc::new(ProcessParameter {
            capital_cost: MoneyPerCapacity(0.0),
            fixed_operating_cost: MoneyPerCapacityPerYear(0.0),
            variable_operating_cost: MoneyPerActivity(0.0),
            lifetime: 1,
            discount_rate: Dimensionless(1.0),
            capacity_to_activity: ActivityPerCapacity(1.0), // Non-zero value
        })
    }

    #[rstest]
    fn test_get_demand_profile(
        commodity_id: CommodityID,
        region_id: RegionID,
        time_slice: TimeSliceID,
        asset: Asset,
    ) {
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
                commodity_id.clone(),
                TimeSliceID {
                    season: "summer".into(),
                    time_of_day: "night".into(),
                },
            ),
            Flow(0.0),
        ); // Should be ignored

        // Call get_demand_profile
        let result = get_demand_profile(&flow_map);

        // Check result
        let mut expected = HashMap::new();
        expected.insert(
            (commodity_id.clone(), region_id.clone(), time_slice.clone()),
            Flow(17.0),
        );
        assert_eq!(result, expected);
    }

    #[rstest]
    fn test_get_demand_limiting_capacity(
        commodity_id: CommodityID,
        time_slice: TimeSliceID,
        region_id: RegionID,
        time_slice_info: TimeSliceInfo,
        svd_commodity: Commodity,
    ) {
        // Create a process flow using the existing commodity fixture
        let process_flow = ProcessFlow {
            commodity: Rc::new(svd_commodity),
            coeff: FlowPerActivity(2.0), // 2 units of flow per unit of activity
            kind: FlowType::Fixed,
            cost: MoneyPerFlow(0.0),
            is_primary_output: true,
        };

        // Create a process with the flows and activity limits
        let mut process = process(
            [region_id.clone()].into_iter().collect(),
            process_parameter_map([region_id.clone()].into_iter().collect()),
        );

        // Add the flow to the process
        process.flows.insert(
            (region_id.clone(), 2015), // Using default commission year from fixture
            [(commodity_id.clone(), process_flow)].into_iter().collect(),
        );

        // Add activity limits
        process.activity_limits.insert(
            (region_id.clone(), 2015, time_slice.clone()),
            Dimensionless(0.0)..=Dimensionless(1.0),
        );

        // Update process parameters to have non-zero capacity_to_activity
        let updated_parameter = process_parameter_with_capacity_to_activity();
        process
            .parameters
            .insert((region_id.clone(), 2015), updated_parameter);

        // Create asset with the configured process
        let asset = asset(process);

        // Create demand map - demand of 10.0 for our time slice
        let mut demand = HashMap::new();
        demand.insert(time_slice.clone(), Flow(10.0));

        // Call the function
        let result = get_demand_limiting_capacity(&time_slice_info, &asset, &commodity_id, &demand);

        // Expected calculation:
        // max_flow_per_cap = activity_per_capacity_limit (1.0) * coeff (2.0) = 2.0
        // required_capacity = demand (10.0) / max_flow_per_cap (2.0) = 5.0
        assert_eq!(result, Capacity(5.0));
    }

    #[rstest]
    fn test_get_demand_limiting_capacity_multiple_time_slices(
        time_slice_info2: TimeSliceInfo,
        svd_commodity: Commodity,
        commodity_id: CommodityID,
        region_id: RegionID,
    ) {
        // Create time slices from the fixture (day and night)
        let (time_slice1, time_slice2) =
            time_slice_info2.time_slices.keys().collect_tuple().unwrap();

        // Create a process flow using the existing commodity fixture
        let process_flow = ProcessFlow {
            commodity: Rc::new(svd_commodity),
            coeff: FlowPerActivity(1.0), // 1 unit of flow per unit of activity
            kind: FlowType::Fixed,
            cost: MoneyPerFlow(0.0),
            is_primary_output: true,
        };

        // Create a process with the flows and activity limits
        let mut process = process(
            [region_id.clone()].into_iter().collect(),
            process_parameter_map([region_id.clone()].into_iter().collect()),
        );

        // Add the flow to the process
        process.flows.insert(
            (region_id.clone(), 2015), // Using default commission year from fixture
            [(commodity_id.clone(), process_flow)].into_iter().collect(),
        );

        // Add activity limits for both time slices with different limits
        process.activity_limits.insert(
            (region_id.clone(), 2015, time_slice1.clone()),
            Dimensionless(0.0)..=Dimensionless(2.0), // Higher limit for day
        );
        process.activity_limits.insert(
            (region_id.clone(), 2015, time_slice2.clone()),
            Dimensionless(0.0)..=Dimensionless(0.0), // Zero limit for night - should be skipped
        );

        // Update process parameters to have non-zero capacity_to_activity
        let updated_parameter = process_parameter_with_capacity_to_activity();
        process
            .parameters
            .insert((region_id.clone(), 2015), updated_parameter);

        // Create asset with the configured process
        let asset = asset(process);

        // Create demand map with different demands for each time slice
        let mut demand = HashMap::new();
        demand.insert(time_slice1.clone(), Flow(4.0)); // Requires capacity of 4.0/2.0 = 2.0
        demand.insert(time_slice2.clone(), Flow(3.0)); // Would require infinite capacity, but should be skipped

        // Call the function
        let result =
            get_demand_limiting_capacity(&time_slice_info2, &asset, &commodity_id, &demand);

        // Expected: maximum of the capacity requirements across time slices (excluding zero limit)
        // Time slice 1: demand (4.0) / (activity_limit (2.0) * coeff (1.0)) = 2.0
        // Time slice 2: skipped due to zero activity limit
        // Maximum = 2.0
        assert_eq!(result, Capacity(2.0));
    }
}
