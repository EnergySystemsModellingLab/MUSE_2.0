//! Functionality for running the MUSE 2.0 simulation.
use crate::asset::AssetPool;
use crate::model::Model;
use crate::output::DataWriter;
use anyhow::Result;
use log::{error, info};
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
/// * `output_path` - The folder to which output files will be written
/// * `debug_model` - Whether to write additional information (e.g. duals) to output files
pub fn run(
    model: Model,
    mut assets: AssetPool,
    output_path: &Path,
    debug_model: bool,
) -> Result<()> {
    let mut writer = DataWriter::create(output_path, &model.model_path, debug_model)?;

    // Iterate over milestone years
    let mut year_iter = model.iter_years();
    let mut year = year_iter.next().unwrap(); // NB: There will be at least one year

    // There shouldn't be assets already commissioned, but let's do this just in case
    assets.decommission_old(year);

    // **TODO:** Remove annotation when the loop actually loops
    #[allow(clippy::never_loop)]
    loop {
        info!("Milestone year: {year}");

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, &[], year)?;
        let flow_map = solution.create_flow_map();
        let prices = CommodityPrices::calculate(&model, &solution, year);

        // Write active assets and results of dispatch optimisation to file. Note that we have to
        // be careful about when we call this function, as we want to include newly commissioned
        // assets and write them **before** assets have been decommissioned.
        writer.write(year, &solution, &assets, &flow_map, &prices)?;

        if let Some(next_year) = year_iter.next() {
            year = next_year;

            // NB: Agent investment is not carried out in first milestone year
            perform_agent_investment(&model, &flow_map, &prices, &mut assets);

            // Decommission assets whose lifetime has passed
            assets.decommission_old(year);
        } else {
            // No more milestone years. Simulation is finished.
            break;
        }

        // **TODO:** Remove this when we implement at least some of the agent investment code
        //   See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/304
        error!("Agent investment is not yet implemented. Exiting...");
        break;
    }

    writer.flush()?;

    Ok(())
}
