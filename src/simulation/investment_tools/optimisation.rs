//! Optimisation problem for investment tools.
use crate::asset::AssetRef;
use crate::simulation::investment_tools::constraints::{
    add_activity_constraints, add_capacity_constraint, add_demand_constraints,
};
use crate::simulation::investment_tools::costs::{activity_cost, annual_fixed_cost};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Flow, MoneyPerActivity, MoneyPerCapacity};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Map storing cost coefficients for each variable type
pub struct CostCoefficientsMap {
    /// Cost per unit of capacity
    pub capacity_cost: MoneyPerCapacity,
    /// Cost per unit of activity in each time slice
    pub activity_costs: IndexMap<TimeSliceID, MoneyPerActivity>,
}

/// Variable map for optimization
pub struct VariableMap {
    /// Capacity variable
    pub capacity_var: Variable,
    /// Activity variables in each time slice
    pub activity_vars: IndexMap<TimeSliceID, Variable>,
}

/// Solution to the optimisation problem
pub struct Solution {
    _solution: highs::Solution,
    _variables: VariableMap,
}

/// Methods for optimisation
pub enum Method {
    /// LCOX method (not yet fully implemented)
    Lcox,
    /// NPV method
    Npv,
}

/// Calculates the cost coefficients for a given method.
fn calculate_cost_coefficients_for_method(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    method: &Method,
) -> CostCoefficientsMap {
    // Capacity variable
    let cost = match method {
        Method::Lcox => annual_fixed_cost(asset),
        Method::Npv => -annual_fixed_cost(asset),
    };
    let capacity_cost = cost;

    // Activity variables
    let mut activity_costs = IndexMap::new();
    for time_slice in time_slice_info.iter_ids() {
        let cost = match method {
            Method::Lcox => activity_cost(asset, reduced_costs, time_slice.clone()),
            Method::Npv => -activity_cost(asset, reduced_costs, time_slice.clone()),
        };
        activity_costs.insert(time_slice.clone(), cost);
    }

    CostCoefficientsMap {
        capacity_cost,
        activity_costs,
    }
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

/// Adds constraints to the problem.
fn add_constraints(
    problem: &mut Problem,
    asset: &AssetRef,
    variables: &VariableMap,
    demand: &HashMap<TimeSliceID, Flow>,
) {
    add_capacity_constraint(problem, asset, variables.capacity_var);
    add_activity_constraints(
        problem,
        asset,
        variables.capacity_var,
        &variables.activity_vars,
    );
    add_demand_constraints(problem, asset, demand, &variables.activity_vars);
}

/// Performs optimisation for a given strategy.
pub fn perform_optimisation(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    method: &Method,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();

    // Calculate cost coefficients
    let cost_coefficients =
        calculate_cost_coefficients_for_method(asset, time_slice_info, reduced_costs, method);

    // Add variables
    let variables = add_variables(&mut problem, &cost_coefficients);

    // Add constraints
    add_constraints(&mut problem, asset, &variables, demand);

    // Solve problem
    let sense = match method {
        Method::Lcox => Sense::Minimise,
        Method::Npv => Sense::Maximise,
    };
    let highs_model = problem.optimise(sense);

    // Solve model
    let solved_model = highs_model.solve();
    match solved_model.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            _solution: solved_model.get_solution(),
            _variables: variables,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    }
}
