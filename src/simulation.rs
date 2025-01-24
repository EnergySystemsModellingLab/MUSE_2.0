//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use log::info;
use std::collections::HashMap;
use std::rc::Rc;

pub mod optimisation;
use optimisation::perform_dispatch_optimisation;
pub mod investment;
use investment::perform_agent_investment;
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
