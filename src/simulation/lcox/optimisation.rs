use crate::asset::{AssetID, AssetPool, AssetRef};
use crate::model::Model;
use crate::time_slice::TimeSliceID;
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;

/// A decision variable in the optimisation
type Variable = highs::Col;

/// Represents different types of optimization variables
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VariableType {
    /// Capacity investment for existing assets
    AssetCapacity(AssetID),
    /// Capacity investment for candidate assets
    CandidateCapacity(AssetRef),
    /// Activity level for existing assets in each time slice
    AssetActivity(AssetID, TimeSliceID),
    /// Activity level for candidate assets in each time slice
    CandidateActivity(AssetRef, TimeSliceID),
}

/// A comprehensive variable map for LCOX optimization
#[derive(Default)]
pub struct VariableMap {
    /// Maps variable types to their corresponding optimization variables
    variables: IndexMap<VariableType, Variable>,

    /// Separate collections for efficient access by variable type
    asset_capacity_vars: IndexMap<AssetID, Variable>,
    candidate_capacity_vars: IndexMap<AssetRef, Variable>,
    asset_activity_vars: IndexMap<(AssetID, TimeSliceID), Variable>,
    candidate_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
}

/// Add a capacity variable for an existing asset
pub fn add_asset_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_id: AssetID,
) {
    let var = problem.add_column(0.0, 0.0..);
    let var_type = VariableType::AssetCapacity(asset_id);
    variables.variables.insert(var_type.clone(), var);
    variables.asset_capacity_vars.insert(asset_id, var);
}

/// Add a capacity variable for a candidate asset
pub fn add_candidate_capacity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
) {
    let var = problem.add_column(0.0, 0.0..);
    let var_type = VariableType::CandidateCapacity(asset_ref.clone());
    variables.variables.insert(var_type.clone(), var);
    variables.candidate_capacity_vars.insert(asset_ref, var);
}

/// Add an activity variable for an existing asset in a time slice
pub fn add_asset_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_id: AssetID,
    time_slice: TimeSliceID,
) {
    let var = problem.add_column(0.0, 0.0..);
    let var_type = VariableType::AssetActivity(asset_id, time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .asset_activity_vars
        .insert((asset_id, time_slice), var);
}

/// Add an activity variable for a candidate asset in a time slice
pub fn add_candidate_activity_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    asset_ref: AssetRef,
    time_slice: TimeSliceID,
) {
    let var = problem.add_column(0.0, 0.0..);
    let var_type = VariableType::CandidateActivity(asset_ref.clone(), time_slice.clone());
    variables.variables.insert(var_type.clone(), var);
    variables
        .candidate_activity_vars
        .insert((asset_ref, time_slice), var);
}

pub struct Solution {
    solution: highs::Solution,
    variables: VariableMap,
}

pub fn perform_lcox_optimisation(
    model: &Model,
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    year: u32,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();
    let mut variables = VariableMap::default();

    // Add variables

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
