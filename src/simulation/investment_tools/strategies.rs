use crate::asset::{AssetPool, AssetRef};
use crate::simulation::investment_tools::constraints::{
    add_activity_constraints_for_candidates, add_activity_constraints_for_existing,
    add_capacity_constraints_for_candidates, add_demand_constraints,
};
use crate::simulation::investment_tools::costs::{
    activity_cost_for_candidate, activity_cost_for_existing, annual_fixed_cost_for_candidate,
    annual_fixed_cost_for_existing,
};
use crate::simulation::investment_tools::optimisation::{
    add_candidate_activity_variable, add_candidate_capacity_variable,
    add_existing_activity_variable, add_existing_capacity_variable, CostCoefficientsMap,
    VariableMap,
};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceInfo;
use highs::{RowProblem as Problem, Sense};
use itertools::iproduct;

/// Trait defining the interface for optimization strategies
pub trait Strategy {
    /// Calculate cost coefficients for the strategy
    fn calculate_cost_coefficients(
        &self,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap;

    /// Add constraints to the optimization problem
    fn add_constraints(&self, problem: &mut Problem, variables: &VariableMap);

    /// The optimization sense (minimize or maximize)
    fn sense(&self) -> Sense;
}

/// LCOX (Levelized Cost of X) optimization strategy
pub struct LcoxStrategy;

impl Strategy for LcoxStrategy {
    fn calculate_cost_coefficients(
        &self,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap {
        calculate_cost_coefficients_for_method(
            asset_pool,
            candidate_assets,
            time_slice_info,
            reduced_costs,
            Method::Lcox,
        )
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
    fn calculate_cost_coefficients(
        &self,
        asset_pool: &AssetPool,
        candidate_assets: &[AssetRef],
        time_slice_info: &TimeSliceInfo,
        reduced_costs: &ReducedCosts,
    ) -> CostCoefficientsMap {
        calculate_cost_coefficients_for_method(
            asset_pool,
            candidate_assets,
            time_slice_info,
            reduced_costs,
            Method::Npv,
        )
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

pub enum Method {
    Lcox,
    Npv,
}

fn calculate_cost_coefficients_for_method(
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    method: Method,
) -> CostCoefficientsMap {
    let mut cost_coefficients = CostCoefficientsMap::default();
    let existing_assets = asset_pool.as_slice();

    // Add capacity variables for existing assets
    for asset in existing_assets {
        // let cost = annual_fixed_cost_for_existing(asset);
        let cost = match method {
            Method::Lcox => annual_fixed_cost_for_existing(asset),
            Method::Npv => -annual_fixed_cost_for_existing(asset),
        };
        cost_coefficients
            .existing_capacity_costs
            .insert(asset.clone(), cost);
    }

    // Add activity variables for existing assets
    for (asset, time_slice) in iproduct!(existing_assets.iter(), time_slice_info.iter_ids()) {
        let cost = match method {
            Method::Lcox => activity_cost_for_existing(asset, reduced_costs, time_slice.clone()),
            Method::Npv => -activity_cost_for_existing(asset, reduced_costs, time_slice.clone()),
        };
        cost_coefficients
            .existing_activity_costs
            .insert((asset.clone(), time_slice.clone()), cost);
    }

    // Add capacity variables for candidate assets
    for asset in candidate_assets {
        let cost = match method {
            Method::Lcox => annual_fixed_cost_for_candidate(asset),
            Method::Npv => -annual_fixed_cost_for_candidate(asset),
        };
        cost_coefficients
            .candidate_capacity_costs
            .insert(asset.clone(), cost);
    }

    // Add activity variables for candidate assets
    for (asset, time_slice) in iproduct!(candidate_assets.iter(), time_slice_info.iter_ids()) {
        let cost = match method {
            Method::Lcox => activity_cost_for_candidate(asset, reduced_costs, time_slice.clone()),
            Method::Npv => -activity_cost_for_candidate(asset, reduced_costs, time_slice.clone()),
        };
        cost_coefficients
            .candidate_activity_costs
            .insert((asset.clone(), time_slice.clone()), cost);
    }

    cost_coefficients
}
