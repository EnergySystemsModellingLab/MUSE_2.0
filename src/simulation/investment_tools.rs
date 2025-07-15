//! Calculation for investment tools such as Levelised Cost of X (LCOX) and Net Present Value (NPV).
use crate::agent::ObjectiveType;
use crate::asset::{Asset, AssetRef};
use crate::commodity::CommodityID;
use crate::finance::{lcox, profitability_index};
use crate::simulation::prices::ReducedCosts;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use crate::units::{Activity, Capacity, Dimensionless, Flow, MoneyPerActivity};
use anyhow::Result;
use indexmap::IndexMap;
use std::collections::HashMap;

mod coefficients;
mod constraints;
mod costs;
mod optimisation;
use coefficients::{calculate_coefficients_for_lcox, calculate_coefficients_for_npv};
use optimisation::perform_optimisation;

/// A map of demand across time slices
pub type DemandMap = HashMap<TimeSliceID, Flow>;

/// The output of investment appraisal
pub struct AppraisalOutput {
    /// The asset being appraised
    pub asset: AssetRef,
    /// The required capacity for the asset
    pub capacity: Capacity,
    /// Additional, tool-specific output information
    pub tool_output: Box<dyn ToolOutput>,
}

/// A trait representing tool-specific output information for a particular asset
pub trait ToolOutput {
    /// Return the comparison metric for this output.
    ///
    /// A lower value of this number indicates a better result, but may not have any meaning beyond
    /// that.
    ///
    /// It is a logic error to compare comparison metrics returned by different appraisal tools.
    fn comparison_metric(&self) -> f64;

    /// Convert this [`ToolOutput`] into a map of the remaining demand, if the asset were selected.
    ///
    /// It is assumed that `previous_demand` has entries for every time slice and it is a logic
    /// error if not.
    fn into_unmet_demand(
        self,
        asset: &Asset,
        commodity_id: &CommodityID,
        previous_demand: DemandMap,
    ) -> DemandMap;
}

/// Additional output data for LCOX
pub struct LCOXOutput {
    cost_index: MoneyPerActivity,
    unmet_demand: HashMap<TimeSliceID, Flow>,
}

impl ToolOutput for LCOXOutput {
    fn comparison_metric(&self) -> f64 {
        self.cost_index.value()
    }

    fn into_unmet_demand(
        self,
        _asset: &Asset,
        _commodity_id: &CommodityID,
        _previous_demand: DemandMap,
    ) -> DemandMap {
        self.unmet_demand
    }
}

/// Additional output data for NPV
pub struct NPVOutput {
    profitability_index: Dimensionless,
    activity: IndexMap<TimeSliceID, Activity>,
}

impl ToolOutput for NPVOutput {
    fn comparison_metric(&self) -> f64 {
        // A higher profitability index indicates a better result, so we make it negative for
        // comparing
        -self.profitability_index.value()
    }

    fn into_unmet_demand(
        self,
        asset: &Asset,
        commodity_id: &CommodityID,
        previous_demand: DemandMap,
    ) -> DemandMap {
        let coeff = asset.get_flow(commodity_id).unwrap().coeff;

        // Subtract the flow produced by this asset for this commodity from previous demand
        let mut demand = previous_demand;
        for (time_slice, demand) in demand.iter_mut() {
            let activity = self.activity.get(time_slice).unwrap();
            *demand -= *activity * coeff;
        }

        demand
    }
}

/// Calculate LCOX based on the specified reduced costs and demand for a particular tranche.
///
/// This is more commonly referred to as Levelised Cost of *Electricity*, but as the model can
/// include other flows, we use the term LCOX.
///
/// # Returns
///
/// Required capacity for asset and additional information in [`LCOXOutput`].
pub fn calculate_lcox(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<(Capacity, LCOXOutput)> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_lcox(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation(
        asset,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        highs::Sense::Minimise,
    )?;

    // Calculate LCOX
    let annual_fixed_cost = coefficients.capacity_coefficient;
    let activity_costs = coefficients.activity_coefficients;
    let cost_index = lcox(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_costs,
    );

    // Placeholder for unmet demand (**TODO.**)
    let unmet_demand = demand.keys().cloned().map(|ts| (ts, Flow(0.0))).collect();

    Ok((
        results.capacity,
        LCOXOutput {
            cost_index,
            unmet_demand,
        },
    ))
}

/// Calculate NPV based on the specified reduced costs and demand for a particular tranche.
///
/// # Returns
///
/// Required capacity for asset and additional information in [`NPVOutput`].
pub fn calculate_npv(
    asset: &AssetRef,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<(Capacity, NPVOutput)> {
    // Calculate coefficients
    let coefficients = calculate_coefficients_for_npv(asset, time_slice_info, reduced_costs);

    // Perform optimisation to calculate capacity and activity
    let results = perform_optimisation(
        asset,
        &coefficients,
        demand,
        time_slice_info,
        time_slice_level,
        highs::Sense::Maximise,
    )?;

    // Calculate profitability index
    let annual_fixed_cost = -coefficients.capacity_coefficient;
    let activity_surpluses = coefficients.activity_coefficients;
    let profitability_index = profitability_index(
        results.capacity,
        annual_fixed_cost,
        &results.activity,
        &activity_surpluses,
    );

    Ok((
        results.capacity,
        NPVOutput {
            profitability_index,
            activity: results.activity,
        },
    ))
}

/// Appraise the given investment with the specified objective type
pub fn appraise_investment(
    asset: &AssetRef,
    objective_type: &ObjectiveType,
    reduced_costs: &ReducedCosts,
    demand: &DemandMap,
    time_slice_info: &TimeSliceInfo,
    time_slice_level: TimeSliceLevel,
) -> Result<AppraisalOutput> {
    // Macro to reduce boilerplate
    macro_rules! appraisal_method {
        ($fn: ident) => {{
            let (capacity, output) = $fn(
                asset,
                reduced_costs,
                demand,
                time_slice_info,
                time_slice_level,
            )?;

            Ok(AppraisalOutput {
                asset: asset.clone(),
                capacity,
                tool_output: Box::new(output),
            })
        }};
    }

    // Delegate appraisal to relevant function
    match objective_type {
        ObjectiveType::LevelisedCostOfX => appraisal_method!(calculate_lcox),
        ObjectiveType::NetPresentValue => appraisal_method!(calculate_npv),
    }
}
