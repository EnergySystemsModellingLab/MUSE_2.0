//! Functionality for running the MUSE 2.0 simulation.
use crate::asset::AssetPool;
use crate::model::Model;
use crate::output::DataWriter;
use crate::simulation::optimisation::{DispatchRunner, FlowMap};
use crate::simulation::prices::{update_prices_and_reduced_costs, ReducedCosts};
use anyhow::{Context, Result};
use log::info;
use std::path::Path;

pub mod investment;
pub mod optimisation;
use investment::perform_agent_investment;
pub mod prices;
pub use prices::CommodityPrices;

/// The outputs of the dispatch optimisation and price calculation steps
pub struct DispatchOutput {
    flow_map: FlowMap,
    prices: CommodityPrices,
    reduced_costs: ReducedCosts,
}

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
    let mut year_iter = model.iter_years().peekable();
    let year = year_iter.next().unwrap(); // NB: There will be at least one year

    info!("Milestone year: {year}");

    // There shouldn't be assets already commissioned, but let's do this just in case
    assets.decommission_old(year);

    // Commission assets for base year
    assets.commission_new(year);

    // Run dispatch to get flows, prices and reduced costs
    let next_year = year_iter.peek().copied();
    let mut output = run_dispatch_for_base_year(&model, &assets, year, next_year, &mut writer)?;

    for year in year_iter {
        info!("Milestone year: {year}");

        // Decommission assets whose lifetime has passed. We do this *before* agent investment, to
        // prevent agents from selecting assets that are being decommissioned in this milestone
        // year.
        assets.decommission_old(year);

        perform_agent_investment(&model, year, &mut assets, &mut output, &mut writer)
            .context("Agent investment failed")?;

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Write assets and results of dispatch optimisation to file
        writer.write(year, &assets, &output.flow_map, &output.prices)?;
    }

    writer.flush()?;

    Ok(())
}

// Run dispatch to get flows, prices and reduced costs for first milestone year
fn run_dispatch_for_base_year(
    model: &Model,
    assets: &AssetPool,
    year: u32,
    next_year: Option<u32>,
    writer: &mut DataWriter,
) -> Result<DispatchOutput> {
    let mut dispatch = DispatchRunner::new(year);

    // Dispatch optimisation with existing assets only
    let solution_existing = dispatch.run(model, assets, writer)?;
    let flow_map = solution_existing.create_flow_map();

    // Perform a separate dispatch run with existing assets and candidates (if there are any)
    let solution = dispatch
        .try_run_with_candidates(model, assets, next_year, writer)?
        .unwrap_or(solution_existing);

    // Calculate commodity prices and asset reduced costs
    let mut prices = CommodityPrices::default();
    let mut reduced_costs = ReducedCosts::default();
    update_prices_and_reduced_costs(
        model,
        &solution,
        assets,
        year,
        &mut prices,
        &mut reduced_costs,
    );

    // Write assets and results of dispatch optimisation to file
    writer.write(year, assets, &flow_map, &prices)?;

    Ok(DispatchOutput {
        flow_map,
        prices,
        reduced_costs,
    })
}
