//! Code for adding constraints to the dispatch optimisation problem.
use super::VariableMap;
use crate::asset::{AssetID, AssetPool};
use crate::commodity::{CommodityID, CommodityType};
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceSelection};
use highs::RowProblem as Problem;

/// Corresponding variables for a constraint along with the row offset in the solution
pub struct KeysWithOffset<T> {
    offset: usize,
    keys: Vec<T>,
}

impl<T> KeysWithOffset<T> {
    /// Zip the keys with the corresponding dual values in the solution, accounting for the offset
    pub fn zip_duals<'a>(&'a self, duals: &'a [f64]) -> impl Iterator<Item = (&'a T, f64)> {
        assert!(
            self.offset + self.keys.len() <= duals.len(),
            "Bad constraint keys: dual rows out of range"
        );

        self.keys.iter().zip(duals[self.offset..].iter().copied())
    }
}

/// Indicates the commodity ID and time slice selection covered by each commodity balance constraint
pub type CommodityBalanceKeys = KeysWithOffset<(CommodityID, RegionID, TimeSliceSelection)>;

/// Indicates the asset ID and time slice covered by each capacity constraint
pub type CapacityKeys = KeysWithOffset<(AssetID, TimeSliceID)>;

/// The keys for different constraints
pub struct ConstraintKeys {
    /// Keys for commodity balance constraints
    pub commodity_balance_keys: CommodityBalanceKeys,
    /// Keys for capacity constraints
    pub capacity_keys: CapacityKeys,
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

    let capacity_keys =
        add_asset_capacity_constraints(problem, variables, assets, &model.time_slice_info);

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
) -> CommodityBalanceKeys {
    // Row offset in problem. This line **must** come before we add more constraints.
    let offset = problem.num_rows();

    let mut terms = Vec::new();
    let mut keys = Vec::new();
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

    CommodityBalanceKeys { offset, keys }
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
) -> CapacityKeys {
    // Row offset in problem. This line **must** come before we add more constraints.
    let offset = problem.num_rows();

    let mut terms = Vec::new();
    let mut keys = Vec::new();
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

    CapacityKeys { offset, keys }
}
