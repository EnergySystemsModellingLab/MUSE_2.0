//! Functionality for running the MUSE 2.0 simulation.
use crate::agent::AssetPool;
use crate::model::Model;
use crate::output::{
    AssetsWriter, CapacityDualsWriter, CommodityFlowWriter, CommodityPricesWriter,
    FixedAssetsDualsWriter,
};
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
pub fn run(model: Model, mut assets: AssetPool, output_path: &Path) -> Result<()> {
    let mut assets_wtr = AssetsWriter::create(output_path)?;
    let mut flows_wtr = CommodityFlowWriter::create(output_path)?;
    let mut prices_wtr = CommodityPricesWriter::create(output_path)?;
    let mut fixed_duals_wtr = FixedAssetsDualsWriter::create(output_path)?;
    let mut capacity_duals_wtr = CapacityDualsWriter::create(output_path)?;

    let mut opt_solution = None;
    for year in model.iter_years() {
        info!("Milestone year: {year}");

        // Assets that have been decommissioned cannot be selected by agents
        assets.decomission_old(year);

        // NB: Agent investment is not carried out in first milestone year
        if let Some(solution) = opt_solution {
            perform_agent_investment(&model, &solution, &mut assets);

            // **TODO:** Remove this when we implement at least some of the agent investment code
            //   See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/304
            error!("Agent investment is not yet implemented. Exiting...");
            return Ok(());
        }

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Write current assets to CSV. This indicates the set of assets fed into the dispatch
        // optimisation, so we *must* do it after agent investment and new assets are commissioned
        assets_wtr.write(year, assets.iter())?;

        // Dispatch optimisation
        let solution = perform_dispatch_optimisation(&model, &assets, year)?;
        let prices = CommodityPrices::from_model_and_solution(&model, &solution);

        // Write result of dispatch optimisation to file
        flows_wtr.write(year, &assets, solution.iter_commodity_flows_for_assets())?;
        prices_wtr.write(year, &prices)?;
        fixed_duals_wtr.write(year, &assets, solution.iter_fixed_asset_duals())?;
        capacity_duals_wtr.write(year, &assets, solution.iter_capacity_duals())?;

        opt_solution = Some(solution);
    }

    Ok(())
}
