//! Code for performing agent investment.
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use crate::agent::{Agent, ObjectiveType};
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{ActivityPerCapacity, Capacity, Dimensionless, Flow};
use anyhow::{ensure, Result};
use indexmap::IndexSet;
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
        for (agent, commodity_portion) in
            get_responsible_agents(model.agents.values(), commodity_id, year)
        {
            let objective_type = agent.objectives.get(&year).unwrap();

            for region_id in agent.regions.iter() {
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
                    agent,
                    commodity_id,
                    region_id,
                    &demand_for_commodity,
                    year,
                )
                .collect();

                // Choose assets from among existing pool and candidates
                let best_assets = select_best_assets(
                    opt_assets,
                    commodity_id,
                    objective_type,
                    reduced_costs,
                    demand_for_commodity,
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
                        .unwrap(),
            )
        })
        .collect()
}

/// Get the agents responsible for a given commodity in a given year along with the commodity
/// portion for which they are responsible
fn get_responsible_agents<'a, I>(
    agents: I,
    commodity_id: &'a CommodityID,
    year: u32,
) -> impl Iterator<Item = (&'a Agent, Dimensionless)>
where
    I: Iterator<Item = &'a Agent>,
{
    agents.filter_map(move |agent| {
        let portion = agent
            .commodity_portions
            .get(&(commodity_id.clone(), year))?;

        Some((agent, *portion))
    })
}

/// Get required capacity by time slice
fn iter_capacity_per_time_slice<'a, I>(
    act_per_cap: I,
    asset: &'a Asset,
    commodity_id: &'a CommodityID,
    demand: &'a DemandMap,
) -> impl Iterator<Item = (&'a TimeSliceID, Capacity)> + 'a
where
    I: IntoIterator<Item = (&'a TimeSliceID, ActivityPerCapacity)> + 'a,
{
    // Flow coefficient for this commodity
    let coeff = asset.get_flow(commodity_id).unwrap().coeff;

    // Required capacity to meet demand in any time slice
    act_per_cap
        .into_iter()
        .map(move |(time_slice, max_act_per_cap)| {
            let max_flow_per_cap = max_act_per_cap * coeff;

            let capacity = demand[time_slice] / max_flow_per_cap;
            (time_slice, capacity)
        })
}

/// Get the maximum required capacity across time slices
fn get_demand_limiting_capacity(
    time_slice_info: &TimeSliceInfo,
    asset: &Asset,
    commodity_id: &CommodityID,
    demand: &DemandMap,
) -> Capacity {
    let act_per_cap = time_slice_info.iter_ids().map(|time_slice| {
        let value = *asset.get_activity_per_capacity_limits(time_slice).end();
        (time_slice, value)
    });
    iter_capacity_per_time_slice(act_per_cap, asset, commodity_id, demand)
        .map(|(_, capacity)| capacity)
        .max_by(|a, b| a.total_cmp(b))
        .unwrap()
}

/// Get options from existing and potential assets for the given parameters
fn get_asset_options<'a>(
    time_slice_info: &'a TimeSliceInfo,
    all_existing_assets: &'a [AssetRef],
    agent: &'a Agent,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    demand: &'a DemandMap,
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
        agent,
        region_id,
        commodity_id,
        demand,
        year,
    );

    chain(existing_assets, candidate_assets)
}

