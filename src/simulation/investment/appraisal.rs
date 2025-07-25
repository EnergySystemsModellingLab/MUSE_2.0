//! Calculation for investment tools such as Levelised Cost of X (LCOX) and Net Present Value (NPV).
use super::DemandMap;
use crate::agent::ObjectiveType;
use crate::asset::AssetRef;
use crate::commodity::Commodity;
use crate::finance::{lcox, profitability_index};
use crate::model::Model;
use crate::simulation::prices::ReducedCosts;
use crate::units::Capacity;
use anyhow::Result;

mod coefficients;
mod constraints;
mod costs;
mod optimisation;
use coefficients::{calculate_coefficients_for_lcox, calculate_coefficients_for_npv};
use optimisation::perform_optimisation;

/// The output of investment appraisal required to compare potential investment decisions
pub struct AppraisalOutput {
    /// The asset being appraised
    pub asset: AssetRef,
    /// The hypothetical capacity to install
    pub capacity: Capacity,
    /// The hypothetical unmet demand following investment in this asset
    pub unmet_demand: DemandMap,
    /// The comparison metric to compare investment decisions (lower is better)
    pub metric: f64,
}

/// Calculate LCOX for a hypothetical investment in the given asset.
///
/// This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
/// include other flows, we use the term LCOX.
fn calculate_lcox(
    model: &Model,
    asset: &AssetRef,
    max_capacity: Option<Capacity>,
    commodity: &Commodity,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
) -> Result<AppraisalOutput> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_lcox(
        asset,
        &model.time_slice_info,
        reduced_costs,
        model.parameters.value_of_lost_load,
    );

    // Perform optimisation to calculate capacity, activity and unmet demand
    let results = perform_optimisation(
        asset,
        max_capacity,
        commodity,
        &coefficients,
        demand,
        &model.time_slice_info,
        highs::Sense::Minimise,
    )?;

    // Calculate LCOX for the hypothetical investment
    let cost_index = lcox(
        &results.activity,
        &results.unmet_demand,
        results.objective_value,
        model.parameters.value_of_lost_load,
    );

    // Return appraisal output
    Ok(AppraisalOutput {
        asset: asset.clone(),
        capacity: results.capacity,
        unmet_demand: results.unmet_demand,
        metric: cost_index.value(),
    })
}

/// Calculate NPV for a hypothetical investment in the given asset.
fn calculate_npv(
    model: &Model,
    asset: &AssetRef,
    max_capacity: Option<Capacity>,
    commodity: &Commodity,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
) -> Result<AppraisalOutput> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_npv(asset, &model.time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity, activity and unmet demand
    let results = perform_optimisation(
        asset,
        max_capacity,
        commodity,
        &coefficients,
        demand,
        &model.time_slice_info,
        highs::Sense::Maximise,
    )?;

    // Calculate profitability index for the hypothetical investment
    let annual_fixed_cost = -coefficients.capacity_coefficient;
    let profitability_index =
        profitability_index(results.capacity, annual_fixed_cost, results.objective_value);

    // Return appraisal output
    // Higher profitability index is better, so we make it negative for comparison
    Ok(AppraisalOutput {
        asset: asset.clone(),
        capacity: results.capacity,
        unmet_demand: results.unmet_demand,
        metric: -profitability_index.value(),
    })
}

/// Appraise the given investment with the specified objective type
pub fn appraise_investment(
    model: &Model,
    asset: &AssetRef,
    max_capacity: Option<Capacity>,
    commodity: &Commodity,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
) -> Result<AppraisalOutput> {
    let appraisal_method = match objective_type {
        ObjectiveType::LevelisedCostOfX => calculate_lcox,
        ObjectiveType::NetPresentValue => calculate_npv,
    };
    appraisal_method(model, asset, max_capacity, commodity, reduced_costs, demand)
}
