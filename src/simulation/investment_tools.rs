//! Calculation for investment tools such as Levelised Cost of X (LCOX) and Net Present Value (NPV).
use crate::asset::AssetRef;
use crate::finance::{lcox, profitability_index};
use crate::simulation::investment_tools::optimisation::{perform_optimisation_for_method, Method};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Capacity, Dimensionless, Flow, MoneyPerActivity};
use std::collections::HashMap;

mod constraints;
mod costs;
mod optimisation;

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
/// include other flows, we use the term LCOX.
///
/// # Returns
///
/// Cost index for asset, new capacity (if applicable) and any unmet demand, to be included in the
/// next tranche.
pub fn calculate_lcox(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> (
    MoneyPerActivity,
    Option<Capacity>,
    HashMap<TimeSliceID, Flow>,
) {
    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation_for_method(
        asset,
        time_slice_info,
        time_slice_level,
        reduced_costs,
        demand,
        &Method::Lcox,
    )
    .unwrap();

    // Extract capacity result for candidate assets
    let new_capacity = (!asset.is_commissioned()).then_some(results.capacity);

    // Calculate LCOX
    let annual_fixed_cost = results.coefficients.capacity_coefficient;
    let activity_costs = results.coefficients.activity_coefficients;
    let lcox = lcox(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_costs,
    );

    // Placeholder for unmet demand (**TODO.**)
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    (lcox, new_capacity, unmet)
}

/// Calculate NPV based on the specified reduced costs and demand for a particular tranche.
pub fn calculate_npv(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> (Dimensionless, Option<Capacity>) {
    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation_for_method(
        asset,
        time_slice_info,
        time_slice_level,
        reduced_costs,
        demand,
        &Method::Npv,
    )
    .unwrap();

    // Extract capacity result for candidate assets
    let new_capacity = (!asset.is_commissioned()).then_some(results.capacity);

    // Calculate profitability index
    let annual_fixed_cost = -results.coefficients.capacity_coefficient;
    let activity_surpluses = results.coefficients.activity_coefficients;
    let profitability_index = profitability_index(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_surpluses,
    );

    (profitability_index, new_capacity)
}
