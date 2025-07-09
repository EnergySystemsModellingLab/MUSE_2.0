//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::process::Process;
use crate::region::RegionID;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceID;
use crate::units::{Capacity, Dimensionless, Flow, MoneyPerActivity, MoneyPerCapacity, Year};
use std::collections::HashMap;

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// # Returns
///
/// Cost index for asset, new capacity (if applicable) and any unmet demand, to be included in the
/// next tranche.
pub fn calculate_lcox(
    asset: &AssetRef,
    _reduced_costs: &HashMap<(AssetRef, TimeSliceID), MoneyPerActivity>,
    demand: &HashMap<TimeSliceID, Flow>,
) -> (
    MoneyPerActivity,
    Option<Capacity>,
    HashMap<TimeSliceID, Flow>,
) {
    // **TODO:** Add LCOX calculation.
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/171
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    let new_capacity = asset.is_commissioned().then_some(Capacity(43.0));
    (MoneyPerActivity(42.0), new_capacity, unmet)
}

/// Calculates the capital recovery factor (CRF) for a given lifetime and discount rate.
///
/// The CRF is used to annualize capital costs over the lifetime of an asset.
pub fn capital_recovery_factor(lifetime: u32, discount_rate: Dimensionless) -> Dimensionless {
    if lifetime == 0 {
        return Dimensionless(0.0);
    }
    if discount_rate == Dimensionless(0.0) {
        return Dimensionless(1.0) / Dimensionless(lifetime as f64);
    }
    let factor = (Dimensionless(1.0) + discount_rate).powi(lifetime as i32);
    (discount_rate * factor) / (factor - Dimensionless(1.0))
}

/// Calculates the annual capital cost for a technology per unit of capacity
pub fn annual_capital_cost(
    capital_cost: MoneyPerCapacity,
    lifetime: u32,
    discount_rate: Dimensionless,
) -> MoneyPerCapacity {
    let crf = capital_recovery_factor(lifetime, discount_rate);
    let total_capital_cost = capital_cost * crf;
    total_capital_cost * crf
}

/// Calculates the annual capital cost per unit of capacity for a process.
pub fn annual_capital_cost_for_process(
    process: &Process,
    region: RegionID,
    year: u32,
) -> MoneyPerCapacity {
    let process_parameter = process.parameters.get(&(region, year)).unwrap();
    let capital_cost = process_parameter.capital_cost;
    let lifetime = process_parameter.lifetime;
    let discount_rate = process_parameter.discount_rate;
    annual_capital_cost(capital_cost, lifetime, discount_rate)
}

/// Calculates the annual fixed costs per unit of capacity for an asset.
pub fn annual_fixed_cost_for_asset(asset: &AssetRef) -> MoneyPerCapacity {
    let fixed_operating_cost = asset.process_parameter.fixed_operating_cost;
    fixed_operating_cost * Year(1.0)
}

/// Calculates the annual fixed costs per unit of capacity for a process.
pub fn annnual_fixed_cost_for_process(
    process: &Process,
    region: RegionID,
    year: u32,
) -> MoneyPerCapacity {
    let process_parameter = process.parameters.get(&(region.clone(), year)).unwrap();
    let fixed_operating_cost = process_parameter.fixed_operating_cost;
    let annual_fixed_operating_cost = fixed_operating_cost * Year(1.0);
    let capital_costs = annual_capital_cost_for_process(process, region, year);
    annual_fixed_operating_cost + capital_costs
}

/// Calculates the cost per unit of activity for an asset.
pub fn activity_cost_for_asset(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) -> MoneyPerActivity {
    *reduced_costs.get(&(asset.clone(), time_slice)).unwrap()
}

/// Calculates the cost per unit of activity for a process (TODO).
pub fn activity_cost_for_process(
    _process: &Process,
    _region: RegionID,
    _reduced_costs: &ReducedCosts,
    _time_slice: TimeSliceID,
) -> MoneyPerActivity {
    MoneyPerActivity(1.0)
}
