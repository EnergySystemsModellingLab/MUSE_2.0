//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::asset::{Asset, AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::output::DataWriter;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Activity, Flow, Money, MoneyPerActivity, MoneyPerFlow, UnitType};
use anyhow::{anyhow, ensure, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use itertools::{chain, iproduct};
use log::debug;
use std::ops::Range;

mod constraints;
use constraints::{add_asset_constraints, ConstraintKeys};

/// A map of commodity flows calculated during the optimisation
pub type FlowMap = IndexMap<(AssetRef, CommodityID, TimeSliceID), Flow>;

/// A nested flow map, with flows grouped by commodity and region
pub type NestedFlowMap = IndexMap<(CommodityID, RegionID), IndexMap<TimeSliceID, Flow>>;

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

    /// Iterate over the variable map
    fn iter(&self) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, Variable)> {
        self.0
            .iter()
            .map(|((asset, time_slice), var)| (asset, time_slice, *var))
    }
}

/// The solution to the dispatch optimisation problem
pub struct Solution<'a> {
    solution: highs::Solution,
    variables: VariableMap,
    active_asset_var_idx: Range<usize>,
    candidate_asset_var_idx: Range<usize>,
    time_slice_info: &'a TimeSliceInfo,
    constraint_keys: ConstraintKeys,
    /// The objective value for the solution
    pub objective_value: Money,
}

impl Solution<'_> {
    /// Create a map of commodity flows for each asset's coeffs at every time slice.
    ///
    /// Note that this only includes commodity flows which relate to assets, so not every commodity
    /// in the simulation will necessarily be represented.
    pub fn create_flow_map(&self) -> FlowMap {
        // The decision variables represent assets' activity levels, not commodity flows. We
        // multiply this value by the flow coeffs to get commodity flows.
        let mut flows = FlowMap::new();
        for (asset, time_slice, activity) in self.iter_activity_for_active() {
            for flow in asset.iter_flows() {
                let flow_key = (asset.clone(), flow.commodity.id.clone(), time_slice.clone());
                let flow_value = activity * flow.coeff;
                flows.insert(flow_key, flow_value);
            }
        }

        flows
    }

    /// Create a nested flow map, with flows grouped by commodity and region
    pub fn create_nested_flow_map(&self) -> NestedFlowMap {
        let flow_map = self.create_flow_map();
        let mut nested_flow_map = NestedFlowMap::new();
        for ((asset, commodity_id, time_slice), flow) in flow_map.iter() {
            *nested_flow_map
                .entry((commodity_id.clone(), asset.region_id.clone()))
                .or_default()
                .entry(time_slice.clone())
                .or_insert(Flow(0.0)) += *flow;
        }
        nested_flow_map
    }

    /// Activity for each active asset
    pub fn iter_activity(&self) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, Activity)> {
        self.variables
            .0
            .keys()
            .zip(self.solution.columns())
            .map(|((asset, time_slice), activity)| (asset, time_slice, Activity(*activity)))
    }

    /// Activity for each active asset
    fn iter_activity_for_active(
        &self,
    ) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, Activity)> {
        self.zip_var_keys_with_output(&self.active_asset_var_idx, self.solution.columns())
    }

    /// Reduced costs for candidate assets
    pub fn iter_reduced_costs_for_candidates(
        &self,
    ) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, MoneyPerActivity)> {
        self.zip_var_keys_with_output(&self.candidate_asset_var_idx, self.solution.dual_columns())
    }

    /// Keys and dual values for commodity balance constraints.
    pub fn iter_commodity_balance_duals(
        &self,
    ) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, MoneyPerFlow)> {
        // Each commodity balance constraint applies to a particular time slice
        // selection (depending on time slice level). Where this covers multiple timeslices,
        // we return the same dual for each individual timeslice.
        self.constraint_keys
            .commodity_balance_keys
            .zip_duals(self.solution.dual_rows())
            .flat_map(|((commodity_id, region_id, ts_selection), price)| {
                ts_selection
                    .iter(self.time_slice_info)
                    .map(move |(ts, _)| (commodity_id, region_id, ts, price))
            })
    }

    /// Keys and dual values for activity constraints.
    pub fn iter_activity_duals(
        &self,
    ) -> impl Iterator<Item = (&AssetRef, &TimeSliceID, MoneyPerActivity)> {
        self.constraint_keys
            .activity_keys
            .zip_duals(self.solution.dual_rows())
            .map(|((asset, time_slice), dual)| (asset, time_slice, dual))
    }

    /// Zip a subset of keys in the variable map with a subset of the given output variable.
    ///
    /// # Arguments
    ///
    /// * `variable_idx` - The subset of variables to look at
    /// * `output` - The output variable of interest
    fn zip_var_keys_with_output<'a, T: UnitType>(
        &'a self,
        variable_idx: &Range<usize>,
        output: &'a [f64],
    ) -> impl Iterator<Item = (&'a AssetRef, &'a TimeSliceID, T)> {
        let keys = self.variables.0.keys().skip(variable_idx.start);
        assert!(keys.len() >= variable_idx.len());

        keys.zip(output[variable_idx.clone()].iter())
            .map(|((asset, time_slice), value)| (asset, time_slice, T::new(*value)))
    }
}

