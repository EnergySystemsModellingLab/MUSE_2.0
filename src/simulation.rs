//! Functionality for running the MUSE 2.0 simulation.
use std::collections::HashMap;
use std::rc::Rc;

use crate::model::Model;
use crate::process::{Process, ProcessFlow};
use crate::time_slice::TimeSliceID;
use highs::{HighsModelStatus, RowProblem};
use itertools::Itertools;
use log::*;

#[derive(Eq, PartialEq, Hash)]
struct VariableMapKey {
    region_id: Rc<str>,
    process_id: Rc<str>,
    commodity_id: Rc<str>,
    time_slice: TimeSliceID,
}

impl VariableMapKey {
    fn new(
        region_id: &Rc<str>,
        process_id: &Rc<str>,
        commodity_id: &Rc<str>,
        time_slice: &TimeSliceID,
    ) -> Self {
        VariableMapKey {
            region_id: Rc::clone(region_id),
            process_id: Rc::clone(process_id),
            commodity_id: Rc::clone(commodity_id),
            time_slice: time_slice.clone(),
        }
    }
}

/// A map for easy lookup of variables in the optimisation.
type VariableMap = HashMap<VariableMapKey, highs::Col>;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    for year in model.iter_years() {
        info!("* Milestone year: {year}");
        perform_dispatch(model, year);
    }
}

fn perform_dispatch(model: &Model, year: u32) {
    let mut problem = RowProblem::default();

    let vars = add_variables(&mut problem, model, year);

    add_fixed_asset_constraints(&mut problem, &vars, model, year);

    let solved = problem.optimise(highs::Sense::Minimise).solve();
    let status = solved.status();
    if status != HighsModelStatus::Optimal {
        panic!("Could not solve: {status:?}");
    }

    let solution = solved.get_solution();
    info!("Solution: {:?}", solution.columns());
}

fn add_variables(problem: &mut RowProblem, model: &Model, year: u32) -> VariableMap {
    let mut vars = VariableMap::new();
    for region_id in model.iter_regions() {
        info!("** Region: {region_id}");
        for asset in model.get_assets(year, region_id) {
            info!(
                "*** Agent {} has asset {} (commissioned in {})",
                asset.agent_id, asset.process.id, asset.commission_year
            );

            for flow in asset.process.flows.iter() {
                info!("**** Commodity: {}", flow.commodity.id);

                // Just calculate for one time slice for now
                let time_slice = model.time_slice_info.iter().next().unwrap();
                let coeff = calculate_cost_coeff(year, region_id, &asset.process, flow, time_slice);
                info!("**** Coefficient: {coeff}");

                // **HACK**: We need bounds, so just make some up for now
                let bounds = -100..=100;

                let var = problem.add_column(coeff, bounds);

                let key = VariableMapKey::new(
                    region_id,
                    &asset.process.id,
                    &flow.commodity.id,
                    time_slice,
                );

                let existing = vars.insert(key, var).is_some();
                assert!(!existing, "Duplicate entry for var");
            }
        }
    }

    vars
}

fn calculate_cost_coeff(
    year: u32,
    region_id: &Rc<str>,
    process: &Process,
    flow: &ProcessFlow,
    time_slice: &TimeSliceID,
) -> f64 {
    // NB: Whether this cost applies or not will depend on what kind of flow this is, so this is
    // also wrong
    let flow_cost = flow.flow_cost;

    let commodity = &flow.commodity;

    // Only applies if commodity is PAC
    let var_opex = if process
        .pacs
        .iter()
        .map(|pac| &pac.id)
        .contains(&commodity.id)
    {
        info!("It's a PAC");
        process.parameter.variable_operating_cost
    } else {
        info!("It's NOT a PAC");
        0.0
    };

    // NB: Need to check balance type
    let commodity_cost = match flow
        .commodity
        .costs
        .get(region_id.clone(), year, time_slice.clone())
    {
        None => {
            warn!("Commodity cost not found");
            0.0
        }
        Some(cost) => {
            info!("Commodity cost found :-D");
            cost.value
        }
    };

    // Calculation from dispatch optimisation formulation (with the caveat that each of these values
    // is probably wrong :-))
    var_opex + flow_cost + commodity_cost
}

fn add_fixed_asset_constraints(
    problem: &mut RowProblem,
    vars: &VariableMap,
    model: &Model,
    year: u32,
) {
    for region_id in model.iter_regions() {
        for asset in model.get_assets(year, region_id) {
            // Just calculate for one time slice for now
            let time_slice = model.time_slice_info.iter().next().unwrap();

            let pac = asset.process.pacs.first().unwrap();
            let pac_flow = asset
                .process
                .flows
                .iter()
                .find(|flow| flow.commodity.id == pac.id)
                .unwrap()
                .flow;
            let key = VariableMapKey::new(region_id, &asset.process.id, &pac.id, time_slice);
            let pac_var = *vars.get(&key).unwrap();
            let pac_term = (pac_var, -1.0 / pac_flow);
            for flow in asset.process.flows.iter() {
                if flow.commodity.id == pac.id {
                    continue;
                }

                let key = VariableMapKey::new(
                    region_id,
                    &asset.process.id,
                    &flow.commodity.id,
                    time_slice,
                );
                let var = *vars.get(&key).unwrap();
                problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
            }
        }
    }
}
