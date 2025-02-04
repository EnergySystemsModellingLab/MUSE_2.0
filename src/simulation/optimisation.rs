//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use crate::process::ProcessFlow;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use log::{error, info};
use std::iter;
use std::rc::Rc;

/// A decision variable in the optimisation
///
/// Note that this type does **not** include the value of the variable; it just refers to a
/// particular column of the problem.
type Variable = highs::Col;

/// A map for easy lookup of variables in the problem.
///
/// The entries are ordered (see [`IndexMap`]).
///
/// We use this data structure for two things:
///
/// 1. In order define constraints for the optimisation
/// 2. To keep track of the combination of parameters that each variable corresponds to, for when we
///    are reading the results of the optimisation.
#[derive(Default)]
pub struct VariableMap(IndexMap<VariableMapKey, Variable>);

impl VariableMap {
    /// Get the [`Variable`] corresponding to the given parameters.
    fn get(&self, asset_id: u32, commodity_id: &Rc<str>, time_slice: &TimeSliceID) -> Variable {
        let key = VariableMapKey {
            asset_id,
            commodity_id: Rc::clone(commodity_id),
            time_slice: time_slice.clone(),
        };

        *self
            .0
            .get(&key)
            .expect("No variable found for given params")
    }
}

/// A key for a [`VariableMap`]
#[derive(Eq, PartialEq, Hash)]
pub struct VariableMapKey {
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

/// The solution to the dispatch optimisation problem
pub struct Solution {
    variables: VariableMap,
    solution: highs::Solution,
}

impl Solution {
    /// Iterate over the newly calculated commodity flows for assets.
    ///
    /// Note that this only includes commodity flows which relate to assets, so not every commodity
    /// in the simulation will necessarily be represented.
    pub fn iter_commodity_flows_for_assets(&self) -> impl Iterator<Item = (&VariableMapKey, f64)> {
        self.variables
            .0
            .keys()
            .zip(self.solution.columns().iter().copied())
    }

    /// Iterate over the newly calculated commodity prices.
    ///
    /// Note that there may only be prices for a subset of the commodities; the rest will need to be
    /// calculated in another way.
    pub fn iter_commodity_prices(&self) -> impl Iterator<Item = (&Rc<str>, f64)> {
        // **PLACEHOLDER**
        iter::empty()
    }
}

/// Perform the dispatch optimisation.
///
/// For a detailed description, please see the [dispatch optimisation formulation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html
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

    // Add constraints
    add_asset_contraints(&mut problem, &variables, model, assets, year);

    // Solve problem
    let solution = problem.optimise(Sense::Minimise).solve();

    let status = solution.status();
    if status != HighsModelStatus::Optimal {
        // **TODO**: Make this a hard error once the problem is actually solvable
        error!("Could not solve: {status:?}");
    }

    Solution {
        variables,
        solution: solution.get_solution(),
    }
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
    let mut variables = VariableMap::default();

    for asset in assets.iter() {
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

                let existing = variables.0.insert(key, var).is_some();
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

/// Add asset-level constraints
fn add_asset_contraints(
    problem: &mut Problem,
    variables: &VariableMap,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) {
    add_commodity_balance_constraints(problem, variables, model, assets, year);

    // **TODO**: Currently it's safe to assume all process flows are non-flexible, as we enforce
    // this when reading data in. Once we've added support for flexible process flows, we will
    // need to add different constraints for assets with flexible and non-flexible flows.
    //
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/360
    add_fixed_asset_constraints(problem, variables, assets, &model.time_slice_info);

    add_asset_capacity_constraints(problem, variables, assets, &model.time_slice_info);
}

/// Add asset-level input-output commodity balances
fn add_commodity_balance_constraints(
    _problem: &mut Problem,
    _variables: &VariableMap,
    _model: &Model,
    _assets: &AssetPool,
    _year: u32,
) {
    info!("Adding commodity balance constraints...");

    // Sanity check: we rely on the first n values of the dual row values corresponding to the
    // commodity constraints, so these must be the first rows
    assert!(
        _problem.num_rows() == 0,
        "Commodity balance constraints must be added before other constraints"
    );
}

/// Add constraints for non-flexible assets.
///
/// Non-flexible assets are those which have a fixed ratio between inputs and outputs.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#non-flexible-assets
fn add_fixed_asset_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    assets: &AssetPool,
    time_slice_info: &TimeSliceInfo,
) {
    info!("Adding constraints for non-flexible assets...");

    for asset in assets.iter() {
        // Get first PAC. unwrap is safe because all processes have at least one PAC.
        let pac1 = asset.process.iter_pacs().next().unwrap();

        for time_slice in time_slice_info.iter_ids() {
            let pac_var = variables.get(asset.id, &pac1.commodity.id, time_slice);
            let pac_term = (pac_var, -1.0 / pac1.flow);

            for flow in asset.process.flows.iter() {
                // Don't add a constraint for the PAC itself
                if Rc::ptr_eq(&flow.commodity, &pac1.commodity) {
                    continue;
                }

                // We are enforcing that (var / flow) - (pac_var / pac_flow) = 0
                let var = variables.get(asset.id, &flow.commodity.id, time_slice);
                problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
            }
        }
    }
}

/// Add asset-level capacity and availability constraints.
///
/// For every asset at every time slice, the sum of the commodity flows for PACs must not exceed the
/// capacity limits, which are a product of the annual capacity, time slice length and process
/// availability.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#asset-level-capacity-and-availability-constraints
fn add_asset_capacity_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    assets: &AssetPool,
    time_slice_info: &TimeSliceInfo,
) {
    info!("Adding asset-level capacity and availability constraints...");

    let mut terms = Vec::new();
    for asset in assets.iter() {
        for time_slice in time_slice_info.iter_ids() {
            let mut is_input = false; // NB: there will be at least one PAC
            for flow in asset.process.iter_pacs() {
                is_input = flow.flow < 0.0; // NB: PACs will be all inputs or all outputs

                let var = variables.get(asset.id, &flow.commodity.id, time_slice);
                terms.push((var, 1.0));
            }

            let mut limits = asset.get_activity_limits(time_slice);

            // If it's an input flow, the q's will be negative, so we need to invert the limits
            if is_input {
                limits = -limits.end()..=-limits.start();
            }

            problem.add_row(limits, terms.drain(0..));
        }
    }
}
