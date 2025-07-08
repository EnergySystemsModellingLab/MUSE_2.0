//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::process::Process;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::{Capacity, Dimensionless, Flow, Money, MoneyPerActivity, MoneyPerCapacity};
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

/// Calculates the annual capital cost for a technology
pub fn annual_capital_cost(
    capital_cost: MoneyPerCapacity,
    capacity: Capacity,
    lifetime: u32,
    discount_rate: Dimensionless,
) -> Money {
    let crf = capital_recovery_factor(lifetime, discount_rate);
    let total_capital_cost = capital_cost * capacity * crf;
    total_capital_cost * crf
}

/// Calculates the annual capital cost for a process.
pub fn annual_capital_cost_for_process(
    process: &Process,
    capacity: Capacity,
    region: RegionID,
    year: u32,
) -> Money {
    let process_parameter = process.parameters.get(&(region, year)).unwrap();
    let capital_cost = process_parameter.capital_cost;
    let lifetime = process_parameter.lifetime;
    let discount_rate = process_parameter.discount_rate;
    annual_capital_cost(capital_cost, capacity, lifetime, discount_rate)
}
