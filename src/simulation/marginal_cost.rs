//! Code for calculating marginal cost.
use super::CommodityPrices;
use crate::asset::Asset;
use crate::commodity::CommodityID;
use crate::process::Process;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::sync::{Mutex, OnceLock};

/// Calculate marginal cost of a particular commodity for a given process.
///
/// **PLACEHOLDER**: Currently just returns random number between 1.0 and 50.0 inclusive.
///
/// See: <https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/516>
pub fn marginal_cost(
    commodity_of_interest: &CommodityID,
    process: &Process,
    region_id: &RegionID,
    commission_year: u32,
    _current_year: u32,
    _time_slice: &TimeSliceID,
    _prices: &CommodityPrices,
) -> f64 {
    let flows = process
        .flows
        .get(&(region_id.clone(), commission_year))
        .unwrap();

    assert!(
        flows.contains_key(commodity_of_interest),
        "Commodity '{}' is not an output flow for process '{}'",
        commodity_of_interest,
        process.id
    );

    static RNG: OnceLock<Mutex<SmallRng>> = OnceLock::new();
    let mut rng = RNG
        .get_or_init(|| Mutex::new(SmallRng::seed_from_u64(42)))
        .lock()
        .unwrap();
    rng.random_range(1.0..=50.0)
}

/// Calculate marginal cost for an asset.
pub fn marginal_cost_for_asset(
    asset: &Asset,
    commodity_of_interest: &CommodityID,
    current_year: u32,
    time_slice: &TimeSliceID,
    prices: &CommodityPrices,
) -> f64 {
    marginal_cost(
        commodity_of_interest,
        &asset.process,
        &asset.region_id,
        asset.commission_year,
        current_year,
        time_slice,
        prices,
    )
}
