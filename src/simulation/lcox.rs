//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::finance::{lcox, profitability_index};
use crate::simulation::investment_tools::optimisation::{perform_optimisation_for_method, Method};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use crate::units::{Capacity, Dimensionless, Flow, MoneyPerActivity};
use std::collections::HashMap;

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// # Returns
///
/// Cost index for asset, new capacity (if applicable) and any unmet demand, to be included in the
/// next tranche.
pub fn calculate_lcox(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
) -> (
    MoneyPerActivity,
    Option<Capacity>,
    HashMap<TimeSliceID, Flow>,
) {
    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation_for_method(
        asset,
        time_slice_info,
        reduced_costs,
        demand,
        &Method::Lcox,
    )
    .unwrap();

    // Extract capacity result for candidate assets
    let new_capacity = (!asset.is_commissioned()).then_some(results.capacity);

    // Calculate LCOX
    let lcox = lcox(
        results.capacity,
        results.cost_coefficients.capacity_cost,
        &results.activity,
        &results.cost_coefficients.activity_costs,
    );

    // Calculate unmet demand (TODO)
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    (lcox, new_capacity, unmet)
}

/// Calculate NPV based on the specified reduced costs and demand for a particular tranche.
pub fn calculate_npv(
    asset: &AssetRef,
    time_slice_info: &TimeSliceInfo,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
) -> (Dimensionless, Option<Capacity>, HashMap<TimeSliceID, Flow>) {
    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation_for_method(
        asset,
        time_slice_info,
        reduced_costs,
        demand,
        &Method::Npv,
    )
    .unwrap();

    // Extract capacity result for candidate assets
    let new_capacity = (!asset.is_commissioned()).then_some(results.capacity);

    // Calculate profitability index
    let profitability_index = profitability_index(
        results.capacity,
        results.cost_coefficients.capacity_cost,
        &results.activity,
        &results.cost_coefficients.activity_costs,
    );

    // Calculate unnmet demand (TODO)
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    (profitability_index, new_capacity, unmet)
}
