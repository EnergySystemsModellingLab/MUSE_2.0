//! Optimisation problem for investment tools.
use super::coefficients::CoefficientsMap;
use super::constraints::{
    add_activity_constraints, add_capacity_constraint, add_demand_constraints,
};
use crate::asset::AssetRef;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Activity, Capacity, Flow};
use anyhow::{anyhow, Result};
use highs::{RowProblem as Problem, Sense};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Map storing variables for the optimisation problem
struct VariableMap {
    /// Capacity variable
    capacity_var: Variable,
    /// Activity variables in each time slice
    activity_vars: IndexMap<TimeSliceID, Variable>,
    // **TODO.**: VoLL variables (for LCOX)
}

/// Map containing optimisation results and coefficients
pub struct ResultsMap {
    /// Capacity variable
    pub capacity: Capacity,
    /// Activity variables in each time slice
    pub activity: IndexMap<TimeSliceID, Activity>,
}

/// Add variables to the problem based on cost coefficients
fn add_variables(problem: &mut Problem, cost_coefficients: &CoefficientsMap) -> VariableMap {
    // Create capacity variable
    let capacity_var = problem.add_column(cost_coefficients.capacity_coefficient.value(), 0.0..);

    // Create activity variables
    let mut activity_vars = IndexMap::new();
    for (time_slice, cost) in cost_coefficients.activity_coefficients.iter() {
        let var = problem.add_column(cost.value(), 0.0..);
        activity_vars.insert(time_slice.clone(), var);
    }

    VariableMap {
        capacity_var,
        activity_vars,
    }
}

/// Adds constraints to the problem.
fn add_constraints(
    problem: &mut Problem,
    asset: &AssetRef,
    variables: &VariableMap,
    demand: &HashMap<TimeSliceID, Flow>,
    time_slice_level: TimeSliceLevel,
    time_slice_info: &TimeSliceInfo,
) {
    add_capacity_constraint(problem, asset, variables.capacity_var);
    add_activity_constraints(
        problem,
        asset,
        variables.capacity_var,
        &variables.activity_vars,
    );
    add_demand_constraints(
        problem,
        time_slice_level,
        time_slice_info,
        demand,
        &variables.activity_vars,
    );
}

/// Performs optimisation for an asset, given the coefficients and demand.
///
/// Will either maximise or minimise the objective function, depending on the `sense` parameter.
///
/// **TODO.**: Will need to modify constraints to handle unmet demand variables in LCOX case
pub fn perform_optimisation(
    asset: &AssetRef,
    coefficients: &CoefficientsMap,
    demand: &HashMap<TimeSliceID, Flow>,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
    sense: Sense,
) -> Result<ResultsMap> {
    // Set up problem
    let mut problem = Problem::default();

    // Add variables
    let variables = add_variables(&mut problem, coefficients);

    // Add constraints
    add_constraints(
        &mut problem,
        asset,
        &variables,
        demand,
        time_slice_level,
        time_slice_info,
    );

    // Solve model
    let solution = problem
        .optimise(sense)
        .try_solve()
        .map_err(|status| anyhow!("Could not solve: {status:?}"))?
        .get_solution();
    let solution_values = solution.columns();
    Ok(ResultsMap {
        capacity: Capacity::new(solution_values[0]),
        activity: variables
            .activity_vars
            .keys()
            .zip(solution_values[1..].iter())
            .map(|(time_slice, &value)| (time_slice.clone(), Activity::new(value)))
            .collect(),
    })
}
