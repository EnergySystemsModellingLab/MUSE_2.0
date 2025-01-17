//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::agent::AssetPool;
use crate::model::Model;
use crate::simulation::{filter_assets, CommodityPrices};
use log::info;
use std::collections::HashSet;
use std::rc::Rc;

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
pub fn perform_dispatch(
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
