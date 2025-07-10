//! Costs for the optimisation problem.
use crate::asset::AssetRef;
use crate::finance::annual_capital_cost;
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::TimeSliceID;
use crate::units::{MoneyPerActivity, MoneyPerCapacity, Year};

/// Calculates the annual fixed costs per unit of capacity for an asset.
pub fn annual_fixed_cost(asset: &AssetRef) -> MoneyPerCapacity {
    match asset.is_commissioned() {
        true => annual_fixed_cost_for_existing(asset),
        false => annual_fixed_cost_for_candidate(asset),
    }
}

/// Calculates the annual fixed costs per unit of capacity for an existing asset.
fn annual_fixed_cost_for_existing(asset: &AssetRef) -> MoneyPerCapacity {
    let fixed_operating_cost = asset.process_parameter.fixed_operating_cost;
    fixed_operating_cost * Year(1.0)
}

/// Calculates the annual capital cost per unit of capacity for a candidate asset.
fn annual_capital_cost_for_candidate(asset: &AssetRef) -> MoneyPerCapacity {
    let capital_cost = asset.process_parameter.capital_cost;
    let lifetime = asset.process_parameter.lifetime;
    let discount_rate = asset.process_parameter.discount_rate;
    annual_capital_cost(capital_cost, lifetime, discount_rate)
}

/// Calculates the annual fixed costs per unit of capacity for a candidate asset.
fn annual_fixed_cost_for_candidate(asset: &AssetRef) -> MoneyPerCapacity {
    let fixed_operating_cost = asset.process_parameter.fixed_operating_cost;
    let annual_fixed_operating_cost = fixed_operating_cost * Year(1.0);
    let capital_costs = annual_capital_cost_for_candidate(asset);
    annual_fixed_operating_cost + capital_costs
}

/// Calculates the cost per unit of activity for an asset.
pub fn activity_cost(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) -> MoneyPerActivity {
    match asset.is_commissioned() {
        true => activity_cost_for_existing(asset, reduced_costs, time_slice),
        false => activity_cost_for_candidate(asset, reduced_costs, time_slice),
    }
}

/// Calculates the cost per unit of activity for an existing asset.
fn activity_cost_for_existing(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) -> MoneyPerActivity {
    *reduced_costs.get(&(asset.clone(), time_slice)).unwrap()
}

/// Calculates the cost per unit of activity for a candidate asset.
fn activity_cost_for_candidate(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    time_slice: TimeSliceID,
) -> MoneyPerActivity {
    *reduced_costs.get(&(asset.clone(), time_slice)).unwrap()
}
