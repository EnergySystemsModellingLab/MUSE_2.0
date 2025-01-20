//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use log::info;
use std::collections::HashMap;
use std::rc::Rc;

pub mod dispatch;
use dispatch::perform_dispatch;
pub mod update;
use update::{update_commodity_flows, update_commodity_prices};

/// A map relating commodity ID to current price (endogenous)
pub type CommodityPrices = HashMap<Rc<str>, f64>;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: Model, mut assets: AssetPool) {
    // Commodity prices (endogenous)
    let mut prices = CommodityPrices::new();

    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Dispatch optimisation
        let solution = perform_dispatch(&model, &assets, year);
        update_commodity_flows(&solution, &mut assets);
        update_commodity_prices(&solution, &mut prices);

        // Agent investment
        perform_agent_investment(&model, &mut assets);
    }
}

/// Get an iterator of active [`Asset`]s for the specified milestone year.
pub fn filter_assets(assets: &AssetPool, year: u32) -> impl Iterator<Item = &Asset> {
    assets
        .iter()
        .filter(move |asset| asset.commission_year >= year)
}

/// Perform agent investment to determine capacity investment of new assets for next milestone year.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
/// * `prices` - Commodity prices
fn perform_agent_investment(_model: &Model, _assets: &mut AssetPool) {
    info!("Performing agent investment...");
}
