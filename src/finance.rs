//! General functions related to finance.
use crate::time_slice::TimeSliceID;
use crate::units::{Activity, Capacity, Dimensionless, Money, MoneyPerActivity, MoneyPerCapacity};
use indexmap::IndexMap;

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

pub fn profitability_index(
    capacity: Capacity,
    annual_fixed_cost: MoneyPerCapacity,
    activity_map: &IndexMap<TimeSliceID, Activity>,
    activity_costs: &IndexMap<TimeSliceID, MoneyPerActivity>,
) -> Dimensionless {
    // Calculate the annualised capital cost
    let annualised_capital_cost = annual_fixed_cost * capacity;

    // Loop through the time slices
    let mut total_annualised_surplus = Money(0.0);
    for (time_slice, activity) in activity_map.iter() {
        let activity_cost = *activity_costs.get(time_slice).unwrap();
        total_annualised_surplus += activity_cost * *activity;
    }

    annualised_capital_cost / total_annualised_surplus
}
pub fn lcox(
    capacity: Capacity,
    annual_fixed_cost: MoneyPerCapacity,
    activity_map: &IndexMap<TimeSliceID, Activity>,
    activity_costs: &IndexMap<TimeSliceID, MoneyPerActivity>,
) -> MoneyPerActivity {
    // Calculate the annualised capital cost
    let annualised_capital_cost = annual_fixed_cost * capacity;

    // Loop through the time slices
    let mut total_annualised_surplus = Money(0.0);
    let mut total_activity = Activity(0.0);
    for (time_slice, activity) in activity_map.iter() {
        let activity_cost = *activity_costs.get(time_slice).unwrap();
        total_activity += *activity;
        total_annualised_surplus += activity_cost * *activity;
    }

    (annualised_capital_cost + total_annualised_surplus) / total_activity
}
