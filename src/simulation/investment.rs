//! Code for performing agent investment.
use super::optimisation::{DispatchRun, FlowMap};
use super::prices::ReducedCosts;
use crate::agent::Agent;
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{Commodity, CommodityID, CommodityMap};
use crate::model::Model;
use crate::output::DataWriter;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Capacity, Dimensionless, Flow, FlowPerCapacity};
use anyhow::{ensure, Result};
use itertools::chain;
use log::debug;
use std::collections::HashMap;

pub mod appraisal;
use appraisal::appraise_investment;

/// A map of demand across time slices for a specific commodity and region
type DemandMap = HashMap<TimeSliceID, Flow>;

/// Demand for a given combination of commodity, region and time slice
type AllDemandMap = HashMap<(CommodityID, RegionID, TimeSliceID), Flow>;

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `year` - Current milestone year
/// * `assets` - The asset pool
/// * `flow_map` - Map of commodity flows
/// * `prices` - Commodity prices
/// * `reduced_costs` - Reduced costs for assets
/// * `writer` - Data writer
pub fn perform_agent_investment(
    model: &Model,
    year: u32,
    assets: &mut AssetPool,
    reduced_costs: &ReducedCosts,
    writer: &mut DataWriter,
) -> Result<()> {
    // Get all existing assets and clear pool
    let existing_assets = assets.take();

    // Initialise demand map
    let mut demand =
        flatten_preset_demands_for_year(&model.commodities, &model.time_slice_info, year);

    for region_id in model.iter_regions() {
        let mut seen_commodities = Vec::new();
        for commodity_id in model.commodity_order[&(region_id.clone(), year)].iter() {
            seen_commodities.push(commodity_id.clone());
            let commodity = &model.commodities[commodity_id];
            let mut new_assets = Vec::new();
            for (agent, commodity_portion) in
                get_responsible_agents(model.agents.values(), commodity_id, region_id, year)
            {
                debug!(
                    "Running investment for agent '{}' with commodity '{}' in region '{}'",
                    &agent.id, commodity_id, region_id
                );

                // Get demand portion for this commodity for this agent in this region/year
                let demand_portion_for_commodity = get_demand_portion_for_commodity(
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
                    &demand_portion_for_commodity,
                    agent,
                    commodity,
                    region_id,
                    year,
                )
                .collect();

                // Choose assets from among existing pool and candidates
                let best_assets = select_best_assets(
                    model,
                    opt_assets,
                    commodity,
                    agent,
                    reduced_costs,
                    demand_portion_for_commodity,
                    year,
                    writer,
                )?;
                new_assets.extend(best_assets);
            }

            // If no assets have been selected, skip dispatch optimisation
            // **TODO**: this probably means there's no demand for the commodity, which we could
            // presumably preempt
            if new_assets.is_empty() {
                continue;
            }

            // Add assets to pool
            new_assets = assets.extend(new_assets);

            // Perform dispatch optimisation with assets that have been selected so far
            // **TODO**: presumably we only need to do this for new_assets, as assets added in
            // previous iterations should not change
            debug!("Running post-investment dispatch for commodity '{commodity_id}' in region '{region_id}'");

            let solution = DispatchRun::new(model, assets.as_slice(), year)
                .with_commodity_subset(&seen_commodities)
                .run(
                    &format!("post {commodity_id}/{region_id} investment"),
                    writer,
                )?;

            // Update demand map with flows from newly added assets
            update_demand_map(&mut demand, &solution.create_flow_map(), &new_assets);
        }
    }

    // Decommission non-selected assets
    assets.decommission_if_not_active(existing_assets, year);

    Ok(())
}

/// Flatten the preset commodity demands for a given year into a map of commodity, region and
/// time slice to demand.
///
/// Since demands for some commodities may be specified at a coarser timeslice level, we need to
/// distribute these demands over all timeslices. Note: the way that we do this distribution is
/// irrelevant, as demands will only be balanced to the appropriate level, but we still need to do
/// this for the solver to work.
///
/// **TODO**: these assumptions may need to be revisited, e.g. when we come to storage technologies
fn flatten_preset_demands_for_year(
    commodities: &CommodityMap,
    time_slice_info: &TimeSliceInfo,
    year: u32,
) -> AllDemandMap {
    let mut demand_map = AllDemandMap::new();
    for (commodity_id, commodity) in commodities.iter() {
        for ((region_id, data_year, time_slice_selection), demand) in commodity.demand.iter() {
            if *data_year != year {
                continue;
            }

            // We split the demand equally over all timeslices in the selection
            // NOTE: since demands will only be balanced to the timeslice level of the commodity
            // it doesn't matter how we do this distribution, only the total matters.
            let n_timeslices = time_slice_selection.iter(time_slice_info).count() as f64;
            let demand_per_slice = *demand / Dimensionless(n_timeslices);
            for (time_slice, _) in time_slice_selection.iter(time_slice_info) {
                demand_map.insert(
                    (commodity_id.clone(), region_id.clone(), time_slice.clone()),
                    demand_per_slice,
                );
            }
        }
    }
    demand_map
}

