//! Code for adding constraints to the dispatch optimisation problem.
use crate::asset::{AssetID, AssetPool};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceSelection};
use highs::RowProblem as Problem;
use std::rc::Rc;

use super::VariableMap;

/// Indicates the commodity ID and time slice selection covered by each commodity balance constraint
pub type CommodityBalanceConstraintKeys = Vec<(CommodityID, RegionID, TimeSliceSelection)>;

/// Indicates the asset ID and time slice covered by each capacity constraint
pub type CapacityConstraintKeys = Vec<(AssetID, TimeSliceID)>;

/// The keys for different constraints
pub struct ConstraintKeys {
    /// Keys for commodity balance constraints
    pub commodity_balance_keys: CommodityBalanceConstraintKeys,
    /// Keys for capacity constraints
    pub capacity_keys: CapacityConstraintKeys,
}

/// Add asset-level constraints
///
/// Note: the ordering of constraints is important, as the dual values of the constraints must later
/// be retrieved to calculate commodity prices.
///
/// # Arguments:
///
/// * `problem` - The optimisation problem
/// * `variables` - The variables in the problem
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns:
///
/// * A vector of keys for commodity balance constraints
/// * A vector of keys for capacity constraints
pub fn add_asset_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) -> ConstraintKeys {
    let commodity_balance_keys =
        add_commodity_balance_constraints(problem, variables, model, assets, year);

    let capacity_keys = add_asset_capacity_constraints(
        problem,
        variables,
        assets,
        &model.time_slice_info,
        &commodity_balance_keys,
    );

    // **TODO**: Currently it's safe to assume all process flows are non-flexible, as we enforce
    // this when reading data in. Once we've added support for flexible process flows, we will
    // need to add different constraints for assets with flexible and non-flexible flows.
    //
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/360
    add_fixed_asset_constraints(problem, variables, assets, &model.time_slice_info);

    // Return constraint keys
    ConstraintKeys {
        commodity_balance_keys,
        capacity_keys,
    }
}

/// Add asset-level input-output commodity balances.
///
/// These constraints fix the supply-demand balance for the whole system.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#commodity-balance-constraints
fn add_commodity_balance_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) -> CommodityBalanceConstraintKeys {
    // Sanity check: we rely on the first n values of the dual row values corresponding to the
    // commodity constraints, so these must be the first rows
    assert!(
        problem.num_rows() == 0,
        "Commodity balance constraints must be added before other constraints"
    );

    let mut terms = Vec::new();
    let mut keys = CommodityBalanceConstraintKeys::new();
    for commodity in model.commodities.values() {
        if commodity.kind != CommodityType::SupplyEqualsDemand
            && commodity.kind != CommodityType::ServiceDemand
        {
            continue;
        }

        for region_id in model.iter_regions() {
            for ts_selection in model
                .time_slice_info
                .iter_selections_for_level(commodity.time_slice_level)
            {
                // Note about performance: this loop **may** prove to be a bottleneck as
                // `time_slice_info.iter_selection` returns a `Box` and so requires a heap
                // allocation each time. For commodities with a `TimeSliceLevel` of `TimeSlice` (the
                // worst case), this means the number of additional heap allocations will equal the
                // number of time slices, which for this function could be in the
                // hundreds/thousands.
                for (time_slice, _) in model.time_slice_info.iter_selection(&ts_selection) {
                    // Add terms for this asset + commodity at this time slice. The coefficient for
                    // each variable is one.
                    terms.extend(
                        assets
                            .iter_for_region_and_commodity(region_id, &commodity.id)
                            .map(|asset| (variables.get(asset.id, &commodity.id, time_slice), 1.0)),
                    );
                }

                // Get the RHS of the equation for a commodity balance constraint. For SED
                // commodities, the RHS will be zero and for SVD commodities it will be equal to the
                // demand for the given time slice selection.
                let rhs = match commodity.kind {
                    CommodityType::SupplyEqualsDemand => 0.0,
                    CommodityType::ServiceDemand => {
                        match ts_selection {
                            TimeSliceSelection::Single(ref ts) => *commodity
                                .demand
                                .get(&(region_id.clone(), year, ts.clone()))
                                .unwrap(),
                            // We currently only support specifying demand at the time slice level:
                            //  https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/391
                            _ => panic!(
                            "Currently SVD commodities must have a time slice level of time slice"
                        ),
                        }
                    }
                    _ => unreachable!(),
                };

                // Add constraint (sum of terms must equal rhs)
                problem.add_row(rhs..=rhs, terms.drain(0..));

                // Keep track of the order in which constraints were added
                keys.push((commodity.id.clone(), region_id.clone(), ts_selection));
            }
        }
    }

    keys
}

/// Add asset-level capacity and availability constraints.
///
/// For every asset at every time slice, the sum of the commodity flows for PACs must not exceed the
/// capacity limits, which are a product of the annual capacity, time slice length and process
/// availability.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#asset-level-capacity-and-availability-constraints
fn add_asset_capacity_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    assets: &AssetPool,
    time_slice_info: &TimeSliceInfo,
    commodity_balance_keys: &CommodityBalanceConstraintKeys,
) -> CapacityConstraintKeys {
    // Sanity check: we rely on the dual rows corresponding to the capacity constraints being
    // immediately after the commodity balance constraints in `problem`.
    assert!(
        problem.num_rows() == commodity_balance_keys.len(),
        "Capacity constraints must be added immediately after commodity constraints."
    );

    let mut terms = Vec::new();
    let mut keys = CapacityConstraintKeys::new();
    for asset in assets.iter() {
        for time_slice in time_slice_info.iter_ids() {
            let mut is_input = false; // NB: there will be at least one PAC
            for flow in asset.iter_pacs() {
                is_input = flow.flow < 0.0; // NB: PACs will be all inputs or all outputs

                let var = variables.get(asset.id, &flow.commodity.id, time_slice);
                terms.push((var, 1.0));
            }

            let mut limits = asset.get_energy_limits(time_slice);

            // If it's an input flow, the q's will be negative, so we need to invert the limits
            if is_input {
                limits = -limits.end()..=-limits.start();
            }

            problem.add_row(limits, terms.drain(0..));

            // Keep track of the order in which constraints were added
            keys.push((asset.id, time_slice.clone()));
        }
    }
    keys
}

/// Add constraints for non-flexible assets.
///
/// Non-flexible assets are those which have a fixed ratio between inputs and outputs.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#non-flexible-assets
fn add_fixed_asset_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    assets: &AssetPool,
    time_slice_info: &TimeSliceInfo,
) {
    for asset in assets.iter() {
        // Get first PAC. unwrap is safe because all processes have at least one PAC.
        let pac1 = asset.iter_pacs().next().unwrap();

        for time_slice in time_slice_info.iter_ids() {
            let pac_var = variables.get(asset.id, &pac1.commodity.id, time_slice);
            let pac_term = (pac_var, -1.0 / pac1.flow);
            for flow in asset.iter_flows() {
                // Don't add a constraint for the PAC itself
                if Rc::ptr_eq(&flow.commodity, &pac1.commodity) {
                    continue;
                }

                // We are enforcing that (var / flow) - (pac_var / pac_flow) = 0
                let var = variables.get(asset.id, &flow.commodity.id, time_slice);
                problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
            }
        }
    }
}
