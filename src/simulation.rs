//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use crate::output::DataWriter;
use anyhow::{bail, Result};
use log::info;
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
    let mut writer = DataWriter::create(output_path)?;

    let mut opt_results = None; // all results of dispatch optimisation
    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Assets that have been decommissioned cannot be selected by agents
        assets.decomission_old(year);

        // NB: Agent investment is not carried out in first milestone year
        if let Some((solution, prices)) = opt_results {
            perform_agent_investment(&model, &solution, &prices, &mut assets);

            // **TODO:** Remove this when we implement at least some of the agent investment code
            //   See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/304
            bail!("Agent investment is not yet implemented. Exiting...");
        }

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, year)?;
        let prices = CommodityPrices::from_model_and_solution(&model, &solution);

        // Write current commodity prices to CSV
        writer.write_prices(year, &prices)?;

        opt_results = Some((solution, prices));
    }

    writer.flush()?;

    Ok(())
}