/// Update demand map with flows from a set of assets
fn update_demand_map(demand: &mut AllDemandMap, flows: &FlowMap, assets: &[AssetRef]) {
    for ((asset, commodity_id, time_slice), flow) in flows.iter() {
        if assets.contains(asset) {
            let key = (
                commodity_id.clone(),
                asset.region_id().clone(),
                time_slice.clone(),
            );

            // Note: we use the negative of the flow as input flows are negative in the flow map.
            demand
                .entry(key)
                .and_modify(|value| *value -= *flow)
                .or_insert(-*flow);
        }
    }
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
    commodity: &Commodity,
    demand: &DemandMap,
) -> Capacity {
    let coeff = asset.get_flow(&commodity.id).unwrap().coeff;
    let mut capacity = Capacity(0.0);

    for time_slice_selection in time_slice_info.iter_selections_at_level(commodity.time_slice_level)
    {
        let demand_for_selection: Flow = time_slice_selection
            .iter(time_slice_info)
            .map(|(time_slice, _)| demand[time_slice])
            .sum();

        // Calculate max capacity required for this time slice selection
        // For commodities with a coarse timeslice level, we have to allow the possibility that all
        // of the demand gets served by production in a single timeslice
        for (time_slice, _) in time_slice_selection.iter(time_slice_info) {
            let max_flow_per_cap =
                *asset.get_activity_per_capacity_limits(time_slice).end() * coeff;
            if max_flow_per_cap != FlowPerCapacity(0.0) {
                capacity = capacity.max(demand_for_selection / max_flow_per_cap);
            }
        }
    }

    capacity
}

/// Get options from existing and potential assets for the given parameters
fn get_asset_options<'a>(
    time_slice_info: &'a TimeSliceInfo,
    all_existing_assets: &'a [AssetRef],
    demand: &'a DemandMap,
    agent: &'a Agent,
    commodity: &'a Commodity,
    region_id: &'a RegionID,
    year: u32,
) -> impl Iterator<Item = AssetRef> + 'a {
    // Get existing assets which produce the commodity of interest
    let existing_assets = all_existing_assets
        .iter()
        .filter_agent(&agent.id)
        .filter_region(region_id)
        .filter_primary_producers_of(&commodity.id)
        .cloned();

    // Get candidates assets which produce the commodity of interest
    let candidate_assets =
        get_candidate_assets(time_slice_info, demand, agent, region_id, commodity, year);

    chain(existing_assets, candidate_assets)
}

/// Get candidate assets which produce a particular commodity for a given agent
fn get_candidate_assets<'a>(
    time_slice_info: &'a TimeSliceInfo,
    demand: &'a DemandMap,
    agent: &'a Agent,
    region_id: &'a RegionID,
    commodity: &'a Commodity,
    year: u32,
) -> impl Iterator<Item = AssetRef> + 'a {
    agent
        .iter_possible_producers_of(region_id, &commodity.id, year)
        .map(move |process| {
            let mut asset = Asset::new_without_capacity(
                Some(agent.id.clone()),
                process.clone(),
                region_id.clone(),
                year,
            )
            .unwrap();
            asset.set_capacity(get_demand_limiting_capacity(
                time_slice_info,
                &asset,
                commodity,
                demand,
            ));

            asset.into()
        })
}

