#![allow(missing_docs)]

use crate::units::*;

pub fn capital_recovery_factor(lifetime: IYear, discount_rate: Dimensionless) -> Dimensionless {
    if lifetime == IYear(0) {
        return Dimensionless(0.0);
    }
    if discount_rate == Dimensionless(0.0) {
        return Dimensionless(1.0 / lifetime.0 as f64);
    }
    let factor = (Dimensionless(1.0) + discount_rate).pow(lifetime);
    (discount_rate * factor) / (factor - Dimensionless(1.0))
}

pub fn annual_capital_cost(
    capital_cost: MoneyPerCapacity,
    capacity: Capacity,
    lifetime: IYear,
    discount_rate: Dimensionless,
) -> MoneyPerYear {
    let crf = capital_recovery_factor(lifetime, discount_rate);
    let total_capital_cost = capital_cost * capacity * crf;
    let annual_capital_cost = total_capital_cost * crf;
    annual_capital_cost * PerYear(1.0) // this is an annualized quantity, so we return it as such
}

pub fn annual_fixed_operating_cost(
    fixed_operating_cost: MoneyPerYearPerCapacity,
    capacity: Capacity,
) -> MoneyPerYear {
    fixed_operating_cost * capacity
}

pub fn annual_fixed_costs(
    capital_cost: MoneyPerCapacity,
    capacity: Capacity,
    lifetime: IYear,
    discount_rate: Dimensionless,
    fixed_operating_cost: MoneyPerYearPerCapacity,
) -> MoneyPerYear {
    let annual_capital_cost = annual_capital_cost(capital_cost, capacity, lifetime, discount_rate);
    let annual_fixed_operating_cost = annual_fixed_operating_cost(fixed_operating_cost, capacity);
    annual_capital_cost + annual_fixed_operating_cost
}
