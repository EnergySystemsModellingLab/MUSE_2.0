use crate::asset::{AssetPool, AssetRef};
use crate::model::Model;
use crate::simulation::lcox::constraints::{
    add_activity_constraints_for_candidates, add_activity_constraints_for_existing,
    add_capacity_constraints_for_candidates, add_demand_constraints,
};
use crate::simulation::lcox::costs::{
    activity_cost_for_candidate, activity_cost_for_existinng, annual_fixed_cost_for_candidate,
    annual_fixed_cost_for_existing,
};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use itertools::iproduct;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Represents different types of optimization variables
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VariableType {
    /// Capacity
    Capacity(AssetRef),
    /// Activity level in a time slice
    Activity(AssetRef, TimeSliceID),
}

/// Variable map for optimization
#[derive(Default)]
pub struct VariableMap {
    variables: IndexMap<VariableType, Variable>,

    /// Also keep separate maps for different types of variables
    existing_capacity_vars: IndexMap<AssetRef, Variable>,
    candidate_capacity_vars: IndexMap<AssetRef, Variable>,
    existing_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    candidate_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
}

/// Add a capacity variable for an existing asset
/// This also constrains the capacity to the asset's existing capacity
fn add_existing_capacity_variable(
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
fn add_candidate_capacity_variable(
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
fn add_existing_activity_variable(
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
fn add_candidate_activity_variable(
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

/// Specific to LCOX
fn add_variables_for_existing(
    problem: &mut Problem,
    variables: &mut VariableMap,
    assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
) {
    // Add capacity variables
    for asset in assets {
        let col_factor = annual_fixed_cost_for_existing(asset);
        add_existing_capacity_variable(problem, variables, asset.clone(), col_factor.value());
    }

    // Add activity variables
    for (asset, time_slice) in iproduct!(assets.iter(), time_slice_info.iter_ids()) {
        let col_factor = activity_cost_for_existinng(asset, reduced_costs, time_slice.clone());
        add_existing_activity_variable(
            problem,
            variables,
            asset.clone(),
            time_slice.clone(),
            col_factor.value(),
        );
    }
}

/// Specific to LCOX
fn add_variables_for_candidates(
    problem: &mut Problem,
    variables: &mut VariableMap,
    assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
) {
    // Add capacity variables
    for asset in assets {
        let col_factor = annual_fixed_cost_for_candidate(asset);
        add_candidate_capacity_variable(problem, variables, asset.clone(), col_factor.value());
    }

    // Add activity variables
    for (asset, time_slice) in iproduct!(assets.iter(), time_slice_info.iter_ids()) {
        let col_factor = activity_cost_for_candidate(asset, reduced_costs, time_slice.clone());
        add_candidate_activity_variable(
            problem,
            variables,
            asset.clone(),
            time_slice.clone(),
            col_factor.value(),
        );
    }
}

/// Solution to the optimisation problem
pub struct Solution {
    solution: highs::Solution,
    variables: VariableMap,
}

pub fn perform_lcox_optimisation(
    model: &Model,
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();
    let mut variables = VariableMap::default();

    // Add variables
    add_variables_for_existing(
        &mut problem,
        &mut variables,
        asset_pool.as_slice(),
        &time_slice_info,
        &reduced_costs,
    );
    add_variables_for_candidates(
        &mut problem,
        &mut variables,
        candidate_assets,
        &time_slice_info,
        &reduced_costs,
    );

    // Add constraints
    add_constraints(&mut problem, &variables);

    // Solve problem
    let highs_model = problem.optimise(Sense::Minimise);

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

/// Specific for LCOX
fn add_constraints(problem: &mut Problem, variables: &VariableMap) {
    add_activity_constraints_for_existing(problem, &variables.existing_activity_vars);
    add_activity_constraints_for_candidates(
        problem,
        &variables.candidate_capacity_vars,
        &variables.candidate_activity_vars,
    );
    add_capacity_constraints_for_candidates(problem, &variables.candidate_capacity_vars);
    add_demand_constraints(
        problem,
        &variables.existing_activity_vars,
        &variables.candidate_activity_vars,
    );
}
