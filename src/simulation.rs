//! Functionality for running the MUSE 2.0 simulation.
use crate::model::Model;
use crate::process::{Process, ProcessFlow};
use crate::time_slice::TimeSliceID;
use highs::{HighsModelStatus, RowProblem};
use itertools::Itertools;
use log::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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
struct VariableMap {
    map: HashMap<VariableMapKey, highs::Col>,
    touched: RefCell<HashMap<VariableMapKey, bool>>,
}

impl VariableMap {
    fn get(
        &self,
        region_id: &Rc<str>,
        process_id: &Rc<str>,
        commodity_id: &Rc<str>,
        time_slice: &TimeSliceID,
    ) -> highs::Col {
        let key = VariableMapKey::new(region_id, process_id, commodity_id, time_slice);
        self.touched.borrow_mut().insert(key.clone(), true);

        *self.map.get(&key).unwrap()
    }
}

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
    add_asset_capacity_constraints(&mut problem, &vars, model, year);
    add_sed_commodity_balance_constraints(&mut problem, &vars, model, year);

    find_untouched(&vars);

    let solved = problem.optimise(highs::Sense::Minimise).solve();
    let status = solved.status();
    if status != HighsModelStatus::Optimal {
        panic!("Could not solve: {status:?}");
    }

    let solution = solved.get_solution();
    info!("Solution: {:?}", solution.columns());
}

fn add_variables(problem: &mut RowProblem, model: &Model, year: u32) -> VariableMap {
    let mut vars = VariableMap {
        map: HashMap::new(),
        touched: RefCell::new(HashMap::new()),
    };
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

                let bounds = f64::NEG_INFINITY..=f64::INFINITY;

                let var = problem.add_column(coeff, bounds);

                let key = VariableMapKey::new(
                    region_id,
                    &asset.process.id,
                    &flow.commodity.id,
                    time_slice,
                );

                let existing = vars.map.insert(key.clone(), var).is_some();
                assert!(!existing, "Duplicate entry for var");

                vars.touched.get_mut().insert(key, false);
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
    let mut coeff = flow.flow_cost;

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

    // Should this be conditional?!
    if let Some(commodity_cost) =
        flow.commodity
            .costs
            .get(region_id.clone(), year, time_slice.clone())
    {
        coeff += commodity_cost.value;
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
            let pac_var = vars.get(region_id, &asset.process.id, &pac.id, time_slice);
            let pac_term = (pac_var, -1.0 / pac_flow);
            for flow in asset.process.flows.iter() {
                if flow.commodity.id == pac.id {
                    continue;
                }

                let var = vars.get(region_id, &asset.process.id, &flow.commodity.id, time_slice);
                problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
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
            // Just calculate for one time slice for now
            let (time_slice, ts_length) = model.time_slice_info.fractions.iter().next().unwrap();

            let pac = asset.process.pacs.first().unwrap();
            let var = vars.get(region_id, &asset.process.id, &pac.id, time_slice);
            let coeff = 1.0 / (asset.capacity_a * ts_length);
            terms.push((var, coeff));
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
    for region_id in model.iter_regions() {
        for commodity_id in model.commodities.keys() {
            // Just calculate for one time slice for now
            let time_slice = model.time_slice_info.iter().next().unwrap();

            let process_ids = model
                .get_assets(year, region_id)
                .map(|asset| &asset.process)
                .filter(|process| process_affects_commodity(process, commodity_id))
                .map(|process| &process.id);

            let vars = process_ids
                .map(|process_id| vars.get(region_id, process_id, commodity_id, time_slice));

            let terms = vars.map(|var| (var, 1.0)).collect_vec();
            problem.add_row(0.0..=0.0, terms);
        }
    }
}

fn find_untouched(vars: &VariableMap) {
    let binding = vars.touched.borrow();
    let (touched, untouched): (Vec<_>, Vec<_>) = binding
        .keys()
        .partition(|key| *vars.touched.borrow().get(key).unwrap());

    for key in touched {
        info!("TOUCHED: {key:?}");
    }
    info!("!!!!!!!!!!!!!!!!!!!!!!!!");
    for key in untouched {
        info!("UNTOUCHED: {key:?}");
    }
}
