#![allow(missing_docs)]

use crate::units::*;

/// Calculates the capital recovery factor (CRF) for a given lifetime and discount rate.
///
/// The CRF is used to annualize capital costs over the lifetime of an asset.
pub fn capital_recovery_factor(lifetime: IYear, discount_rate: Dimensionless) -> Dimensionless {
    if lifetime == IYear(0) {
        return Dimensionless(0.0);
    }
    if discount_rate == Dimensionless(0.0) {
        return Dimensionless(1.0 / lifetime.0 as f64);
    }
    let factor = (Dimensionless(1.0) + discount_rate).powi(lifetime.0 as i32);
    (discount_rate * factor) / (factor - Dimensionless(1.0))
}

/// Calculates the annual capital cost for a technology
pub fn annual_capital_cost(
    capital_cost: MoneyPerCapacity,
    capacity: Capacity,
    lifetime: IYear,
    discount_rate: Dimensionless,
) -> MoneyPerYear {
    let crf = capital_recovery_factor(lifetime, discount_rate);
    let total_capital_cost = capital_cost * capacity * crf;
    let annual_capital_cost = total_capital_cost * crf;
    MoneyPerYear(annual_capital_cost.0) // this is an annualized quantity, so we return it as such
}

/// Calculates the annual fixed operating cost for a technology
pub fn annual_fixed_operating_cost(
    fixed_operating_cost: MoneyPerCapacityPerYear,
    capacity: Capacity,
) -> MoneyPerYear {
    fixed_operating_cost * capacity
}

/// Calculates the annual fixed costs for a technology
///
/// This is the sum of the annual capital cost and the annual fixed operating cost.
pub fn annual_fixed_costs(
    capital_cost: MoneyPerCapacity,
    capacity: Capacity,
    lifetime: IYear,
    discount_rate: Dimensionless,
    fixed_operating_cost: MoneyPerCapacityPerYear,
) -> MoneyPerYear {
    let annual_capital_cost = annual_capital_cost(capital_cost, capacity, lifetime, discount_rate);
    let annual_fixed_operating_cost = annual_fixed_operating_cost(fixed_operating_cost, capacity);
    annual_capital_cost + annual_fixed_operating_cost
}

/// Calculates the annual variable cost for a technology
pub fn annual_variable_cost(
    variable_operating_cost: MoneyPerActivity,
    capacity: Capacity,
    cap2act: ActivityPerCapacity,
    utilization: Dimensionless,
) -> MoneyPerYear {
    let capacity_a = capacity * cap2act;
    let annual_variable_cost = variable_operating_cost * capacity_a * utilization;
    MoneyPerYear(annual_variable_cost.0) // this is an annualized quantity, so we return it as such
}
