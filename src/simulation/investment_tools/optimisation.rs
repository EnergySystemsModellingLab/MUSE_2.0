use crate::asset::{AssetPool, AssetRef};
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::simulation::investment_tools::strategies::Strategy;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{
    Activity, Capacity, Dimensionless, Flow, Money, MoneyPerActivity, MoneyPerCapacity,
    MoneyPerFlow, UnitType,
};
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
    pub unmet_demand_costs: IndexMap<(CommodityID, RegionID, TimeSliceID), MoneyPerFlow>,
}

/// Variable map for optimization
#[derive(Default)]
pub struct VariableMap {
    pub existing_capacity_vars: IndexMap<AssetRef, Variable>,
    pub candidate_capacity_vars: IndexMap<AssetRef, Variable>,
    pub existing_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    pub candidate_activity_vars: IndexMap<(AssetRef, TimeSliceID), Variable>,
    pub unmet_demand_vars: IndexMap<(CommodityID, RegionID, TimeSliceID), Variable>,
    /// Maps variable to its index in the solution array
    pub variable_to_index: IndexMap<Variable, usize>,
    /// Next variable index to assign
    next_index: usize,
}

impl VariableMap {
    fn add_variable(&mut self, var: Variable) {
        self.variable_to_index.insert(var, self.next_index);
        self.next_index += 1;
    }
}

#[derive(Default)]
pub struct ResultsMap {
    pub candidate_capacity_results: IndexMap<AssetRef, Capacity>,
    pub existing_activity_results: IndexMap<(AssetRef, TimeSliceID), Activity>,
    pub candidate_activity_results: IndexMap<(AssetRef, TimeSliceID), Activity>,
    pub unmet_demand_results: IndexMap<(CommodityID, RegionID, TimeSliceID), Flow>,
}

/// Solution to the optimisation problem
pub struct Solution {
    solution: highs::Solution,
    variables: VariableMap,
}

impl Solution {
    pub fn get_solution_value<T>(&self, var: &Variable) -> T
    where
        T: UnitType,
    {
        let index = self.variables.variable_to_index[var];
        T::new(self.solution.columns()[index])
    }

    pub fn create_results_map(&self) -> ResultsMap {
        let mut results_map = ResultsMap::default();

        // Insert candidate capacity results
        for (asset_ref, var) in self.variables.candidate_capacity_vars.iter() {
            results_map
                .candidate_capacity_results
                .insert(asset_ref.clone(), self.get_solution_value::<Capacity>(var));
        }

        // Insert existing activity results
        for ((asset_ref, time_slice), var) in self.variables.existing_activity_vars.iter() {
            results_map.existing_activity_results.insert(
                (asset_ref.clone(), time_slice.clone()),
                self.get_solution_value::<Activity>(var),
            );
        }

        // Insert candidate activity results
        for ((asset_ref, time_slice), var) in self.variables.candidate_activity_vars.iter() {
            results_map.candidate_activity_results.insert(
                (asset_ref.clone(), time_slice.clone()),
                self.get_solution_value::<Activity>(var),
            );
        }

        // Insert unmet demand results
        for ((commodity, region, time_slice), var) in self.variables.unmet_demand_vars.iter() {
            results_map.unmet_demand_results.insert(
                (commodity.clone(), region.clone(), time_slice.clone()),
                self.get_solution_value::<Flow>(var),
            );
        }
        results_map
    }
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
    variables.add_variable(var);
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
    variables.add_variable(var);
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
    variables.add_variable(var);
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
    variables.add_variable(var);
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
    variables.add_variable(var);
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
            cost.value(),
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
) -> Result<ResultsMap> {
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
    let solved_model = highs_model.solve();
    let solution = match solved_model.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            solution: solved_model.get_solution(),
            variables,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    };

    // Assemble results
    let results_map = solution.unwrap().create_results_map();
    Ok(results_map)
}

pub fn calculate_profitability_index_for_candidate(
    results: &ResultsMap,
    cost_coefficients: &CostCoefficientsMap,
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
) -> Dimensionless {
    // Get the capacity result for the asset
    let capacity = *results.candidate_capacity_results.get(asset).unwrap();

    // Get the cost coefficients for the asset
    let cost_coefficient = *cost_coefficients
        .existing_capacity_costs
        .get(asset)
        .unwrap();

    // Calculate the annualised capital cost
    let annualised_capital_cost = cost_coefficient * capacity;

    // Loop through the time slices
    let mut total_annualised_surplus = Money(0.0);
    for time_slice in time_slice_info.iter_ids() {
        // Get the activity result for the asset
        let activity = *results
            .candidate_activity_results
            .get(&(asset.clone(), time_slice.clone()))
            .unwrap();
        let cost_coefficient = *cost_coefficients
            .candidate_activity_costs
            .get(&(asset.clone(), time_slice.clone()))
            .unwrap();
        total_annualised_surplus += cost_coefficient * activity;
    }

    annualised_capital_cost / total_annualised_surplus
}
