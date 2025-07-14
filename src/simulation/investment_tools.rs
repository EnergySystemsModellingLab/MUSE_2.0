//! Calculation for investment tools such as Levelised Cost of X (LCOX) and Net Present Value (NPV).
use crate::asset::AssetRef;
use crate::finance::{lcox, profitability_index};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Capacity, Dimensionless, Flow, MoneyPerActivity};
use anyhow::Result;
use std::collections::HashMap;

mod coefficients;
mod constraints;
mod costs;
mod optimisation;
use coefficients::{calculate_coefficients_for_lcox, calculate_coefficients_for_npv};
use optimisation::perform_optimisation;

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
) -> Result<(MoneyPerActivity, Capacity, HashMap<TimeSliceID, Flow>)> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_lcox(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation(
        asset,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        true,
    )?;

    // Calculate LCOX
    let annual_fixed_cost = coefficients.capacity_coefficient;
    let activity_costs = coefficients.activity_coefficients;
    let lcox = lcox(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_costs,
    );

    // Placeholder for unmet demand (**TODO.**)
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    Ok((lcox, results.capacity, unmet))
}

/// Calculate NPV based on the specified reduced costs and demand for a particular tranche.
pub fn calculate_npv(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &HashMap<TimeSliceID, Flow>,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<(Dimensionless, Capacity)> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_npv(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation(
        asset,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        false,
    )?;

    // Calculate profitability index
    let annual_fixed_cost = -coefficients.capacity_coefficient;
    let activity_surpluses = coefficients.activity_coefficients;
    let profitability_index = profitability_index(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_surpluses,
    );

    Ok((profitability_index, results.capacity))
}
