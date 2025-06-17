//! Code for adding constraints to the dispatch optimisation problem.
use super::VariableMap;
use crate::asset::{AssetPool, AssetRef};
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
pub type CapacityKeys = KeysWithOffset<(AssetRef, TimeSliceID)>;

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

    let mut keys = Vec::new();
    let mut terms = Vec::new();
    for (commodity_id, commodity) in model.commodities.iter() {
        if !matches!(
            commodity.kind,
            CommodityType::SupplyEqualsDemand | CommodityType::ServiceDemand
        ) {
            continue;
        }

        for region_id in model.iter_regions() {
            for ts_selection in model
                .time_slice_info
                .iter_selections_at_level(commodity.time_slice_level)
            {
                for (asset, flow) in assets.iter_for_region_and_commodity(region_id, commodity_id) {
                    // If the commodity has a time slice level of season/annual, the constraint will
                    // cover multiple time slices
                    for (time_slice, _) in ts_selection.iter(&model.time_slice_info) {
                        let var = variables.get(asset, time_slice);
                        terms.push((var, flow.coeff));
                    }
                }

                // Add constraint. For SED commodities, the RHS is zero and for SVD commodities it
                // is the exogenous demand supplied by the user.
                let rhs = if commodity.kind == CommodityType::ServiceDemand {
                    *commodity
                        .demand
                        .get(&(region_id.clone(), year, ts_selection.clone()))
                        .unwrap()
                } else {
                    0.0
                };
                problem.add_row(rhs..=rhs, terms.drain(..));
                keys.push((
                    commodity_id.clone(),
                    region_id.clone(),
                    ts_selection.clone(),
                ))
            }
        }
    }

    CommodityBalanceKeys { offset, keys }
}

/// Add asset-level capacity and availability constraints.
///
/// For every asset at every time slice, the sum of the commodity flows for assets must not exceed
/// the capacity limits, which are a product of the annual capacity, time slice length and process
/// availability.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#asset-level-capacity-and-availability-constraints
fn add_asset_capacity_constraints(
    problem: &mut Problem,
    _variables: &VariableMap,
    _assets: &AssetPool,
    _time_slice_info: &TimeSliceInfo,
) -> CapacityKeys {
    // Row offset in problem. This line **must** come before we add more constraints.
    let offset = problem.num_rows();

    let keys = Vec::new();

    // **TODO:** Add capacity/availability constraints:
    //  https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/579

    CapacityKeys { offset, keys }
}
