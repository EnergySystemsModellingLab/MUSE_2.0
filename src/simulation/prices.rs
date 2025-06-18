//! Code for updating the simulation state.
use super::optimisation::Solution;
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use log::warn;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A map relating commodity ID + region + time slice to current price (endogenous)
#[derive(Default)]
pub struct CommodityPrices(BTreeMap<(CommodityID, RegionID, TimeSliceID), f64>);

impl CommodityPrices {
    /// Calculate commodity prices based on the result of the dispatch optimisation
    pub fn from_model_and_solution(model: &Model, solution: &Solution) -> Self {
        let mut prices = CommodityPrices::default();
        let commodity_regions_updated = prices.add_from_solution(solution);

        // Find commodity/region combinations not updated in last step
        let mut remaining_commodity_regions = HashSet::new();
        for commodity_id in model.commodities.keys() {
            for region_id in model.regions.keys() {
                let key = (commodity_id.clone(), region_id.clone());
                if !commodity_regions_updated.contains(&key) {
                    remaining_commodity_regions.insert(key);
                }
            }
        }

        prices.add_remaining(remaining_commodity_regions.iter(), &model.time_slice_info);

        prices
    }

    /// Add commodity prices for which there are values in the solution
    ///
    /// Commodity prices are calculated as the sum of the commodity balance duals and the highest
    /// activity dual for each commodity/timeslice.
    ///
    /// # Arguments
    ///
    /// * `solution` - The solution to the dispatch optimisation
    ///
    /// # Returns
    ///
    /// The set of commodity/region pairs for which prices were added.
    fn add_from_solution(&mut self, solution: &Solution) -> HashSet<(CommodityID, RegionID)> {
        let mut commodity_regions_updated = HashSet::new();

        // Calculate highest activity dual for each commodity/region/timeslice
        let mut highest_duals = HashMap::new();
        for (asset, time_slice, dual) in solution.iter_activity_duals() {
            // Iterate over all output flows
            for flow in asset.iter_flows().filter(|flow| flow.coeff > 0.0) {
                // Update the highest dual for this commodity/timeslice
                highest_duals
                    .entry((
                        flow.commodity.id.clone(),
                        asset.region_id.clone(),
                        time_slice.clone(),
                    ))
                    .and_modify(|current_dual| {
                        if dual > *current_dual {
                            *current_dual = dual;
                        }
                    })
                    .or_insert(dual);
            }
        }

        // Add the highest capacity dual for each commodity/region/timeslice to each commodity
        // balance dual
        for (commodity_id, region_id, time_slice, dual) in solution.iter_commodity_balance_duals() {
            let key = (commodity_id.clone(), region_id.clone(), time_slice.clone());
            let price = dual + highest_duals.get(&key).unwrap_or(&0.0);
            self.insert(commodity_id, region_id, time_slice, price);
            commodity_regions_updated.insert((commodity_id.clone(), region_id.clone()));
        }

        commodity_regions_updated
    }

    /// Add prices for any commodity not updated by the dispatch step.
    ///
    /// # Arguments
    ///
    /// * `commodity_regions` - Commodity/region pairs to update
    /// * `time_slice_info` - Information about time slices
    fn add_remaining<'a, I>(&mut self, commodity_regions: I, time_slice_info: &TimeSliceInfo)
    where
        I: Iterator<Item = &'a (CommodityID, RegionID)>,
    {
        for (commodity_id, region_id) in commodity_regions {
            warn!("No prices calculated for commodity {commodity_id} in region {region_id}; setting to NaN");
            for time_slice in time_slice_info.iter_ids() {
                self.insert(commodity_id, region_id, time_slice, f64::NAN);
            }
        }
    }

    /// Insert a price for the given commodity, region and time slice
    pub fn insert(
        &mut self,
        commodity_id: &CommodityID,
        region_id: &RegionID,
        time_slice: &TimeSliceID,
        price: f64,
    ) {
        let key = (commodity_id.clone(), region_id.clone(), time_slice.clone());
        self.0.insert(key, price);
    }

    /// Iterate over the map.
    ///
    /// # Returns
    ///
    /// An iterator of tuples containing commodity ID, region ID, time slice and price.
    pub fn iter(&self) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, f64)> {
        self.0
            .iter()
            .map(|((commodity_id, region_id, ts), price)| (commodity_id, region_id, ts, *price))
    }
}
