//! Code for performing agent investment.
use super::demand::{
    calculate_demand_in_tranche, calculate_load, get_demand_profile, get_tranches,
};
use super::lcox::{calculate_lcox, AppraisalOutput};
use super::optimisation::FlowMap;
use super::prices::ReducedCosts;
use crate::agent::{Agent, ObjectiveType};
use crate::asset::{Asset, AssetIterator, AssetPool, AssetRef};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::{Capacity, Flow, FlowPerYear};
use itertools::Itertools;
use log::info;
use std::collections::HashMap;

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
    assets: &AssetPool,
    year: u32,
) {
    info!("Performing agent investment...");

    // We consider SVD commodities first
    let commodities_of_interest = model
        .commodities
        .iter()
        .filter(|(_, commodity)| commodity.kind == CommodityType::ServiceDemand)
        .map(|(id, _)| id.clone())
        .collect();
    let demand = get_demand_profile(&commodities_of_interest, flow_map);

    for commodity_id in commodities_of_interest.iter() {
        for agent in get_responsible_agents(model.agents.values(), commodity_id, year) {
            let objective_type = agent.objectives.get(&year).unwrap();

            for region_id in agent.regions.iter() {
                // Existing and candidate assets from which to choose
                let opt_assets = get_asset_options(
                    assets,
                    agent,
                    region_id,
                    commodity_id,
                    year,
                    model.parameters.candidate_asset_capacity,
                );

                // Calculate load for every time slice and peak load
                let (load_map, peak_load) =
                    calculate_load(&model.time_slice_info, commodity_id, region_id, &demand);

                for asset in opt_assets {
                    let appraisal_func = |tranche_demand: &HashMap<_, _>| match objective_type {
                        ObjectiveType::LevelisedCostOfX => {
                            calculate_lcox(&asset, reduced_costs, tranche_demand)
                        }
                    };
                    perform_appraisal_for_tranches(model, &load_map, peak_load, appraisal_func);
                }
            }
        }
    }

    // **TODO:** Perform agent investment. For now, let's just leave the pool unmodified.
    // assets.replace_active_pool(new_pool);
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

/// Get options from existing and potential assets for the given parameters
fn get_asset_options(
    assets: &AssetPool,
    agent: &Agent,
    region_id: &RegionID,
    commodity_id: &CommodityID,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> Vec<AssetRef> {
    // Get existing assets which produce the commodity of interest
    let existing_assets = assets
        .iter()
        .filter_agent(&agent.id)
        .filter_region(region_id)
        .filter_producers_of(commodity_id)
        .cloned();
    let mut opt_assets = existing_assets.collect_vec();

    // Get candidates assets which produce the commodity of interest
    let candidate_assets = get_candidate_assets(
        agent,
        commodity_id,
        region_id,
        year,
        candidate_asset_capacity,
    );
    if let Some(candidate_assets) = candidate_assets {
        opt_assets.extend(candidate_assets);
    }

    opt_assets
}

/// Get candidate assets which produce a particular commodity for a given agent
fn get_candidate_assets<'a>(
    agent: &'a Agent,
    commodity_id: &'a CommodityID,
    region_id: &'a RegionID,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> Option<impl Iterator<Item = AssetRef> + 'a> {
    let flows_key = (region_id.clone(), year);

    let producers = agent
        .search_space
        .get(&(commodity_id.clone(), year))?
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
    let assets = producers.map(move |process| {
        Asset::new(
            Some(agent.id.clone()),
            process.clone(),
            region_id.clone(),
            candidate_asset_capacity,
            year,
        )
        .unwrap()
        .into()
    });

    Some(assets)
}

/// Divide demand into tranches and perform appraisal over each in turn
fn perform_appraisal_for_tranches<F>(
    model: &Model,
    load_map: &HashMap<TimeSliceID, FlowPerYear>,
    peak_load: FlowPerYear,
    appraisal_func: F,
) where
    F: Fn(&HashMap<TimeSliceID, Flow>) -> AppraisalOutput,
{
    // We want to consider the tranche with the highest load factor first, but in our case
    // that will always be the first
    let mut unmet_demand: Option<HashMap<TimeSliceID, Flow>> = None;
    for tranche in get_tranches(peak_load, model.parameters.num_demand_tranches) {
        let demand_iter = calculate_demand_in_tranche(&model.time_slice_info, load_map, &tranche);

        // Get demand for current tranche
        let tranche_demand = if let Some(unmet_demand) = unmet_demand {
            // If there is unmet demand from the previous tranche, we include it here
            demand_iter
                .map(|(ts, demand)| {
                    let unmet = *unmet_demand.get(&ts).unwrap();
                    (ts, demand + unmet)
                })
                .collect()
        } else {
            demand_iter.collect()
        };

        // Investment appraisal
        let appraisal_output = appraisal_func(&tranche_demand);
        unmet_demand = Some(appraisal_output.unmet_demand);
    }
}
