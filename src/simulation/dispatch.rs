//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use crate::process::ProcessFlow;
use crate::simulation::filter_assets;
use crate::time_slice::TimeSliceID;
use log::info;

/// Placeholder for optimisation problem object
struct Problem;

impl Problem {
    /// Solve the problem
    fn solve(&self) -> Solution {
        Solution {}
    }
}

/// Placeholder for optimisation solution object
pub struct Solution;

/// Placeholder for map of decision variables
struct VariableMap;

/// Perform the dispatch optimisation.
///
/// Updates commodity flows for assets and commodity prices.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns
///
/// A set of IDs for commodities whose prices weren't updated.
pub fn perform_dispatch(model: &Model, assets: &AssetPool, year: u32) -> Solution {
    info!("Performing dispatch optimisation...");

    // Set up problem
    let mut problem = Problem {};
    let variables = add_variables(&mut problem, model, assets, year);
    add_commodity_balance_constraints(&mut problem, &variables, model, assets);

    // Return solution.
    // **TODO**: In practice we will need to return other values needed to interpret the solution
    // (e.g. the keys from the variable map)
    problem.solve()
}

/// Add variables to the optimisation problem.
///
/// # Arguments
///
/// * `problem` - The optimisation problem
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns
///
/// A [`VariableMap`] with the problem's variables as values.
fn add_variables(
    _problem: &mut Problem,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) -> VariableMap {
    info!("Adding variables to problem...");
    let variables = VariableMap {};

    for asset in filter_assets(assets, year) {
        for flow in asset.process.flows.iter() {
            for time_slice in model.time_slice_info.iter_ids() {
                let _coeff = calculate_cost_coefficient(year, asset, flow, time_slice);

                // **TODO**: Create variable in _problem with cost coefficients and store the
                // resulting variable object in the variable map for later use
            }
        }
    }

    variables
}

/// Calculate the cost coefficient for a decision variable
fn calculate_cost_coefficient(
    _year: u32,
    _asset: &Asset,
    _flow: &ProcessFlow,
    _time_slice: &TimeSliceID,
) -> f64 {
    f64::NAN
}

/// Add asset-level input-output commodity balances
fn add_commodity_balance_constraints(
    _problem: &mut Problem,
    _variables: &VariableMap,
    _model: &Model,
    _assets: &AssetPool,
) {
    info!("Adding commodity balance constraints...");
}