/// Get the best assets for meeting demand for the given commodity
#[allow(clippy::too_many_arguments)]
fn select_best_assets(
    model: &Model,
    mut opt_assets: Vec<AssetRef>,
    commodity: &Commodity,
    agent: &Agent,
    reduced_costs: &ReducedCosts,
    mut demand: DemandMap,
    year: u32,
    writer: &mut DataWriter,
) -> Result<Vec<AssetRef>> {
    let mut best_assets: Vec<AssetRef> = Vec::new();

    let mut remaining_candidate_capacity = HashMap::from_iter(
        opt_assets
            .iter()
            .filter(|asset| !asset.is_commissioned())
            .map(|asset| (asset.clone(), asset.capacity())),
    );

    let mut round = 0;
    while is_any_remaining_demand(&demand) {
        ensure!(
            !opt_assets.is_empty(),
            "Failed to meet demand for commodity '{}' with provided assets",
            &commodity.id
        );

        // Appraise all options
        let mut outputs_for_opts = Vec::new();
        for asset in opt_assets.iter() {
            let max_capacity = (!asset.is_commissioned()).then(|| {
                let max_capacity = model.parameters.capacity_limit_factor * asset.capacity();
                let remaining_capacity = remaining_candidate_capacity[asset];
                max_capacity.min(remaining_capacity)
            });

            let output = appraise_investment(
                model,
                asset,
                max_capacity,
                commodity,
                &agent.objectives[&year],
                reduced_costs,
                &demand,
            )?;

            outputs_for_opts.push(output);
        }

        // Save appraisal results
        writer.write_appraisal_debug_info(
            year,
            &format!("{} {} round {}", &commodity.id, &agent.id, round),
            &outputs_for_opts,
        )?;

        // Select the best investment option
        let best_output = outputs_for_opts
            .into_iter()
            .min_by(|a, b| a.metric.partial_cmp(&b.metric).unwrap())
            .expect("No outputs given");

        // Sanity check. We currently have no good way to handle this scenario and it can
        // cause an infinite loop.
        assert!(
            best_output.capacity > Capacity(0.0),
            "Attempted to select asset '{}' with zero capacity.\nSee: \
            https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/716",
            &best_output.asset.process_id()
        );

        // Log the selected asset
        let commissioned_txt = if best_output.asset.is_commissioned() {
            "existing"
        } else {
            "candidate"
        };
        debug!(
            "Selected {} asset '{}' (capacity: {})",
            commissioned_txt,
            &best_output.asset.process_id(),
            best_output.capacity
        );

        // Update the assets
        update_assets(
            best_output.asset,
            best_output.capacity,
            &mut opt_assets,
            &mut remaining_candidate_capacity,
            &mut best_assets,
        );

        demand = best_output.unmet_demand;
        round += 1;
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
            existing_asset.make_mut().increase_capacity(capacity);
        } else {
            // Update the capacity of the chosen asset
            best_asset.make_mut().set_capacity(capacity);
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
    use crate::commodity::Commodity;
    use crate::fixture::{
        asset, process, process_parameter_map, region_id, svd_commodity, time_slice,
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
    fn test_get_demand_limiting_capacity(
        time_slice: TimeSliceID,
        region_id: RegionID,
        time_slice_info: TimeSliceInfo,
        svd_commodity: Commodity,
    ) {
        // Create a process flow using the existing commodity fixture
        let commodity_rc = Rc::new(svd_commodity);
        let process_flow = ProcessFlow {
            commodity: Rc::clone(&commodity_rc),
            coeff: FlowPerActivity(2.0), // 2 units of flow per unit of activity
            kind: FlowType::Fixed,
            cost: MoneyPerFlow(0.0),
        };

        // Create a process with the flows and activity limits
        let mut process = process(
            [region_id.clone()].into_iter().collect(),
            process_parameter_map([region_id.clone()].into_iter().collect()),
        );

        // Add the flow to the process
        process.flows.insert(
            (region_id.clone(), 2015), // Using default commission year from fixture
            [(commodity_rc.id.clone(), process_flow)]
                .into_iter()
                .collect(),
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
        let result = get_demand_limiting_capacity(&time_slice_info, &asset, &commodity_rc, &demand);

        // Expected calculation:
        // max_flow_per_cap = activity_per_capacity_limit (1.0) * coeff (2.0) = 2.0
        // required_capacity = demand (10.0) / max_flow_per_cap (2.0) = 5.0
        assert_eq!(result, Capacity(5.0));
    }

    #[rstest]
    fn test_get_demand_limiting_capacity_multiple_time_slices(
        time_slice_info2: TimeSliceInfo,
        svd_commodity: Commodity,
        region_id: RegionID,
    ) {
        // Create time slices from the fixture (day and night)
        let (time_slice1, time_slice2) =
            time_slice_info2.time_slices.keys().collect_tuple().unwrap();

        // Create a process flow using the existing commodity fixture
        let commodity_rc = Rc::new(svd_commodity);
        let process_flow = ProcessFlow {
            commodity: Rc::clone(&commodity_rc),
            coeff: FlowPerActivity(1.0), // 1 unit of flow per unit of activity
            kind: FlowType::Fixed,
            cost: MoneyPerFlow(0.0),
        };

        // Create a process with the flows and activity limits
        let mut process = process(
            [region_id.clone()].into_iter().collect(),
            process_parameter_map([region_id.clone()].into_iter().collect()),
        );

        // Add the flow to the process
        process.flows.insert(
            (region_id.clone(), 2015), // Using default commission year from fixture
            [(commodity_rc.id.clone(), process_flow)]
                .into_iter()
                .collect(),
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
            get_demand_limiting_capacity(&time_slice_info2, &asset, &commodity_rc, &demand);

        // Expected: maximum of the capacity requirements across time slices (excluding zero limit)
        // Time slice 1: demand (4.0) / (activity_limit (2.0) * coeff (1.0)) = 2.0
        // Time slice 2: skipped due to zero activity limit
        // Maximum = 2.0
        assert_eq!(result, Capacity(2.0));
    }
}
