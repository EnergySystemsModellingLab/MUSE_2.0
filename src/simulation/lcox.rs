//! Calculation for Levelised Cost of X (LCOX).
//!
//! This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
//! include other flows, we use the term LCOX.
use crate::asset::AssetRef;
use crate::time_slice::TimeSliceID;
use crate::units::{Capacity, Flow, MoneyPerActivity};
use std::collections::HashMap;

/// Output of investment appraisal functions
pub struct AppraisalOutput {
    /// Cost index for this asset/commodity combination
    pub cost_index: MoneyPerActivity,
    /// The required capacity, if a candidate asset
    pub capacity: Option<Capacity>,
    /// Leftover demand
    pub unmet_demand: HashMap<TimeSliceID, Flow>,
}

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
) -> AppraisalOutput {
    // **TODO:** Add LCOX calculation.
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/171
    let cost_index = MoneyPerActivity(42.0);
    let capacity = asset.is_commissioned().then_some(Capacity(43.0));
    let unmet_demand = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    AppraisalOutput {
        cost_index,
        capacity,
        unmet_demand,
    }
}
