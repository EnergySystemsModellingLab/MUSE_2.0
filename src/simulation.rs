//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use crate::time_slice::TimeSliceID;
use log::info;
use std::collections::HashMap;
use std::rc::Rc;

pub mod optimisation;
use optimisation::perform_dispatch_optimisation;
pub mod investment;
use investment::perform_agent_investment;
pub mod update;
use update::{update_commodity_flows, update_commodity_prices};

/// A combination of commodity ID and time slice
type CommodityPriceKey = (Rc<str>, TimeSliceID);

/// A map relating commodity ID + time slice to current price (endogenous)
#[derive(Default)]
pub struct CommodityPrices(HashMap<CommodityPriceKey, f64>);

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

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: Model, mut assets: AssetPool) {
    let mut prices = CommodityPrices::default();

    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Commission and decommission assets for this milestone year
        assets.decomission_old(year);
        assets.commission_new(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, year);
        update_commodity_flows(&solution, &mut assets);
        update_commodity_prices(&model.commodities, &solution, &mut prices);

        // Agent investment
        perform_agent_investment(&model, &mut assets);
    }
}
