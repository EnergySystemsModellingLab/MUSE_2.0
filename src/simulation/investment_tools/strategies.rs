use crate::asset::AssetRef;
use crate::simulation::investment_tools::constraints::{
    add_activity_constraints, add_capacity_constraint, add_demand_constraints,
};
use crate::simulation::investment_tools::costs::{activity_cost, annual_fixed_cost};
use crate::simulation::investment_tools::optimisation::{CostCoefficientsMap, VariableMap};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceID;
use crate::time_slice::TimeSliceInfo;
use crate::units::Flow;
use highs::{RowProblem as Problem, Sense};
use indexmap::IndexMap;
use std::collections::HashMap;

/// Trait defining the interface for optimization strategies
pub trait Strategy {
    /// The optimization sense (minimize or maximize)
    fn sense(&self) -> Sense;

    /// Calculate cost coefficients for the strategy
    fn calculate_cost_coefficients(
        &self,
        asset: &AssetRef,
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap;

    /// Add constraints to the optimization problem
    fn add_constraints(
        &self,
        problem: &mut Problem,
        asset: &AssetRef,
        variables: &VariableMap,
        demand: &HashMap<TimeSliceID, Flow>,
    );
}

/// LCOX (Levelized Cost of X) optimization strategy
pub struct LcoxStrategy;

impl Strategy for LcoxStrategy {
    fn sense(&self) -> Sense {
        Sense::Minimise
    }

    fn calculate_cost_coefficients(
        &self,
        asset: &AssetRef,
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap {
        calculate_cost_coefficients_for_method(asset, time_slice_info, reduced_costs, Method::Lcox)
    }

    fn add_constraints(
        &self,
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
}

/// NPV (Net Present Value) optimization strategy
pub struct NpvStrategy;

impl Strategy for NpvStrategy {
    fn sense(&self) -> Sense {
        Sense::Maximise
    }

    fn calculate_cost_coefficients(
        &self,
        asset: &AssetRef,
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap {
        calculate_cost_coefficients_for_method(asset, time_slice_info, reduced_costs, Method::Npv)
    }

    fn add_constraints(
        &self,
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
}

pub enum Method {
    Lcox,
    Npv,
}

fn calculate_cost_coefficients_for_method(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    method: Method,
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
