//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use crate::process::ProcessFlow;
use crate::simulation::filter_assets;
use crate::time_slice::TimeSliceID;
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use log::{error, info};
use std::rc::Rc;

/// The solution to the dispatch optimisation problem (PLACEHOLDER)
pub struct Solution;

// A decision variable in the optimisation
type Variable = highs::Col;

/// A map for easy lookup of variables in the optimisation.
type VariableMap = IndexMap<VariableMapKey, Variable>;

#[derive(Eq, PartialEq, Hash)]
struct VariableMapKey {
    asset_id: u32,
    commodity_id: Rc<str>,
    time_slice: TimeSliceID,
}

impl VariableMapKey {
    /// Create a new [`VariableMapKey`]
    fn new(asset_id: u32, commodity_id: Rc<str>, time_slice: TimeSliceID) -> Self {
        Self {
            asset_id,
            commodity_id,
            time_slice,
        }
    }
}

/// Perform the dispatch optimisation.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns
///
/// A solution containing new commodity flows for assets and prices for (some) commodities.
pub fn perform_dispatch_optimisation(model: &Model, assets: &AssetPool, year: u32) -> Solution {
    info!("Performing dispatch optimisation...");

    // Set up problem
    let mut problem = Problem::default();
    let variables = add_variables(&mut problem, model, assets, year);
    add_commodity_balance_constraints(&mut problem, &variables, model, assets, year);

    // Solve problem
    let solution = problem.optimise(Sense::Minimise).solve();

    let status = solution.status();
    if status != HighsModelStatus::Optimal {
        // **TODO**: Make this a hard error once the problem is actually solvable
        error!("Could not solve: {status:?}");
    }

    // **PLACEHOLDER**
    Solution {}
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
    problem: &mut Problem,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) -> VariableMap {
    info!("Adding variables to problem...");
    let mut variables = VariableMap::new();

    for asset in filter_assets(assets, year) {
        for flow in asset.process.flows.iter() {
            for time_slice in model.time_slice_info.iter_ids() {
                let coeff = calculate_cost_coefficient(year, asset, flow, time_slice);

                // var's value must be <= 0 for inputs and >= 0 for outputs
                let var = if flow.flow < 0.0 {
                    problem.add_column(coeff, ..=0.0)
                } else {
                    problem.add_column(coeff, 0.0..)
                };

                let key = VariableMapKey::new(
                    asset.id,
                    Rc::clone(&flow.commodity.id),
                    time_slice.clone(),
                );

                let existing = variables.insert(key, var).is_some();
                assert!(!existing, "Duplicate entry for var");
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
    // **PLACEHOLDER**
    1.0
}

/// Add asset-level input-output commodity balances
fn add_commodity_balance_constraints(
    _problem: &mut Problem,
    _variables: &VariableMap,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) {
    info!("Adding commodity balance constraints...");

    for asset in filter_assets(assets, year) {
        for _flow in asset.process.flows.iter() {
            for _time_slice in model.time_slice_info.iter_ids() {
                // TODO: Add constraints

                // You add constraints as rows to the problem, like so;
                //
                // let var = variables.get(asset.id, &flow.commodity.id, &time_slice);
                // problem.add_row(-1..=1, &[(var, 1.0)]);
                //
                // This means "var must be >= -1 and <= 1". See highs documentation for more
                // examples.
            }
        }
    }
}
