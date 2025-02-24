//! Code for updating the simulation state.
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::model::Model;
use crate::time_slice::TimeSliceInfo;
use log::{info, warn};
use std::collections::HashSet;
use std::rc::Rc;

/// Update commodity prices for assets based on the result of the dispatch optimisation.
pub fn update_commodity_prices(model: &Model, solution: &Solution, prices: &mut CommodityPrices) {
    info!("Updating commodity prices...");
    let commodities_updated = update_commodity_prices_from_solution(solution, prices);

    // Find commodities not updated in last step
    let remaining_commodities = model
        .commodities
        .keys()
        .filter(|id| !commodities_updated.contains(*id));
    update_remaining_commodity_prices(remaining_commodities, &model.time_slice_info, prices);
}

/// Update the commodity prices for which there are values in the solution
fn update_commodity_prices_from_solution(
    solution: &Solution,
    prices: &mut CommodityPrices,
) -> HashSet<Rc<str>> {
    info!("Updating commodity prices...");

    let mut commodities_updated = HashSet::new();

    for (commodity_id, time_slice, price) in solution.iter_commodity_prices() {
        prices.insert(commodity_id, time_slice, price);
        commodities_updated.insert(Rc::clone(commodity_id));
    }

    commodities_updated
}

/// Update prices for any commodity not updated by the dispatch step.
///
/// **TODO**: This will likely take additional arguments, depending on how we decide to do this step
///
/// # Arguments
///
/// * `commodity_ids` - IDs of commodities to update
/// * `prices` - Commodity prices
fn update_remaining_commodity_prices<'a, I>(
    commodity_ids: I,
    time_slice_info: &TimeSliceInfo,
    prices: &mut CommodityPrices,
) where
    I: Iterator<Item = &'a Rc<str>>,
{
    info!("Updating remaining commodity prices...");

    for commodity_id in commodity_ids {
        warn!("No prices calculated for commodity {commodity_id}; setting to NaN");
        for time_slice in time_slice_info.iter_ids() {
            prices.insert(commodity_id, time_slice, f64::NAN);
        }
    }
}
