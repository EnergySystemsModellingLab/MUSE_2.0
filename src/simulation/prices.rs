//! Code for updating the simulation state.
use crate::asset::AssetRef;
use crate::commodity::CommodityID;
use crate::model::Model;
use crate::region::RegionID;
use crate::time_slice::TimeSliceID;
use crate::units::{MoneyPerActivity, MoneyPerFlow};
use itertools::iproduct;
use std::collections::{BTreeMap, HashMap};

/// A map relating commodity ID + region + time slice to current price (endogenous)
#[derive(Default)]
pub struct CommodityPrices(BTreeMap<(CommodityID, RegionID, TimeSliceID), MoneyPerFlow>);

impl CommodityPrices {
    /// Add prices based on levies/incentives.
    ///
    /// If a commodity already has a price based on the previous dual-based calculation, we choose
    /// the higher of the two.
    ///
    /// # Arguments
    ///
    /// * `model` - The model
    /// * `year` - The milestone year of interest
    pub fn with_levies(mut self, model: &Model, year: u32) -> Self {
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

        self
    }

    /// Remove the impact of scarcity on prices.
    ///
    /// # Arguments
    ///
    /// * `activity_duals` - Value of activity duals from solution
    pub fn without_scarcity_pricing<'a, I>(mut self, activity_duals: I) -> Self
    where
        I: Iterator<Item = (&'a AssetRef, &'a TimeSliceID, MoneyPerActivity)>,
    {
        let highest_duals = get_highest_activity_duals(activity_duals);

        // Add the highest activity dual for each commodity/region/timeslice to each commodity
        // balance dual
        for (key, highest) in highest_duals.iter() {
            if let Some(price) = self.0.get_mut(key) {
                // **HACK:** The units are mismatched here. We probably need to multiply by a flow
                // coeff.
                //
                // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/663
                *price += MoneyPerFlow(highest.value());
            }
        }

        self
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

impl<'a> FromIterator<(&'a CommodityID, &'a RegionID, &'a TimeSliceID, MoneyPerFlow)>
    for CommodityPrices
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (&'a CommodityID, &'a RegionID, &'a TimeSliceID, MoneyPerFlow)>,
    {
        let map = iter
            .into_iter()
            .map(|(commodity_id, region_id, time_slice, price)| {
                (
                    (commodity_id.clone(), region_id.clone(), time_slice.clone()),
                    price,
                )
            })
            .collect();
        CommodityPrices(map)
    }
}

fn get_highest_activity_duals<'a, I>(
    activity_duals: I,
) -> HashMap<(CommodityID, RegionID, TimeSliceID), MoneyPerActivity>
where
    I: Iterator<Item = (&'a AssetRef, &'a TimeSliceID, MoneyPerActivity)>,
{
    // Calculate highest activity dual for each commodity/region/timeslice
    let mut highest_duals = HashMap::new();
    for (asset, time_slice, dual) in activity_duals {
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

    highest_duals
}