/// Try to solve the model, returning an error if the model is incoherent or result is non-optimal
pub fn solve_optimal(model: highs::Model) -> Result<highs::SolvedModel> {
    let solved = model
        .try_solve()
        .map_err(|err| anyhow!("Incoherent model: {err:?}"))?;

    let status = solved.status();
    ensure!(
        status == HighsModelStatus::Optimal,
        "Could not find optimal result for model: {status:?}"
    );

    Ok(solved)
}

/// Perform the dispatch optimisation.
///
/// If `commodities` is provided, the commodity balance constraints will only be added for these
/// commodities, else they will be added for all commodities.
///
/// For a detailed description, please see the [dispatch optimisation formulation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/model/dispatch_optimisation.html
///
/// # Arguments
///
/// * `model` - The model
/// * `asset_pool` - The asset pool
/// * `candidate_assets` - Candidate assets for inclusion in active pool
/// * `commodities` - The subset of commodities to apply constraints to
/// * `year` - Current milestone year
/// * `run_number` - Which dispatch run for the current year this is
/// * `data_writer` - For saving output data
///
/// # Returns
///
/// A solution containing new commodity flows for assets and prices for (some) commodities.
pub fn perform_dispatch_optimisation<'a>(
    model: &'a Model,
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    commodities: Option<&[CommodityID]>,
    year: u32,
    run_number: u32,
    writer: &mut DataWriter,
) -> Result<Solution<'a>> {
    let solution = perform_dispatch_optimisation_no_save(
        model,
        asset_pool,
        candidate_assets,
        commodities,
        year,
    )?;

    writer.write_debug_info(year, run_number, &solution)?;

    Ok(solution)
}

/// Perform the dispatch optimisation without saving output data
fn perform_dispatch_optimisation_no_save<'a>(
    model: &'a Model,
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    commodities: Option<&[CommodityID]>,
    year: u32,
) -> Result<Solution<'a>> {
    // Set up problem
    let mut problem = Problem::default();
    let mut variables = VariableMap::default();
    let active_asset_var_idx = add_variables(
        &mut problem,
        &mut variables,
        &model.time_slice_info,
        asset_pool.as_slice(),
        year,
    );
    let candidate_asset_var_idx = add_variables(
        &mut problem,
        &mut variables,
        &model.time_slice_info,
        candidate_assets,
        year,
    );

    // Add constraints
    let all_assets = chain(asset_pool.iter(), candidate_assets.iter());
    let mut all_commodities = Vec::new();
    let commodities = commodities.unwrap_or_else(|| {
        all_commodities = model.commodities.keys().cloned().collect();
        &all_commodities
    });
    let constraint_keys = add_asset_constraints(
        &mut problem,
        &variables,
        model,
        all_assets,
        commodities,
        year,
    );

    // Solve model
    let solution = solve_optimal(problem.optimise(Sense::Minimise))?;

    let objective_value = Money(solution.objective_value());
    debug!("Objective value: {objective_value}");

    Ok(Solution {
        solution: solution.get_solution(),
        variables,
        active_asset_var_idx,
        candidate_asset_var_idx,
        time_slice_info: &model.time_slice_info,
        constraint_keys,
        objective_value,
    })
}

/// Add variables to the optimisation problem.
///
/// # Arguments
///
/// * `problem` - The optimisation problem
/// * `variables` - The variable map
/// * `time_slice_info` - Information about assets
/// * `assets` - Assets to include
/// * `year` - Current milestone year
fn add_variables(
    problem: &mut Problem,
    variables: &mut VariableMap,
    time_slice_info: &TimeSliceInfo,
    assets: &[AssetRef],
    year: u32,
) -> Range<usize> {
    // This line **must** come before we add more variables
    let start = problem.num_cols();

    for (asset, time_slice) in iproduct!(assets.iter(), time_slice_info.iter_ids()) {
        let coeff = calculate_cost_coefficient(asset, year, time_slice);
        let var = problem.add_column(coeff.value(), 0.0..);
        let key = (asset.clone(), time_slice.clone());
        let existing = variables.0.insert(key, var).is_some();
        assert!(!existing, "Duplicate entry for var");
    }

    start..problem.num_cols()
}

/// Calculate the cost coefficient for a decision variable
fn calculate_cost_coefficient(
    asset: &Asset,
    year: u32,
    time_slice: &TimeSliceID,
) -> MoneyPerActivity {
    asset.get_operating_cost(year, time_slice)
}
