//! Code for updating the simulation state.
use super::optimisation::Solution;
use super::CommodityPrices;
use crate::agent::AssetPool;
use crate::commodity::CommodityMap;
use log::info;
use std::collections::HashSet;
use std::rc::Rc;

/// Update commodity flows for assets based on the result of the dispatch optimisation.
pub fn update_commodity_flows(_solution: &Solution, _assets: &mut AssetPool) {
    info!("Updating commodity flows...");
}

/// Update commodity prices for assets based on the result of the dispatch optimisation.
pub fn update_commodity_prices(
    commodities: &CommodityMap,
    solution: &Solution,
    prices: &mut CommodityPrices,
) {
    info!("Updating commodity prices...");
    let commodities_updated = update_commodity_prices_from_solution(solution, prices);

    // Find commodities not updated in last step
    let remaining_commodities = commodities
        .keys()
        .filter(|id| !commodities_updated.contains(*id))
        .cloned();
    update_remaining_commodity_prices(remaining_commodities, prices);
}

/// Update the commodity prices for which there are values in the solution
fn update_commodity_prices_from_solution(
    solution: &Solution,
    prices: &mut CommodityPrices,
) -> HashSet<Rc<str>> {
    info!("Updating commodity prices...");

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
fn update_remaining_commodity_prices<I>(_commodity_ids: I, _prices: &mut CommodityPrices)
where
    I: Iterator<Item = Rc<str>>,
{
    info!("Updating remaining commodity prices...");
}
