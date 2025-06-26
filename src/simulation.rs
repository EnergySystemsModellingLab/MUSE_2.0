//! Functionality for running the MUSE 2.0 simulation.
use crate::asset::{Asset, AssetPool, AssetRef};
use crate::model::Model;
use crate::output::DataWriter;
use crate::process::ProcessMap;
use crate::units::Capacity;
use anyhow::Result;
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

    // Commission assets for baseline year
    assets.commission_new(year);

    // Dispatch optimisation
    let candidates = if let Some(next_year) = year_iter.peek() {
        candidate_assets_for_year(&model.processes, *next_year)
    } else {
        // If there is only one milestone year, there are no candidates for next year
        Vec::new()
    };
    let solution = perform_dispatch_optimisation(&model, &assets, &candidates, year)?;
    let flow_map = solution.create_flow_map();

    // Calculate prices. We need to know the scarcity-adjusted prices as well as unadjusted. Note
    // that the order of operations is important.
    let shadow_prices = CommodityPrices::from_iter(solution.iter_commodity_balance_duals());
    let adjusted_prices = shadow_prices
        .clone()
        .without_scarcity_pricing(solution.iter_activity_duals())
        .with_levies(&model, year);
    let unadjusted_prices = shadow_prices.with_levies(&model, year);

    // Write active assets and results of dispatch optimisation to file
    writer.write(year, &solution, &assets, &flow_map, &adjusted_prices)?;

    for year in year_iter {
        info!("Milestone year: {year}");

        // Decommission assets whose lifetime has passed. We do this *before* agent investment, to
        // prevent agents from selecting assets that are being decommissioned in this milestone
        // year.
        assets.decommission_old(year);

        // NB: Agent investment will actually be in a loop with more calls to
        // `perform_dispatch_optimisation`, but let's leave this as a placeholder for now
        perform_agent_investment(
            &model,
            &solution,
            &flow_map,
            &adjusted_prices,
            &unadjusted_prices,
            &assets,
            year,
        );

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // **TODO:** Write output data for this milestone year. Skipping for now as agent investment
        // is not implemented, so data will just be a duplicate of first milestone year.
    }

    writer.flush()?;

    Ok(())
}

/// Get all candidate assets for a specified year
fn candidate_assets_for_year(processes: &ProcessMap, year: u32) -> Vec<AssetRef> {
    let mut candidates = Vec::new();
    for process in processes
        .values()
        .filter(move |process| process.active_for_year(year))
    {
        for region_id in process.regions.iter() {
            candidates.push(
                Asset::new(
                    None,
                    process.clone(),
                    region_id.clone(),
                    Capacity(0.0),
                    year,
                )
                .unwrap()
                .into(),
            );
        }
    }

    candidates
}
