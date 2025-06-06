//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::asset::{Asset, AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use std::collections::HashMap;

mod constraints;
use constraints::{add_asset_constraints, CommodityBalanceKeys, ConstraintKeys};

/// A map of commodity flows calculated during the optimisation
pub type FlowMap = IndexMap<(AssetRef, CommodityID, TimeSliceID), f64>;

/// A decision variable in the optimisation
///
/// Note that this type does **not** include the value of the variable; it just refers to a
/// particular column of the problem.
type Variable = highs::Col;

/// Actual utilisation in last milestone year.
///
/// We group by commodity and time slice first to make it easier to look up values by asset ID.
pub type UtilisationMap = HashMap<(CommodityID, TimeSliceID), HashMap<AssetRef, f64>>;

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
pub struct VariableMap(IndexMap<(AssetRef, TimeSliceID), Variable>);

impl VariableMap {
    /// Get the [`Variable`] corresponding to the given parameters.
    fn get(&self, asset: &AssetRef, time_slice: &TimeSliceID) -> Variable {
        let key = (asset.clone(), time_slice.clone());

        *self
            .0
            .get(&key)
            .expect("No variable found for given params")
    }
}

/// The solution to the dispatch optimisation problem
pub struct Solution<'a> {
    solution: highs::Solution,
    variables: VariableMap,
    time_slice_info: &'a TimeSliceInfo,
    constraint_keys: ConstraintKeys,
}

impl Solution<'_> {
    /// Create a map of commodity flows for each asset's coeffs at every time slice.
    ///
    /// Note that this only includes commodity flows which relate to assets, so not every commodity
    /// in the simulation will necessarily be represented.
    pub fn create_flow_map(&self) -> FlowMap {
        // The decision variables represent assets' activity levels, not commodity flows. We
        // multiply this value by the flow coeffs to get commodity flows.
        self.variables
            .0
            .keys()
            .zip(self.solution.columns())
            .flat_map(|((asset, time_slice), activity)| {
                asset.iter_flows().map(move |flow| {
                    (
                        (asset.clone(), flow.commodity.id.clone(), time_slice.clone()),
                        activity * flow.coeff,
                    )
                })
            })
            .collect()
    }

    /// Store the actual utilisation from the previous milestone year in a map
    pub fn create_utilisation_map(&self) -> UtilisationMap {
        let mut utilisations = HashMap::new();
        for ((asset, commodity_id, time_slice), flow) in self.create_flow_map() {
            let map = utilisations
                .entry((commodity_id.clone(), time_slice.clone()))
                .or_insert_with(HashMap::new);
            map.insert(asset, flow);
        }

        utilisations
    }

    /// Helper function for iterating over commodity balance or demand duals
    fn iter_balance_duals<'a>(
        &'a self,
        keys: &'a CommodityBalanceKeys,
    ) -> impl Iterator<Item = (&'a CommodityID, &'a RegionID, &'a TimeSliceID, f64)> {
        // Each constraint applies to a particular time slice selection, depending on time slice
        // level. Where this covers multiple timeslices, we return the same dual for each
        // individual timeslice.
        keys.zip_duals(self.solution.dual_rows()).flat_map(
            |((commodity_id, region_id, ts_selection), price)| {
                ts_selection
                    .iter(self.time_slice_info)
                    .map(move |(ts, _)| (commodity_id, region_id, ts, price))
            },
        )
    }

    /// Keys and dual values for commodity balance constraints.
    pub fn iter_commodity_balance_duals(
        &self,
    ) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, f64)> {
        self.iter_balance_duals(&self.constraint_keys.commodity_balance_keys)
    }

    /// Keys and dual values for demand satisfaction constraints
    pub fn iter_demand_duals(
        &self,
    ) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, f64)> {
        self.iter_balance_duals(&self.constraint_keys.demand_keys)
    }

    /// Keys and dual values for capacity constraints.
    pub fn iter_capacity_duals(&self) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, f64)> {
        self.constraint_keys
            .capacity_keys
            .zip_duals(self.solution.dual_rows())
            .map(|((asset, time_slice), dual)| (asset, time_slice, dual))
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
pub fn perform_dispatch_optimisation<'a>(
    model: &'a Model,
    assets: &AssetPool,
    year: u32,
) -> Result<Solution<'a>> {
    // Set up problem
    let mut problem = Problem::default();
    let variables = add_variables(&mut problem, model, assets, year);

    // Add constraints
    let constraint_keys = add_asset_constraints(&mut problem, &variables, model, assets, year);

    // Solve problem
    let mut highs_model = problem.optimise(Sense::Minimise);

    // **HACK**: Dump output of HiGHS solver to stdout. Among other things, this includes the
    // objective value for the solution. Sadly it doesn't go via our logger, so this information
    // will not be included in the log file.
    //
    // Should be removed when we write the objective value to output data properly. See:
    //   https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/428
    enable_highs_logging(&mut highs_model);

    // Solve model
    let solution = highs_model.solve();
    match solution.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            solution: solution.get_solution(),
            variables,
            time_slice_info: &model.time_slice_info,
            constraint_keys,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    }
}

/// Enable logging for the HiGHS solver
fn enable_highs_logging(model: &mut highs::Model) {
    // **HACK**: Skip this step if logging is disabled (e.g. when running tests)
    if let Ok(log_level) = std::env::var("MUSE2_LOG_LEVEL") {
        if log_level.eq_ignore_ascii_case("off") {
            return;
        }
    }

    model.set_option("log_to_console", true);
    model.set_option("output_flag", true);
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
    let mut variables = VariableMap::default();

    for asset in assets.iter() {
        for time_slice in model.time_slice_info.iter_ids() {
            let coeff = calculate_cost_coefficient(asset, year, time_slice);
            let var = problem.add_column(coeff, 0.0..);
            let key = (asset.clone(), time_slice.clone());
            let existing = variables.0.insert(key, var).is_some();
            assert!(!existing, "Duplicate entry for var");
        }
    }

    variables
}

/// Calculate the cost coefficient for a decision variable
fn calculate_cost_coefficient(asset: &Asset, year: u32, time_slice: &TimeSliceID) -> f64 {
    // The cost for all commodity flows (including levies/incentives)
    let flows_cost: f64 = asset
        .iter_flows()
        .map(|flow| flow.get_total_cost(&asset.region_id, year, time_slice))
        .sum();

    asset.process_parameter.variable_operating_cost + flows_cost
}
