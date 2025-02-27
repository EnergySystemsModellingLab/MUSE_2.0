//! Code for updating the simulation state.
use super::optimisation::Solution;
use crate::model::Model;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use indexmap::IndexMap;
use log::warn;
use std::collections::HashSet;
use std::rc::Rc;

/// A combination of commodity ID and time slice
type CommodityPriceKey = (Rc<str>, TimeSliceID);

/// A map relating commodity ID + time slice to current price (endogenous)
#[derive(Default)]
pub struct CommodityPrices(IndexMap<CommodityPriceKey, f64>);

impl CommodityPrices {
    /// Get the price for the given commodity and time slice
    pub fn get(&self, commodity_id: &Rc<str>, time_slice: &TimeSliceID) -> f64 {
        let key = (Rc::clone(commodity_id), time_slice.clone());
        *self
            .0
            .get(&key)
            .expect("Missing price for given commodity and time slice")
    }

    /// Insert a price for the given commodity and time slice
    pub fn insert(&mut self, commodity_id: &Rc<str>, time_slice: &TimeSliceID, price: f64) {
        let key = (Rc::clone(commodity_id), time_slice.clone());
        self.0.insert(key, price);
    }

    /// Iterate over the map.
    ///
    /// # Returns
    ///
    /// An iterator of tuples containing commodity ID, time slice and price.
    pub fn iter(&self) -> impl Iterator<Item = (&Rc<str>, &TimeSliceID, f64)> {
        self.0
            .iter()
            .map(|((commodity_id, ts), price)| (commodity_id, ts, *price))
    }
}

/// Update commodity prices for assets based on the result of the dispatch optimisation.
pub fn update_commodity_prices(model: &Model, solution: &Solution, prices: &mut CommodityPrices) {
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
    for commodity_id in commodity_ids {
        warn!("No prices calculated for commodity {commodity_id}; setting to NaN");
        for time_slice in time_slice_info.iter_ids() {
            prices.insert(commodity_id, time_slice, f64::NAN);
        }
    }
}
