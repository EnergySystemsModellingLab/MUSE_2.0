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

    // Write assets to file
    writer.write_assets(assets.iter_all())?;

    // Run dispatch optimisation
    info!("Running dispatch optimisation...");
    let next_year = year_iter.peek().copied();
    let (flow_map, prices, mut reduced_costs) =
        run_dispatch_for_year(&model, assets.as_slice(), year, next_year, &mut writer)?;

    // Write results of dispatch optimisation to file
    writer.write_flows(year, &flow_map)?;
    writer.write_prices(year, &prices)?;
    writer.write_debug_reduced_costs(year, &reduced_costs)?;

    while let Some(year) = year_iter.next() {
        info!("Milestone year: {year}");

        // Decommission assets whose lifetime has passed. We do this *before* agent investment, to
        // prevent agents from selecting assets that are being decommissioned in this milestone
        // year.
        assets.decommission_old(year);

        // Newly commissioned assets will be included in optimisation for at least one milestone
        // year before agents have the option of decommissioning them
        assets.commission_new(year);

        // Perform agent investment
        info!("Running agent investment...");
        perform_agent_investment(&model, year, &mut assets, &reduced_costs, &mut writer)
            .context("Agent investment failed")?;

        // Write assets
        writer.write_assets(assets.iter_all())?;

        // Run dispatch optimisation
        info!("Running dispatch optimisation...");
        let next_year = year_iter.peek().copied();
        let (flow_map, prices, new_reduced_costs) =
            run_dispatch_for_year(&model, assets.as_slice(), year, next_year, &mut writer)?;

        // Write results of dispatch optimisation to file
        writer.write_flows(year, &flow_map)?;
        writer.write_prices(year, &prices)?;
        writer.write_debug_reduced_costs(year, &new_reduced_costs)?;

        // Reduced costs for the next year
        reduced_costs = new_reduced_costs;
    }

    writer.flush()?;

    Ok(())
}

// Run dispatch to get flows, prices and reduced costs for a milestone year
fn run_dispatch_for_year(
    model: &Model,
    assets: &[AssetRef],
    year: u32,
    next_year: Option<u32>,
    writer: &mut DataWriter,
) -> Result<(FlowMap, CommodityPrices, ReducedCosts)> {
    // Dispatch optimisation with existing assets only
    let solution_existing = perform_dispatch_optimisation(
        model,
        assets,
        &[],
        None,
        year,
        "final without candidates",
        writer,
    )?;
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
        perform_dispatch_optimisation(
            model,
            assets,
            &candidates,
            None,
            year,
            "final with candidates",
            writer,
        )?
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
                Asset::new_mock(
                    Rc::clone(process),
                    region_id.clone(),
                    year,
                    candidate_asset_capacity,
                )
                .into(),
            );
        }
    }

    candidates
}
