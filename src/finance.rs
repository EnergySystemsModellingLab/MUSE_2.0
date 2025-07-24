//! General functions related to finance.
use crate::time_slice::TimeSliceID;
use crate::units::{Activity, Capacity, Dimensionless, Money, MoneyPerActivity, MoneyPerCapacity};
use indexmap::IndexMap;
use std::collections::HashMap;

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

/// Calculates the annual capital cost for a process per unit of capacity
pub fn annual_capital_cost(
    capital_cost: MoneyPerCapacity,
    lifetime: u32,
    discount_rate: Dimensionless,
) -> MoneyPerCapacity {
    let crf = capital_recovery_factor(lifetime, discount_rate);
    let total_capital_cost = capital_cost * crf;
    total_capital_cost * crf
}

/// Calculates an annual profitability index based on capacity and activity.
pub fn profitability_index(
    capacity: Capacity,
    annual_fixed_cost: MoneyPerCapacity,
    activity: &HashMap<TimeSliceID, Activity>,
    activity_surpluses: &IndexMap<TimeSliceID, MoneyPerActivity>,
) -> Dimensionless {
    // Calculate the annualised fixed costs
    let annualised_fixed_cost = annual_fixed_cost * capacity;

    // Calculate the total annualised surplus
    let mut total_annualised_surplus = Money(0.0);
    for (time_slice, activity) in activity.iter() {
        let activity_surplus = *activity_surpluses.get(time_slice).unwrap();
        total_annualised_surplus += activity_surplus * *activity;
    }

    total_annualised_surplus / annualised_fixed_cost
}

/// Calculates annual LCOX based on capacity and activity.
pub fn lcox(
    capacity: Capacity,
    annual_fixed_cost: MoneyPerCapacity,
    activity: &HashMap<TimeSliceID, Activity>,
    activity_costs: &IndexMap<TimeSliceID, MoneyPerActivity>,
) -> MoneyPerActivity {
    // Calculate the annualised fixed costs
    let annualised_fixed_cost = annual_fixed_cost * capacity;

    // Calculate the total activity costs
    let mut total_activity_costs = Money(0.0);
    let mut total_activity = Activity(0.0);
    for (time_slice, activity) in activity.iter() {
        let activity_cost = *activity_costs.get(time_slice).unwrap();
        total_activity += *activity;
        total_activity_costs += activity_cost * *activity;
    }

    (annualised_fixed_cost + total_activity_costs) / total_activity
}
