//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::time_slice::TimeSliceID;
use crate::units::{Flow, MoneyPerActivity};
use std::collections::HashMap;

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// # Returns
///
/// Cost index for commodity and any unmet demand, to be included in the next tranche.
pub fn calculate_lcox(
    _reduced_costs: &HashMap<(AssetRef, TimeSliceID), MoneyPerActivity>,
    demand: &HashMap<TimeSliceID, Flow>,
) -> (MoneyPerActivity, HashMap<TimeSliceID, Flow>) {
    // **TODO:** Add LCOX calculation.
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/171
    let unmet = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();
    (MoneyPerActivity(42.0), unmet)
}
