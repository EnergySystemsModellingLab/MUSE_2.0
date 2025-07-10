use crate::asset::AssetRef;
use crate::simulation::investment_tools::strategies::Strategy;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Flow, MoneyPerActivity, MoneyPerCapacity};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Map storing cost coefficients for each variable type
pub struct CostCoefficientsMap {
    pub capacity_cost: MoneyPerCapacity,
    pub activity_costs: IndexMap<TimeSliceID, MoneyPerActivity>,
}

/// Variable map for optimization
pub struct VariableMap {
    pub capacity_var: Variable,
    pub activity_vars: IndexMap<TimeSliceID, Variable>,
}

/// Solution to the optimisation problem
pub struct Solution {
    solution: highs::Solution,
    variables: VariableMap,
}

/// Add variables to the problem based on cost coefficients
pub fn add_variables(
    problem: &mut Problem,
    cost_coefficients: &CostCoefficientsMap,
) -> VariableMap {
    // Create capacity variable
    let capacity_var = problem.add_column(cost_coefficients.capacity_cost.value(), 0.0..);

    // Create activity variables
    let mut activity_vars = IndexMap::new();
    for (time_slice, cost) in cost_coefficients.activity_costs.iter() {
        let var = problem.add_column(cost.value(), 0.0..);
        activity_vars.insert(time_slice.clone(), var);
    }

    VariableMap {
        capacity_var,
        activity_vars,
    }
}

/// Perform optimisation for a given strategy
pub fn perform_optimisation(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    strategy: &dyn Strategy,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();

    // Calculate cost coefficients
    let cost_coefficients =
        strategy.calculate_cost_coefficients(asset, time_slice_info, reduced_costs);

    // Add variables
    let variables = add_variables(&mut problem, &cost_coefficients);

    // Add constraints
    strategy.add_constraints(&mut problem, asset, &variables, demand);

    // Solve problem
    let highs_model = problem.optimise(strategy.sense());

    // Solve model
    let solved_model = highs_model.solve();
    match solved_model.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            solution: solved_model.get_solution(),
            variables,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    }
}
