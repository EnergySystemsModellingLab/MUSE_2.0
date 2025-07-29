//! Functionality for running the MUSE 2.0 simulation.
use crate::asset::{Asset, AssetPool, AssetRef};
use crate::model::Model;
use crate::output::DataWriter;
use crate::process::ProcessMap;
use crate::simulation::optimisation::FlowMap;
use crate::simulation::prices::{update_prices_and_reduced_costs, ReducedCosts};
use crate::units::Capacity;
use anyhow::{Context, Result};
use log::info;
use std::path::Path;
use std::rc::Rc;

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

    // Commission assets for base year
    assets.commission_new(year);

    // Run dispatch to get flows, prices and reduced costs
    let next_year = year_iter.peek().copied();
    let (flow_map, _prices, reduced_costs) =
        run_dispatch_for_base_year(&model, &assets, year, next_year, &mut writer)?;

    for year in year_iter {
        info!("Milestone year: {year}");

        // Decommission assets whose lifetime has passed. We do this *before* agent investment, to
        // prevent agents from selecting assets that are being decommissioned in this milestone
        // year.
        assets.decommission_old(year);

        // NB: Agent investment will actually be in a loop with more calls to
        // `perform_dispatch_optimisation`, but let's leave this as a placeholder for now
        perform_agent_investment(&model, &flow_map, &reduced_costs, &mut assets, year)
            .context("Agent investment failed")?;

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // **TODO:** Write output data for this milestone year. Skipping for now as agent investment
        // is not implemented, so data will just be a duplicate of first milestone year.
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
) -> Result<(FlowMap, CommodityPrices, ReducedCosts)> {
    // Dispatch optimisation with existing assets only
    let solution_existing = perform_dispatch_optimisation(model, assets, &[], year, 0, writer)?;
    let flow_map = solution_existing.create_flow_map();

    // Get candidate assets for next year, if any
    let candidates = next_year
        .map(|next_year| {
            candidate_assets_for_year(
                &model.processes,
                next_year,
                model.parameters.candidate_asset_capacity,
            )
        })
        .unwrap_or_default();

    // Perform a separate dispatch run with existing assets and candidates (if there are any)
    let solution = if candidates.is_empty() {
        solution_existing
    } else {
        perform_dispatch_optimisation(model, assets, &candidates, year, 1, writer)?
    };

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

    // Write active assets and results of dispatch optimisation to file
    writer.write(year, assets, &flow_map, &prices)?;

    Ok((flow_map, prices, reduced_costs))
}

/// Get all candidate assets for a specified year
fn candidate_assets_for_year(
    processes: &ProcessMap,
    year: u32,
    candidate_asset_capacity: Capacity,
) -> Vec<AssetRef> {
    let mut candidates = Vec::new();
    for process in processes
        .values()
        .filter(move |process| process.active_for_year(year))
    {
        for region_id in process.regions.iter() {
            candidates.push(
                Asset::new(
                    None,
                    Rc::clone(process),
                    region_id.clone(),
                    candidate_asset_capacity,
                    year,
                )
                .unwrap()
                .into(),
            );
        }
    }

    candidates
}
