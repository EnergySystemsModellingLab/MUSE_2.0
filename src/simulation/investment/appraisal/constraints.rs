//! Constraints for the optimisation problem.
use super::optimisation::Variable;
use super::DemandMap;
use crate::asset::AssetRef;
use crate::commodity::Commodity;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceSelection};
use crate::units::Capacity;
use highs::RowProblem as Problem;
use indexmap::IndexMap;

/// Adds a capacity constraint to the problem.
///
/// The behaviour depends on whether the asset is commissioned or a candidate:
/// - For a commissioned asset, the capacity is fixed.
/// - For a candidate asset, the capacity is variable between zero and an upper bound.
pub fn add_capacity_constraint(
    problem: &mut Problem,
    asset: &AssetRef,
    max_capacity: Option<Capacity>,
    capacity_var: Variable,
) {
    let capacity = max_capacity.unwrap_or(asset.capacity);
    let bounds = if asset.is_commissioned() {
        capacity.value()..=capacity.value()
    } else {
        0.0..=capacity.value()
    };
    problem.add_row(bounds, [(capacity_var, 1.0)]);
}

/// Adds activity constraints to the problem.
///
/// Constrains the activity variables to be within the asset's activity limits.
///
/// The behaviour depends on whether the asset is commissioned or a candidate:
/// - For an commissioned asset, the activity limits have fixed bounds based on the asset's (fixed)
///   capacity.
/// - For a candidate asset, the activity limits depend on the capacity of the asset, which is
///   itself variable. The constraints are therefore applied to both the capacity and activity
///   variables. We need separate constraints for the upper and lower bounds.
pub fn add_activity_constraints(
    problem: &mut Problem,
    asset: &AssetRef,
    capacity_var: Variable,
    activity_vars: &IndexMap<TimeSliceID, Variable>,
) {
    if asset.is_commissioned() {
        add_activity_constraints_for_existing(problem, asset, activity_vars)
    } else {
        add_activity_constraints_for_candidate(problem, asset, capacity_var, activity_vars)
    }
}

fn add_activity_constraints_for_existing(
    problem: &mut Problem,
    asset: &AssetRef,
    activity_vars: &IndexMap<TimeSliceID, Variable>,
) {
    for (time_slice, var) in activity_vars.iter() {
        let limits = asset.get_activity_limits(time_slice);
        let limits = limits.start().value()..=limits.end().value();
        problem.add_row(limits, [(*var, 1.0)]);
    }
}

fn add_activity_constraints_for_candidate(
    problem: &mut Problem,
    asset: &AssetRef,
    capacity_var: Variable,
    activity_vars: &IndexMap<TimeSliceID, Variable>,
) {
    for (time_slice, activity_var) in activity_vars.iter() {
        let limits = asset.get_activity_per_capacity_limits(time_slice);
        let lower_limit = limits.start().value();
        let upper_limit = limits.end().value();

        // Upper bound: activity ≤ capacity * upper_limit
        problem.add_row(..=0.0, [(*activity_var, 1.0), (capacity_var, -upper_limit)]);

        // Lower bound: activity ≥ capacity * lower_limit
        problem.add_row(..=0.0, [(*activity_var, -1.0), (capacity_var, lower_limit)]);
    }
}

/// Adds demand constraints to the problem.
///
/// Constrains supply to be less than or equal to demand, which adapts based on the commodity's
/// balance level.
pub fn add_demand_constraints(
    problem: &mut Problem,
    asset: &AssetRef,
    commodity: &Commodity,
    time_slice_info: &TimeSliceInfo,
    demand: &DemandMap,
    activity_vars: &IndexMap<TimeSliceID, Variable>,
    unmet_demand_vars: &IndexMap<TimeSliceSelection, Variable>,
) {
    let flow_coeff = asset.get_flow(&commodity.id).unwrap().coeff;
    for (ts_selection, demand_for_ts_selection) in demand {
        let mut terms = Vec::new();
        for (time_slice, _) in ts_selection.iter(time_slice_info) {
            terms.push((*activity_vars.get(time_slice).unwrap(), flow_coeff.value()));
        }
        terms.push((*unmet_demand_vars.get(ts_selection).unwrap(), 1.0));
        problem.add_row(
            demand_for_ts_selection.value()..=demand_for_ts_selection.value(),
            terms,
        );
    }
}
