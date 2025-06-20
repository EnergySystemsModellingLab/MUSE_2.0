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
    let year = year_iter.next().unwrap(); // NB: There will be at least one year

    info!("Milestone year: {year}");

    // There shouldn't be assets already commissioned, but let's do this just in case
    assets.decommission_old(year);

    // Newly commissioned assets will be included in optimisation for at least one milestone
    // year before agents have the option of decommissioning them
    assets.commission_new(year);

    // Dispatch optimisation
    let solution = perform_dispatch_optimisation(&model, &assets, &[], year)?;
    let flow_map = solution.create_flow_map();
    let prices = CommodityPrices::from_model_and_solution(&model, &solution);

    // Write active assets and results of dispatch optimisation to file. Note that we have to
    // be careful about when we call this function, as we want to include newly commissioned
    // assets and write them **before** assets have been decommissioned.
    writer.write(year, &solution, &assets, &flow_map, &prices)?;

    let break_after_first_year = true;

    for year in year_iter {
        info!("Milestone year: {year}");

        // NB: Agent investment is not carried out in first milestone year
        perform_agent_investment(&model, &flow_map, &prices, &mut assets);

        if break_after_first_year {
            // **TODO:** Remove this when we implement at least some of the agent investment code
            //   See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/304
            error!("Agent investment is not yet implemented. Exiting...");
            break;
        }

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Decommission assets whose lifetime has passed
        assets.decommission_old(year);

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, &[], year)?;
        let flow_map = solution.create_flow_map();
        let prices = CommodityPrices::from_model_and_solution(&model, &solution);

        // Write active assets and results of dispatch optimisation to file. Note that we have to
        // be careful about when we call this function, as we want to include newly commissioned
        // assets and write them **before** assets have been decommissioned.
        writer.write(year, &solution, &assets, &flow_map, &prices)?;
    }

    writer.flush()?;

    Ok(())
}
