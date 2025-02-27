//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use crate::output::write_commodity_prices_to_csv;
use anyhow::Result;
use log::info;
use std::fs::OpenOptions;
use std::path::Path;

pub mod optimisation;
use optimisation::perform_dispatch_optimisation;
pub mod investment;
use investment::perform_agent_investment;
pub mod prices;
use prices::update_commodity_prices;
pub use prices::CommodityPrices;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: Model, mut assets: AssetPool, output_path: &Path) -> Result<()> {
    let mut prices = CommodityPrices::default();

    let file_path = output_path.join("commodity_prices.csv");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;

    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Commission and decommission assets for this milestone year
        assets.decomission_old(year);
        assets.commission_new(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, year)?;
        update_commodity_prices(&model, &solution, &mut prices);

        // Agent investment
        perform_agent_investment(&model, &mut assets);

        // Write current commodity prices to CSV
        write_commodity_prices_to_csv(&mut file, year, &prices)?;
    }

    Ok(())
}
