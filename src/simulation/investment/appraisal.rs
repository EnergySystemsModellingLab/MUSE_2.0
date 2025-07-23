//! Calculation for investment tools such as Levelised Cost of X (LCOX) and Net Present Value (NPV).
use crate::agent::ObjectiveType;
use crate::asset::AssetRef;
use crate::commodity::CommodityID;
use crate::finance::{lcox, profitability_index};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Capacity, Flow};
use anyhow::Result;
use indexmap::IndexMap;

mod coefficients;
mod constraints;
mod costs;
mod optimisation;
use coefficients::{calculate_coefficients_for_lcox, calculate_coefficients_for_npv};
use optimisation::perform_optimisation;

/// A map of demand across time slices
pub type DemandMap = IndexMap<TimeSliceID, Flow>;

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
    asset: &AssetRef,
    commodity_id: &CommodityID,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<AppraisalOutput> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_lcox(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity, activity and unmet demand
    let results = perform_optimisation(
        asset,
        commodity_id,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        highs::Sense::Minimise,
    )?;

    // Calculate LCOX for the hypothetical investment
    let annual_fixed_cost = coefficients.capacity_coefficient;
    let activity_costs = coefficients.activity_coefficients;
    let cost_index = lcox(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_costs,
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
    asset: &AssetRef,
    commodity_id: &CommodityID,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<AppraisalOutput> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_npv(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity, activity and unmet demand
    let results = perform_optimisation(
        asset,
        commodity_id,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        highs::Sense::Maximise,
    )?;

    // Calculate profitability index for the hypothetical investment
    let annual_fixed_cost = -coefficients.capacity_coefficient;
    let activity_surpluses = coefficients.activity_coefficients;
    let profitability_index = profitability_index(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_surpluses,
    );

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
    asset: &AssetRef,
    commodity_id: &CommodityID,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<AppraisalOutput> {
    let appraisal_method = match objective_type {
        ObjectiveType::LevelisedCostOfX => calculate_lcox,
        ObjectiveType::NetPresentValue => calculate_npv,
    };
    appraisal_method(
        asset,
        commodity_id,
        reduced_costs,
        demand,
        time_slice_info,
        time_slice_level,
    )
}
