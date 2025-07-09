use crate::asset::{AssetPool, AssetRef};
use crate::simulation::investment_tools::constraints::{
    add_activity_constraints_for_candidates, add_activity_constraints_for_existing,
    add_capacity_constraints_for_candidates, add_demand_constraints,
};
use crate::simulation::investment_tools::costs::{
    activity_cost_for_candidate, activity_cost_for_existinng, annual_fixed_cost_for_candidate,
    annual_fixed_cost_for_existing,
};
use crate::simulation::investment_tools::optimisation::{
    add_candidate_activity_variable, add_candidate_capacity_variable,
    add_existing_activity_variable, add_existing_capacity_variable, VariableMap,
};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceInfo;
use highs::{RowProblem as Problem, Sense};
use itertools::iproduct;

/// Trait defining the interface for optimization strategies
pub trait Strategy {
    /// Add variables to the optimization problem
    fn add_variables(
        &self,
        problem: &mut Problem,
        variables: &mut VariableMap,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    );

    /// Add constraints to the optimization problem
    fn add_constraints(&self, problem: &mut Problem, variables: &VariableMap);

    /// The optimization sense (minimize or maximize)
    fn sense(&self) -> Sense;
}

/// LCOX (Levelized Cost of X) optimization strategy
pub struct LcoxStrategy;

impl Strategy for LcoxStrategy {
    fn add_variables(
        &self,
        problem: &mut Problem,
        variables: &mut VariableMap,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) {
        let existing_assets = asset_pool.as_slice();

        // Add capacity variables for existing assets
        for asset in existing_assets {
            let col_factor = annual_fixed_cost_for_existing(asset);
            add_existing_capacity_variable(problem, variables, asset.clone(), col_factor.value());
        }

        // Add activity variables for existing assets
        for (asset, time_slice) in iproduct!(existing_assets.iter(), time_slice_info.iter_ids()) {
            let col_factor = activity_cost_for_existinng(asset, reduced_costs, time_slice.clone());
            add_existing_activity_variable(
                problem,
                variables,
                asset.clone(),
                time_slice.clone(),
                col_factor.value(),
            );
        }

        // Add capacity variables for candidate assets
        for asset in candidate_assets {
            let col_factor = annual_fixed_cost_for_candidate(asset);
            add_candidate_capacity_variable(problem, variables, asset.clone(), col_factor.value());
        }

        // Add activity variables for candidate assets
        for (asset, time_slice) in iproduct!(candidate_assets.iter(), time_slice_info.iter_ids()) {
            let col_factor = activity_cost_for_candidate(asset, reduced_costs, time_slice.clone());
            add_candidate_activity_variable(
                problem,
                variables,
                asset.clone(),
                time_slice.clone(),
                col_factor.value(),
            );
        }

        // TODO: Add unnmet demand variables
    }

    fn add_constraints(&self, problem: &mut Problem, variables: &VariableMap) {
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

    fn sense(&self) -> Sense {
        Sense::Minimise
    }
}

/// NPV (Net Present Value) optimization strategy
pub struct NpvStrategy;

impl Strategy for NpvStrategy {
    fn add_variables(
        &self,
        problem: &mut Problem,
        variables: &mut VariableMap,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) {
        let existing_assets = asset_pool.as_slice();

        // Add capacity variables for existing assets
        for asset in existing_assets {
            let col_factor = -annual_fixed_cost_for_existing(asset);
            add_existing_capacity_variable(problem, variables, asset.clone(), col_factor.value());
        }

        // Add activity variables for existing assets
        for (asset, time_slice) in iproduct!(existing_assets.iter(), time_slice_info.iter_ids()) {
            let col_factor = -activity_cost_for_existinng(asset, reduced_costs, time_slice.clone());
            add_existing_activity_variable(
                problem,
                variables,
                asset.clone(),
                time_slice.clone(),
                col_factor.value(),
            );
        }

        // Add capacity variables for candidate assets
        for asset in candidate_assets {
            let col_factor = -annual_fixed_cost_for_candidate(asset);
            add_candidate_capacity_variable(problem, variables, asset.clone(), col_factor.value());
        }

        // Add activity variables for candidate assets
        for (asset, time_slice) in iproduct!(candidate_assets.iter(), time_slice_info.iter_ids()) {
            let col_factor = -activity_cost_for_candidate(asset, reduced_costs, time_slice.clone());
            add_candidate_activity_variable(
                problem,
                variables,
                asset.clone(),
                time_slice.clone(),
                col_factor.value(),
            );
        }
    }

    fn add_constraints(&self, problem: &mut Problem, variables: &VariableMap) {
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

    fn sense(&self) -> Sense {
        Sense::Maximise
    }
}
