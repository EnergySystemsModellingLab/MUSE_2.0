use crate::asset::{AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::region::RegionID;
use crate::simulation::investment_tools::strategies::Strategy;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem};
use indexmap::IndexMap;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Represents different types of optimization variables
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VariableType {
    /// Capacity
    Capacity(AssetRef),
    /// Activity level in a time slice
    Activity(AssetRef, TimeSliceID),
    /// Unmet demand
    UnmetDemand(CommodityID, RegionID, TimeSliceID),
}

/// Variable map for optimization
#[derive(Default)]
pub struct VariableMap {
    pub variables: IndexMap<VariableType, Variable>,

    /// Also keep separate maps for different types of variables
    pub existing_capacity_vars: IndexMap<AssetRef, Variable>,
    pub candidate_capacity_vars: IndexMap<AssetRef, Variable>,
    pub existing_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    pub candidate_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    pub unmet_demand_vars: IndexMap<(CommodityID, RegionID, TimeSliceID), Variable>,
}

/// Solution to the optimisation problem
pub struct Solution {
    solution: highs::Solution,
    variables: VariableMap,
}

/// Add a capacity variable for an existing asset
/// This also constrains the capacity to the asset's existing capacity
pub fn add_existing_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    col_factor: f64,
) {
    let capacity = asset_ref.capacity;
    let var = problem.add_column(col_factor, capacity.value()..capacity.value());
    let var_type = VariableType::Capacity(asset_ref.clone());
    variables.variables.insert(var_type.clone(), var);
    variables.existing_capacity_vars.insert(asset_ref, var);
}

/// Add a capacity variable for a candidate asset
pub fn add_candidate_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    col_factor: f64,
) {
    let var = problem.add_column(col_factor, 0.0..);
    let var_type = VariableType::Capacity(asset_ref.clone());
    variables.variables.insert(var_type.clone(), var);
    variables.candidate_capacity_vars.insert(asset_ref, var);
}

/// Add an activity variable for an existing asset in a time slice
pub fn add_existing_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    time_slice: TimeSliceID,
    col_factor: f64,
) {
    let var = problem.add_column(col_factor, 0.0..);
    let var_type = VariableType::Activity(asset_ref.clone(), time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .existing_activity_vars
        .insert((asset_ref, time_slice), var);
}

/// Add an activity variable for a candidate asset in a time slice
pub fn add_candidate_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    time_slice: TimeSliceID,
    col_factor: f64,
) {
    let var = problem.add_column(col_factor, 0.0..);
    let var_type = VariableType::Activity(asset_ref.clone(), time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .candidate_activity_vars
        .insert((asset_ref, time_slice), var);
}

/// Perform optimisation for a given strategy
pub fn perform_optimisation(
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    strategy: &dyn Strategy,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();
    let mut variables = VariableMap::default();

    // Add variables
    strategy.add_variables(
        &mut problem,
        &mut variables,
        asset_pool,
        candidate_assets,
        time_slice_info,
        reduced_costs,
    );

    // Add constraints
    strategy.add_constraints(&mut problem, &variables);

    // Solve problem
    let highs_model = problem.optimise(strategy.sense());

    // Solve model
    let solution = highs_model.solve();
    match solution.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            solution: solution.get_solution(),
            variables,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    }
}
