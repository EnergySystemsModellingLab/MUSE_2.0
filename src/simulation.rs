//! Functionality for running the MUSE 2.0 simulation.
use std::rc::Rc;

use crate::model::Model;
use crate::process::{Process, ProcessFlow};
use crate::time_slice::TimeSliceID;
use highs::{HighsModelStatus, RowProblem};
use itertools::Itertools;
use log::*;

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

    let _ = add_variables(&mut problem, model, year);

    // // Add constraints
    // for constraint in constraints.iter() {
    //     if constraint.coefficients.len() != vars.len() {
    //         panic!("Wrong number of variables specified for constraint");
    //     }

    //     let mut coeffs = Vec::with_capacity(vars.len());
    //     for (var, coeff) in vars.iter().zip(constraint.coefficients.iter()) {
    //         coeffs.push((*var, *coeff));
    //     }

    //     problem.add_row(constraint.min..=constraint.max, coeffs);
    // }

    let solved = problem.optimise(highs::Sense::Minimise).solve();
    let status = solved.status();
    if status != HighsModelStatus::Optimal {
        panic!("Could not solve: {status:?}");
    }

    let solution = solved.get_solution();
    info!("Solution: {:?}", solution.columns());
}

fn add_variables(problem: &mut RowProblem, model: &Model, year: u32) -> Vec<highs::Col> {
    let mut vars = Vec::new();
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
                vars.push(problem.add_column(coeff, bounds));
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
