//! Costs for LCOX optimisation.
use crate::asset::AssetRef;
use crate::process::Process;
use crate::region::RegionID;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceID;
use crate::units::{Dimensionless, MoneyPerActivity, MoneyPerCapacity, Year};

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
pub fn annual_fixed_cost_for_process(
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
