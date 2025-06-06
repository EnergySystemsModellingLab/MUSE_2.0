//! Code for adding constraints to the dispatch optimisation problem.
use super::VariableMap;
use crate::asset::{AssetPool, AssetRef};
use crate::commodity::CommodityID;
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
        add_asset_capacity_constraints(problem, variables, &model.time_slice_info, assets);

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
    _variables: &VariableMap,
    _model: &Model,
    _assets: &AssetPool,
    _year: u32,
) -> CommodityBalanceKeys {
    // Row offset in problem. This line **must** come before we add more constraints.
    let offset = problem.num_rows();

    let keys = Vec::new();

    // **TODO:** Add commodity balance constraints:
    //  https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/577

    CommodityBalanceKeys { offset, keys }
}

/// Add asset-level capacity and availability constraints.
///
/// This ensures that assets do not exceed their specified capacity and availability for each time
/// slice.
fn add_asset_capacity_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    time_slice_info: &TimeSliceInfo,
    assets: &AssetPool,
) -> CapacityKeys {
    // Row offset in problem. This line **must** come before we add more constraints.
    let offset = problem.num_rows();

    let mut keys = Vec::new();
    for asset in assets.iter() {
        for time_slice in time_slice_info.iter_ids() {
            let var = variables.get(asset, time_slice);
            let limits = asset.get_activity_limits(time_slice);

            problem.add_row(limits, [(var, 1.0)]);
            keys.push((asset.clone(), time_slice.clone()))
        }
    }

    CapacityKeys { offset, keys }
}
