use crate::asset::AssetRef;
use crate::simulation::lcox::optimisation::Variable;
use crate::time_slice::TimeSliceID;
use highs::RowProblem as Problem;
use indexmap::IndexMap;

/// NOTE: Copied from `add_activity_constraints` in `optimisation/constraints.rs`.
pub fn add_activity_constraints_for_existing(
    problem: &mut Problem,
    asset_activity_vars: &IndexMap<(AssetRef, TimeSliceID), Variable>,
) {
    for ((asset, time_slice), var) in asset_activity_vars.iter() {
        let limits = asset.get_activity_limits(time_slice);
        let limits = limits.start().value()..=limits.end().value();
        problem.add_row(limits, [(*var, 1.0)]);
    }
}

pub fn add_activity_constraints_for_candidates(
    problem: &mut Problem,
    candidate_capacity_vars: &IndexMap<AssetRef, Variable>,
    candidate_activity_vars: &IndexMap<(AssetRef, TimeSliceID), Variable>,
) {
    for ((asset, time_slice), activity_var) in candidate_activity_vars.iter() {
        let capacity_var = candidate_capacity_vars.get(asset).unwrap();
        let limits = asset.get_activity_per_capacity_limits(time_slice);
        let lower_limit = limits.start().value();
        let upper_limit = limits.end().value();

        // Upper bound: activity ≤ capacity * upper_limit
        problem.add_row(
            ..=0.0,
            [(*activity_var, 1.0), (*capacity_var, -upper_limit)],
        );

        // Lower bound: activity ≥ capacity * lower_limit
        problem.add_row(
            ..=0.0,
            [(*activity_var, -1.0), (*capacity_var, lower_limit)],
        );
    }
}

pub fn add_demand_constraints(
    problem: &mut Problem,
    asset_activity_vars: &IndexMap<(AssetRef, TimeSliceID), Variable>,
    candidate_activity_vars: &IndexMap<(AssetRef, TimeSliceID), Variable>,
) {
}

pub fn add_capacity_constraints_for_candidates(
    problem: &mut Problem,
    candidate_capacity_vars: &IndexMap<AssetRef, Variable>,
) {
}