/// Get candidate assets which produce a particular commodity for a given agent
fn get_candidate_assets<'a>(
    time_slice_info: &'a TimeSliceInfo,
    agent: &'a Agent,
    region_id: &'a RegionID,
    commodity_id: &'a CommodityID,
    demand: &'a DemandMap,
    year: u32,
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
    mut opt_assets: Vec<AssetRef>,
    commodity_id: &CommodityID,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    mut demand: DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<Vec<AssetRef>> {
    let mut best_assets: Vec<AssetRef> = Vec::new();

    while is_remaining_unmet_demand(&demand) {
        ensure!(
            !opt_assets.is_empty(),
            "Failed to meet demand for commodity '{commodity_id}' with provided assets"
        );

        let mut current_best: Option<AppraisalOutput> = None;
        for asset in opt_assets.iter() {
            let output = appraise_investment(
                asset,
                objective_type,
                reduced_costs,
                &demand,
                time_slice_info,
                time_slice_level,
            )?;

            if current_best
                .as_ref()
                .is_none_or(|best_output| output.is_better_than(best_output))
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
        let (asset, capacity, unmet_demand) = best_output.into_parts(commodity_id, demand);
        demand = unmet_demand;

        let commissioned_txt = if asset.is_commissioned() {
            "existing"
        } else {
            "candidate"
        };
        debug!(
            "Selected {} asset '{}'",
            commissioned_txt, &asset.process.id
        );

        update_assets(asset, capacity, &mut opt_assets, &mut best_assets);
    }

    Ok(best_assets)
}

/// Check whether there is any remaining demand that is unmet in any time slice
fn is_remaining_unmet_demand(demand: &HashMap<TimeSliceID, Flow>) -> bool {
    demand.values().any(|flow| *flow > Flow(0.0))
}

/// Update capacity of chosen asset, if needed, and update both asset options and chosen assets
fn update_assets(
    mut best_asset: AssetRef,
    capacity: Capacity,
    opt_assets: &mut Vec<AssetRef>,
    best_assets: &mut Vec<AssetRef>,
) {
    // New capacity given for candidates only
    if !best_asset.is_commissioned() {
        // Get a reference to the copy of the asset in opt_assets
        let (old_idx, old) = opt_assets
            .iter_mut()
            .enumerate()
            .find(|(_, asset)| **asset == best_asset)
            .unwrap();

        // Remove this capacity from the available remaining capacity for this asset
        old.make_mut().capacity -= capacity;

        // If there's no capacity remaining, remove the asset from the options
        if old.capacity <= Capacity(0.0) {
            opt_assets.swap_remove(old_idx);
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
    use crate::agent::AgentID;
    use crate::asset::{Asset, AssetRef};
    use crate::commodity::Commodity;
    use crate::commodity::CommodityID;
    use crate::fixture::{agent_id, asset, commodity_id, region_id, time_slice, time_slice_info2};
    use crate::process::{
        FlowType, Process, ProcessActivityLimitsMap, ProcessFlow, ProcessFlowsMap,
        ProcessParameter, ProcessParameterMap,
    };
    use crate::units::{
        ActivityPerCapacity, Capacity, Dimensionless, Flow, FlowPerActivity, MoneyPerActivity,
        MoneyPerCapacity, MoneyPerCapacityPerYear,
    };
    use float_cmp::assert_approx_eq;
    use indexmap::indexmap;
    use itertools::{izip, Itertools};
    use rstest::rstest;
    use std::collections::HashMap;
    use std::rc::Rc;

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

    #[rstest]
    fn test_iter_capacity_per_time_slice(
        time_slice_info2: TimeSliceInfo,
        agent_id: AgentID,
        region_id: RegionID,
        commodity_id: CommodityID,
    ) {
        // Setup two time slices
        let (ts_day, ts_night) = time_slice_info2.time_slices.keys().collect_tuple().unwrap();
        let year = 2020u32;

        // Activity limits: [0.0, 1.0] for both slices
        let mut activity_limits = ProcessActivityLimitsMap::new();
        activity_limits.insert(
            (region_id.clone(), year, ts_day.clone()),
            Dimensionless(0.0)..=Dimensionless(1.0),
        );
        activity_limits.insert(
            (region_id.clone(), year, ts_night.clone()),
            Dimensionless(0.0)..=Dimensionless(1.0),
        );

        // Process parameters: capacity_to_activity = 2.0
        let mut parameters = ProcessParameterMap::new();
        parameters.insert(
            (region_id.clone(), year),
            Rc::new(ProcessParameter {
                capital_cost: MoneyPerCapacity(0.0),
                fixed_operating_cost: MoneyPerCapacityPerYear(0.0),
                variable_operating_cost: MoneyPerActivity(0.0),
                lifetime: 1,
                discount_rate: Dimensionless(1.0),
                capacity_to_activity: ActivityPerCapacity(2.0),
            }),
        );

        // Commodity and ProcessFlow
        let commodity = Rc::new(Commodity {
            id: commodity_id.clone(),
            description: "Test commodity".into(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::DayNight,
            levies: Default::default(),
            demand: Default::default(),
        });
        let flow = ProcessFlow {
            commodity: commodity.clone(),
            coeff: FlowPerActivity(3.0),
            kind: FlowType::Fixed,
            cost: crate::units::MoneyPerFlow(0.0),
            is_primary_output: true,
        };
        let mut flows = ProcessFlowsMap::new();
        flows.insert(
            (region_id.clone(), year),
            indexmap! { commodity_id.clone() => flow },
        );

        // Build process
        let process = Process {
            id: "p1".into(),
            description: "desc".into(),
            years: vec![year],
            activity_limits,
            flows,
            parameters,
            regions: [region_id.clone()].into_iter().collect(),
        };

        // Demand: 6.0 for day, 12.0 for night
        let mut demand = HashMap::new();
        demand.insert(ts_day.clone(), Flow(5.0));
        demand.insert(ts_night.clone(), Flow(6.0));

        let act_per_cap = izip!(
            time_slice_info2.iter_ids(),
            [ActivityPerCapacity(0.1), ActivityPerCapacity(0.2)]
        );
        let asset = Asset::new(
            Some(agent_id),
            process.into(),
            region_id,
            Capacity(100.0),
            year,
        )
        .unwrap();

        // Run iter_capacity_per_time_slice
        let result: HashMap<_, _> =
            iter_capacity_per_time_slice(act_per_cap, &asset, &commodity_id, &demand).collect();

        assert_approx_eq!(Capacity, result[&ts_day], Capacity(50.0 / 3.0));
        assert_approx_eq!(Capacity, result[&ts_night], Capacity(60.0 / 6.0));
    }
}
