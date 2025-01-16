//! Functionality for running the MUSE 2.0 simulation.
use crate::commodity::BalanceType;
use crate::model::Model;
use crate::process::{Process, ProcessFlow};
use crate::time_slice::TimeSliceID;
use highs::{HighsModelStatus, RowProblem};
use indexmap::IndexMap;
use itertools::Itertools;
use log::*;
use std::rc::Rc;

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
struct VariableMap(IndexMap<VariableMapKey, highs::Col>);

impl VariableMap {
    fn get(
        &self,
        region_id: &Rc<str>,
        process_id: &Rc<str>,
        commodity_id: &Rc<str>,
        time_slice: &TimeSliceID,
    ) -> highs::Col {
        let key = VariableMapKey::new(region_id, process_id, commodity_id, time_slice);
        *self.0.get(&key).unwrap()
    }
}

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
pub fn run(model: &Model) {
    for year in model.iter_years() {
        info!("Milestone year: {year}");
        let output = perform_dispatch(model, year);
        for (key, flow) in output {
            info!(
                "OUT: {} {} {} {}: {}",
                key.region_id, key.process_id, key.commodity_id, key.time_slice, flow
            )
        }
    }
}

fn perform_dispatch(model: &Model, year: u32) -> impl Iterator<Item = (VariableMapKey, f64)> {
    let mut problem = RowProblem::default();

    let vars = add_variables(&mut problem, model, year);

    add_fixed_asset_constraints(&mut problem, &vars, model, year);
    add_asset_capacity_constraints(&mut problem, &vars, model, year);
    add_sed_commodity_balance_constraints(&mut problem, &vars, model, year);

    info!("Solving for {} variables", vars.0.len());
    info!("Num constraints: {}", problem.num_rows());

    let solved = problem.optimise(highs::Sense::Minimise).solve();
    let status = solved.status();
    if status != HighsModelStatus::Optimal {
        panic!("Could not solve: {status:?}");
    }

    let solution = solved.get_solution();

    // Columns of solution are in same order as vars' keys
    vars.0.into_keys().zip(solution.columns().to_vec())
}

fn add_variables(problem: &mut RowProblem, model: &Model, year: u32) -> VariableMap {
    let mut vars = VariableMap(IndexMap::new());
    for region_id in model.iter_regions() {
        for asset in model.get_assets(year, region_id) {
            for flow in asset.process.flows.iter() {
                for time_slice in model.time_slice_info.iter() {
                    let coeff =
                        calculate_cost_coeff(year, region_id, &asset.process, flow, time_slice);

                    // var's value must be <= 0 for inputs and >= 0 for outputs
                    let var = if flow.flow < 0.0 {
                        problem.add_column(coeff, ..=0.0)
                    } else {
                        problem.add_column(coeff, 0.0..)
                    };

                    let key = VariableMapKey::new(
                        region_id,
                        &asset.process.id,
                        &flow.commodity.id,
                        time_slice,
                    );

                    let existing = vars.0.insert(key, var).is_some();
                    assert!(!existing, "Duplicate entry for var");
                }
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
    // If flow is negative (representing an input), we multiply by -1 to ensure impact on objective
    // function is a positive cost
    let mut coeff = flow.flow_cost.copysign(flow.flow);

    let commodity = &flow.commodity;

    // Only applies if commodity is PAC
    if process
        .pacs
        .iter()
        .map(|pac| &pac.id)
        .contains(&commodity.id)
    {
        coeff += process.parameter.variable_operating_cost
    }

    if let Some(cost) = flow
        .commodity
        .costs
        .get(region_id.clone(), year, time_slice.clone())
    {
        if cost.balance_type == BalanceType::Net
            || (cost.balance_type == BalanceType::Consumption && flow.flow < 0.0)
            || (cost.balance_type == BalanceType::Production && flow.flow > 0.0)
        {
            coeff += cost.value;
        }
    }

    coeff
}

fn add_fixed_asset_constraints(
    problem: &mut RowProblem,
    vars: &VariableMap,
    model: &Model,
    year: u32,
) {
    for region_id in model.iter_regions() {
        for asset in model.get_assets(year, region_id) {
            let pac = asset.process.pacs.first().unwrap();
            let pac_flow = asset
                .process
                .flows
                .iter()
                .find(|flow| flow.commodity.id == pac.id)
                .unwrap()
                .flow;
            for time_slice in model.time_slice_info.iter() {
                let pac_var = vars.get(region_id, &asset.process.id, &pac.id, time_slice);
                let pac_term = (pac_var, -1.0 / pac_flow);
                for flow in asset.process.flows.iter() {
                    if flow.commodity.id == pac.id {
                        continue;
                    }

                    let var =
                        vars.get(region_id, &asset.process.id, &flow.commodity.id, time_slice);
                    problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
                }
            }
        }
    }
}

fn add_asset_capacity_constraints(
    problem: &mut RowProblem,
    vars: &VariableMap,
    model: &Model,
    year: u32,
) {
    let mut terms = Vec::new();
    for region_id in model.iter_regions() {
        for asset in model.get_assets(year, region_id) {
            let pac = asset.process.pacs.first().unwrap();
            for (time_slice, ts_length) in model.time_slice_info.fractions.iter() {
                let var = vars.get(region_id, &asset.process.id, &pac.id, time_slice);
                let coeff = 1.0 / (asset.capacity_a * ts_length);
                terms.push((var, coeff));
            }
        }
    }

    problem.add_row(..=1.0, terms);
}

fn process_affects_commodity(process: &Process, commodity_id: &Rc<str>) -> bool {
    process
        .flows
        .iter()
        .any(|flow| flow.commodity.id == *commodity_id)
}

fn add_sed_commodity_balance_constraints(
    problem: &mut RowProblem,
    vars: &VariableMap,
    model: &Model,
    year: u32,
) {
    let mut cur_vars = Vec::new();
    for region_id in model.iter_regions() {
        for commodity_id in model.commodities.keys() {
            let process_ids = model
                .get_assets(year, region_id)
                .map(|asset| &asset.process)
                .filter(|process| process_affects_commodity(process, commodity_id))
                .map(|process| &process.id);

            for process_id in process_ids {
                for time_slice in model.time_slice_info.iter() {
                    cur_vars.push(vars.get(region_id, process_id, commodity_id, time_slice));
                }
            }

            let terms = cur_vars.drain(0..).map(|var| (var, 1.0));
            problem.add_row(0.0..=0.0, terms);
        }
    }
}
