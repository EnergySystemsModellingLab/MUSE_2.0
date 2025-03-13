//! Code for updating the simulation state.
use super::optimisation::Solution;
use crate::agent::AssetPool;
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
    /// Calculate commodity prices based on the result of the dispatch optimisation.
    ///
    /// Missing prices will be calculated directly from the input data
    pub fn from_model_and_solution(model: &Model, solution: &Solution, assets: &AssetPool) -> Self {
        let mut prices = CommodityPrices::default();
        let commodities_updated = prices.add_from_solution(solution, assets);

        // Find commodities not updated in last step
        let remaining_commodities = model
            .commodities
            .keys()
            .filter(|id| !commodities_updated.contains(*id));
        prices.add_remaining(remaining_commodities, &model.time_slice_info);

        prices
    }

    /// Add commodity prices for which there are values in the solution
    ///
    /// # Arguments
    ///
    /// * `solution` - The solution to the dispatch optimisation
    ///
    /// # Returns
    ///
    /// The set of commodities for which prices were added.
    fn add_from_solution(&mut self, solution: &Solution, assets: &AssetPool) -> HashSet<Rc<str>> {
        let mut commodities_updated = HashSet::new();

        // Calculate highest capacity dual for each commodity/timeslice
        let mut highest_duals: IndexMap<CommodityPriceKey, f64> = IndexMap::new();
        for (asset_id, time_slice, dual) in solution.iter_capacity_duals() {
            // Get the asset
            let asset = assets.get(asset_id).unwrap();

            // Iterate over process pacs
            let process_pacs = asset.process.iter_pacs();
            for pac in process_pacs {
                // Get the commodity
                let commodity = &pac.commodity;

                // If the commodity flow is positive (produced PAC)
                if pac.flow > 0.0 {
                    let key: CommodityPriceKey = (commodity.id.clone(), time_slice.clone());
                    // Update the highest dual for this commodity/timeslice
                    highest_duals
                        .entry(key)
                        .and_modify(|current_dual| {
                            if dual > *current_dual {
                                *current_dual = dual;
                            }
                        })
                        .or_insert(dual);
                }
            }
        }

        // Insert the highest capacity duals into the prices map
        for ((commodity_id, time_slice), dual) in highest_duals.iter() {
            self.insert(commodity_id, time_slice, *dual);
            commodities_updated.insert(Rc::clone(commodity_id));
        }

        // Sum with the commodity balance duals
        for (commodity_id, time_slice, dual) in solution.iter_commodity_balance_duals() {
            let key = (Rc::clone(commodity_id), time_slice.clone());
            let _combined_dual = self
                .0
                .entry(key)
                .and_modify(|current_dual| {
                    *current_dual += dual;
                })
                .or_insert(dual);
            commodities_updated.insert(Rc::clone(commodity_id));
        }

        commodities_updated
    }

    /// Add prices for any commodity not updated by the dispatch step.
    ///
    /// # Arguments
    ///
    /// * `commodity_ids` - IDs of commodities to update
    /// * `time_slice_info` - Information about time slices
    fn add_remaining<'a, I>(&mut self, commodity_ids: I, time_slice_info: &TimeSliceInfo)
    where
        I: Iterator<Item = &'a Rc<str>>,
    {
        for commodity_id in commodity_ids {
            warn!("No prices calculated for commodity {commodity_id}; setting to NaN");
            for time_slice in time_slice_info.iter_ids() {
                self.insert(commodity_id, time_slice, f64::NAN);
            }
        }
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
