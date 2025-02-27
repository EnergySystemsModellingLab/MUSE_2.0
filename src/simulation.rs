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
pub use prices::CommodityPrices;

/// Run the simulation.
///
/// # Arguments:
///
/// * `model` - The model to run
/// * `assets` - The asset pool
pub fn run(model: Model, mut assets: AssetPool, output_path: &Path) -> Result<()> {
    let file_path = output_path.join("commodity_prices.csv");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;

    let mut opt_solution = None;
    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Assets that have been decommissioned cannot be selected by agents
        assets.decomission_old(year);

        // NB: Agent investment is not carried out in first milestone year
        if let Some(solution) = opt_solution {
            perform_agent_investment(&model, &solution, &mut assets);
        }

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, year)?;
        let prices = CommodityPrices::from_model_and_solution(&model, &solution);
        opt_solution = Some(solution);

        // Write current commodity prices to CSV
        write_commodity_prices_to_csv(&mut file, year, &prices)?;
    }

    Ok(())
}
