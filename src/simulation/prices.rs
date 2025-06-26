//! Code for updating the simulation state.
use super::optimisation::Solution;
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::MoneyPerFlow;
use itertools::iproduct;
use std::collections::{BTreeMap, HashMap};

/// A map relating commodity ID + region + time slice to current price (endogenous)
#[derive(Default)]
pub struct CommodityPrices(BTreeMap<(CommodityID, RegionID, TimeSliceID), MoneyPerFlow>);

impl CommodityPrices {
    /// Calculate commodity prices based on the result of the dispatch optimisation and input data.
    ///
    /// Note that it is possible that there will still be commodities without specified prices, if
    /// they were not included in the dispatch optimisation and no levies were specified by the
    /// user.
    pub fn calculate(model: &Model, solution: &Solution, year: u32) -> Self {
        let mut prices = CommodityPrices::default();
        prices.add_from_duals(solution);
        prices.add_from_levies(model, year);

        prices
    }

    /// Add commodity prices using activity and commodity balance duals.
    ///
    /// Commodity prices are calculated as the sum of the commodity balance duals and the highest
    /// activity dual for each commodity/timeslice.
    ///
    /// # Arguments
    ///
    /// * `solution` - The solution to the dispatch optimisation
    fn add_from_duals(&mut self, solution: &Solution) {
        // Calculate highest activity dual for each commodity/region/timeslice
        let mut highest_duals = HashMap::new();
        for (asset, time_slice, dual) in solution.iter_activity_duals() {
            // Iterate over all output flows
            for flow in asset.iter_flows().filter(|flow| flow.is_output()) {
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

        // Add the highest activity dual for each commodity/region/timeslice to each commodity
        // balance dual
        for (commodity_id, region_id, time_slice, mut price) in
            solution.iter_commodity_balance_duals()
        {
            let key = (commodity_id.clone(), region_id.clone(), time_slice.clone());
            if let Some(highest) = highest_duals.get(&key) {
                // **HACK:** The units are mismatched here. We probably need to multiply by a flow
                // coeff.
                //
                // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/663
                price += MoneyPerFlow(highest.value());
            }
            self.0.insert(key, price);
        }
    }

    /// Add prices based on levies/incentives.
    ///
    /// If a commodity already has a price based on the previous dual-based calculation, we choose
    /// the higher of the two.
    ///
    /// # Arguments
    ///
    /// * `model` - The model
    /// * `year` - The milestone year of interest
    fn add_from_levies(&mut self, model: &Model, year: u32) {
        for (region_id, time_slice) in
            iproduct!(model.iter_regions(), model.time_slice_info.iter_ids())
        {
            let levy_key = (region_id.clone(), year, time_slice.clone());
            for commodity in model.commodities.values() {
                if let Some(levy) = commodity.levies.get(&levy_key) {
                    let key = (commodity.id.clone(), region_id.clone(), time_slice.clone());
                    self.0
                        .entry(key)
                        .and_modify(|price| *price = price.max(levy.value))
                        .or_insert(levy.value);
                }
            }
        }
    }

    /// Insert a price for the given commodity, region and time slice
    pub fn insert(
        &mut self,
        commodity_id: &CommodityID,
        region_id: &RegionID,
        time_slice: &TimeSliceID,
        price: MoneyPerFlow,
    ) {
        let key = (commodity_id.clone(), region_id.clone(), time_slice.clone());
        self.0.insert(key, price);
    }

    /// Iterate over the map.
    ///
    /// # Returns
    ///
    /// An iterator of tuples containing commodity ID, region ID, time slice and price.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, MoneyPerFlow)> {
        self.0
            .iter()
            .map(|((commodity_id, region_id, ts), price)| (commodity_id, region_id, ts, *price))
    }

    /// Get the price for the specified commodity for a given region and time slice
    pub fn get(
        &self,
        commodity_id: &CommodityID,
        region_id: &RegionID,
        time_slice: &TimeSliceID,
    ) -> Option<MoneyPerFlow> {
        self.0
            .get(&(commodity_id.clone(), region_id.clone(), time_slice.clone()))
            .copied()
    }
}
