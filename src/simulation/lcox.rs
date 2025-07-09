//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceID;
use crate::units::{Capacity, Flow, MoneyPerActivity};
use std::collections::HashMap;

pub mod constraints;
pub mod costs;
pub mod optimisation;
pub mod strategies;

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// # Returns
///
/// Cost index for asset, new capacity (if applicable) and any unmet demand, to be included in the
/// next tranche.
pub fn calculate_lcox(
    asset: &AssetRef,
    _reduced_costs: &ReducedCosts,
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
