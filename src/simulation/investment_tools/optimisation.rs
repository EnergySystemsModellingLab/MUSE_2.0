use crate::asset::{AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::simulation::investment_tools::strategies::Strategy;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{MoneyPerActivity, MoneyPerCapacity};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem};
use indexmap::IndexMap;

/// A decision variable in the optimisation
pub type Variable = highs::Col;

/// Map storing cost coefficients for each variable type
#[derive(Default)]
pub struct CostCoefficientsMap {
    pub existing_capacity_costs: IndexMap<AssetRef, MoneyPerCapacity>,
    pub candidate_capacity_costs: IndexMap<AssetRef, MoneyPerCapacity>,
    pub existing_activity_costs: IndexMap<(AssetRef, TimeSliceID), MoneyPerActivity>,
    pub candidate_activity_costs: IndexMap<(AssetRef, TimeSliceID), MoneyPerActivity>,
    pub unmet_demand_costs: IndexMap<(CommodityID, RegionID, TimeSliceID), f64>,
}

/// Variable map for optimization
#[derive(Default)]
pub struct VariableMap {
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
    variables
        .candidate_activity_vars
        .insert((asset_ref, time_slice), var);
}

/// Add a unmet demand variable for a commodity in a region in a time slice
pub fn add_unmet_demand_variable(
    problem: &mut Problem,
    variables: &mut VariableMap,
    commodity: CommodityID,
    region: RegionID,
    time_slice: TimeSliceID,
    col_factor: f64,
) {
    let var = problem.add_column(col_factor, 0.0..);
    variables
        .unmet_demand_vars
        .insert((commodity, region, time_slice), var);
}

/// Add variables to the problem based onn cost coefficients
pub fn add_variables(
    problem: &mut Problem,
    variables: &mut VariableMap,
    cost_coefficients: &CostCoefficientsMap,
) {
    // Add capacity variables for existing assets
    for (asset, cost) in cost_coefficients.existing_capacity_costs.iter() {
        add_existing_capacity_variable(problem, variables, asset.clone(), cost.value());
    }

    // Add activity variables for existing assets
    for ((asset, time_slice), cost) in cost_coefficients.existing_activity_costs.iter() {
        add_existing_activity_variable(
            problem,
            variables,
            asset.clone(),
            time_slice.clone(),
            cost.value(),
        );
    }

    // Add capacity variables for candidate assets
    for (asset, cost) in cost_coefficients.candidate_capacity_costs.iter() {
        add_candidate_capacity_variable(problem, variables, asset.clone(), cost.value());
    }

    // Add activity variables for candidate assets
    for ((asset, time_slice), cost) in cost_coefficients.candidate_activity_costs.iter() {
        add_candidate_activity_variable(
            problem,
            variables,
            asset.clone(),
            time_slice.clone(),
            cost.value(),
        );
    }

    // Add unmet demand costs (only for LCOX)
    for ((commodity, region, time_slice), cost) in cost_coefficients.unmet_demand_costs.iter() {
        add_unmet_demand_variable(
            problem,
            variables,
            commodity.clone(),
            region.clone(),
            time_slice.clone(),
            *cost,
        );
    }
}

/// Perform optimisation for a given strategy
pub fn perform_optimisation(
    model: &Model,
    asset_pool: &AssetPool,
    candidate_assets: &[AssetRef],
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    strategy: &dyn Strategy,
) -> Result<Solution> {
    // Set up problem
    let mut problem = Problem::default();
    let mut variables = VariableMap::default();

    // Calculate cost coefficients
    let cost_coefficients = strategy.calculate_cost_coefficients(
        model,
        asset_pool,
        candidate_assets,
        time_slice_info,
        reduced_costs,
    );

    // Add variables
    add_variables(&mut problem, &mut variables, &cost_coefficients);

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
