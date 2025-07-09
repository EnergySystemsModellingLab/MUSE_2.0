use crate::asset::{AssetPool, AssetRef};
use crate::model::Model;
use crate::simulation::lcox::costs::{
    activity_cost_for_asset, activity_cost_for_candidate, annual_fixed_cost_for_asset,
    annual_fixed_cost_for_candidate,
};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use itertools::iproduct;

/// A decision variable in the optimisation
type Variable = highs::Col;

/// Represents different types of optimization variables
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VariableType {
    /// Capacity investment for existing assets
    AssetCapacity(AssetRef),
    /// Capacity investment for candidate assets
    CandidateCapacity(AssetRef),
    /// Activity level for existing assets in each time slice
    AssetActivity(AssetRef, TimeSliceID),
    /// Activity level for candidate assets in each time slice
    CandidateActivity(AssetRef, TimeSliceID),
}

/// A comprehensive variable map for LCOX optimization
#[derive(Default)]
pub struct VariableMap {
    /// Maps variable types to their corresponding optimization variables
    variables: IndexMap<VariableType, Variable>,

    /// Separate collections for efficient access by variable type
    asset_capacity_vars: IndexMap<AssetRef, Variable>,
    candidate_capacity_vars: IndexMap<AssetRef, Variable>,
    asset_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    candidate_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
}

/// Add a capacity variable for an existing asset
fn add_asset_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
) {
    let cost = annual_fixed_cost_for_asset(&asset_ref);
    let capacity = asset_ref.capacity;
    let var = problem.add_column(cost.value(), capacity.value()..capacity.value());
    let var_type = VariableType::AssetCapacity(asset_ref.clone());
    variables.variables.insert(var_type.clone(), var);
    variables.asset_capacity_vars.insert(asset_ref, var);
}

/// Add a capacity variable for a candidate asset
fn add_candidate_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
) {
    let cost = annual_fixed_cost_for_candidate(&asset_ref);
    let var = problem.add_column(cost.value(), 0.0..);
    let var_type = VariableType::CandidateCapacity(asset_ref.clone());
    variables.variables.insert(var_type.clone(), var);
    variables.candidate_capacity_vars.insert(asset_ref, var);
}

/// Add an activity variable for an existing asset in a time slice
fn add_asset_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) {
    let cost = activity_cost_for_asset(&asset_ref, reduced_costs, time_slice.clone());
    let var = problem.add_column(cost.value(), 0.0..);
    let var_type = VariableType::AssetActivity(asset_ref.clone(), time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .asset_activity_vars
        .insert((asset_ref, time_slice), var);
}

/// Add an activity variable for a candidate asset in a time slice
fn add_candidate_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) {
    let cost = activity_cost_for_candidate(&asset_ref, reduced_costs, time_slice.clone());
    let var = problem.add_column(cost.value(), 0.0..);
    let var_type = VariableType::CandidateActivity(asset_ref.clone(), time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .candidate_activity_vars
        .insert((asset_ref, time_slice), var);
}

fn add_variables_for_existing(
    problem: &mut Problem,
    variables: &mut VariableMap,
    assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
) {
    // Add capacity variables
    for asset in assets {
        add_asset_capacity_variable(problem, variables, asset.clone());
    }

    // Add activity variables
    for (asset, time_slice) in iproduct!(assets.iter(), time_slice_info.iter_ids()) {
        add_asset_activity_variable(
            problem,
            variables,
            asset.clone(),
            reduced_costs,
            time_slice.clone(),
        );
    }
}

fn add_variables_for_candidates(
    problem: &mut Problem,
    variables: &mut VariableMap,
    assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
) {
    // Add capacity variables
    for asset in assets {
        add_candidate_capacity_variable(problem, variables, asset.clone());
    }

    // Add activity variables
    for (asset, time_slice) in iproduct!(assets.iter(), time_slice_info.iter_ids()) {
        add_candidate_activity_variable(
            problem,
            variables,
            asset.clone(),
            reduced_costs,
            time_slice.clone(),
        );
    }
}

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
