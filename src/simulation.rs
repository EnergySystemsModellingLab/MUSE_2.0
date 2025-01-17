//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::{Asset, AssetPool};
use crate::model::Model;
use log::info;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// A map relating commodity ID to current price (endogenous)
type CommodityPrices = HashMap<Rc<str>, f64>;

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

        let remaining_commodities = perform_dispatch(&model, &assets, year, &mut prices);
        update_remaining_commodity_prices(&remaining_commodities, &mut prices);
        perform_agent_investment(&model, &mut assets);
    }
}

/// Perform the dispatch optimisation.
///
/// Updates commodity flows for assets and commodity prices.
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
/// * `prices` - Commodity prices
///
/// # Returns
///
/// A set of IDs for commodities whose prices weren't updated.
fn perform_dispatch(
    _model: &Model,
    assets: &AssetPool,
    year: u32,
    _prices: &mut CommodityPrices,
) -> HashSet<Rc<str>> {
    info!("Performing dispatch optimisation...");
    for asset in filter_assets(assets, year) {
        for _flow in asset.process.flows.iter() {
            // **TODO**: Write code for optimisation
        }
    }

    // **PLACEHOLDER**: Should return IDs of commodities whose prices weren't updated
    HashSet::new()
}

/// Get an iterator of active [`Asset`]s for the specified milestone year.
fn filter_assets(assets: &AssetPool, year: u32) -> impl Iterator<Item = &Asset> {
    assets
        .iter()
        .filter(move |asset| asset.commission_year >= year)
}

/// Update prices for any commodity not updated by the dispatch step.
///
/// **TODO**: This will likely take additional arguments, depending on how we decide to do this step
///
/// # Arguments
///
/// * `commodity_ids` - IDs of commodities to update
/// * `prices` - Commodity prices
fn update_remaining_commodity_prices(
    _commodity_ids: &HashSet<Rc<str>>,
    _prices: &mut CommodityPrices,
) {
    info!("Updating remaining commodity prices...");
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
