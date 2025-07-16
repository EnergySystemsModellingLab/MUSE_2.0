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

impl AppraisalOutput {
    /// Whether this [`AppraisalOutput`] indicates a better result than `other`
    pub fn is_better_than(&self, other: &AppraisalOutput) -> bool {
        self.tool_output.comparison_metric() < other.tool_output.comparison_metric()
    }

    /// Update the demand map if this asset is selected.
    ///
    /// This function should only be called once and may panic if called subsequently.
    pub fn update_demand(&mut self, commodity_id: &CommodityID, demand: &mut DemandMap) {
        self.tool_output
            .update_demand(&self.asset, commodity_id, demand);
    }
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

    /// Update the demand, if the appraised asset is selected.
    ///
    /// This function should only be called once and may panic if called subsequently.
    fn update_demand(&mut self, asset: &Asset, commodity_id: &CommodityID, demand: &mut DemandMap);
}

/// Additional output data for LCOX
struct LCOXOutput {
    cost_index: MoneyPerActivity,
    unmet_demand: HashMap<TimeSliceID, Flow>,
}

impl ToolOutput for LCOXOutput {
    fn comparison_metric(&self) -> f64 {
        self.cost_index.value()
    }

    fn update_demand(
        &mut self,
        _asset: &Asset,
        _commodity_id: &CommodityID,
        demand: &mut DemandMap,
    ) {
        assert!(!self.unmet_demand.is_empty(), "update_demand called twice");
        *demand = std::mem::take(&mut self.unmet_demand);
    }
}

/// Additional output data for NPV
struct NPVOutput {
    profitability_index: Dimensionless,
    activity: IndexMap<TimeSliceID, Activity>,
}

impl ToolOutput for NPVOutput {
    fn comparison_metric(&self) -> f64 {
        // A higher profitability index indicates a better result, so we make it negative for
        // comparing
        -self.profitability_index.value()
    }

    fn update_demand(&mut self, asset: &Asset, commodity_id: &CommodityID, demand: &mut DemandMap) {
        let coeff = asset.get_flow(commodity_id).unwrap().coeff;

        // Subtract the flow produced by this asset for this commodity from previous demand
        for (time_slice, demand) in demand.iter_mut() {
            let activity = self.activity.get(time_slice).unwrap();
            *demand -= *activity * coeff;
        }
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
fn calculate_lcox(
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
fn calculate_npv(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityType};
    use crate::fixture::{asset, commodity_id, process, time_slice};
    use crate::process::{FlowType, Process, ProcessFlow};
    use crate::region::RegionID;
    use crate::time_slice::TimeSliceLevel;
    use crate::units::{FlowPerActivity, MoneyPerFlow};
    use indexmap::IndexMap;
    use rstest::rstest;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[rstest]
    fn test_lcoxoutput_comparison_metric() {
        let output = LCOXOutput {
            cost_index: MoneyPerActivity::new(42.0),
            unmet_demand: HashMap::new(),
        };
        assert_eq!(output.comparison_metric(), 42.0);
    }

    #[rstest]
    fn test_lcoxoutput_into_unmet_demand(
        asset: Asset,
        commodity_id: CommodityID,
        time_slice: TimeSliceID,
    ) {
        let mut demand = HashMap::new();
        demand.insert(time_slice.clone(), Flow::new(3.0));
        let demand2 = demand.clone();
        let mut output = LCOXOutput {
            cost_index: MoneyPerActivity::new(1.0),
            unmet_demand: demand.clone(),
        };
        output.update_demand(&asset, &commodity_id, &mut demand);
        assert_eq!(demand, demand2);
    }

    #[rstest]
    fn test_npvoutput_comparison_metric() {
        let output = NPVOutput {
            profitability_index: Dimensionless::new(7.0),
            activity: IndexMap::new(),
        };
        // Should be negative of profitability_index
        assert_eq!(output.comparison_metric(), -7.0);
    }

    #[rstest]
    fn test_npvoutput_into_unmet_demand(
        process: Process,
        commodity_id: CommodityID,
        time_slice: TimeSliceID,
        mut asset: Asset,
    ) {
        // Clone and modify the process to add a flow for the commodity
        let mut process = process;
        let region: RegionID = "GBR".into();
        let year = 2015;
        let coeff = FlowPerActivity::new(2.0);
        let commodity = Rc::new(Commodity {
            id: commodity_id.clone(),
            description: String::new(),
            kind: CommodityType::ServiceDemand,
            time_slice_level: TimeSliceLevel::DayNight,
            levies: Default::default(),
            demand: Default::default(),
        });
        let flow = ProcessFlow {
            commodity: commodity.clone(),
            coeff,
            kind: FlowType::Fixed,
            cost: MoneyPerFlow::new(0.0),
            is_primary_output: true,
        };
        let mut flows_map = IndexMap::new();
        flows_map.insert(commodity_id.clone(), flow);
        process.flows.insert((region.clone(), year), flows_map);
        asset.process = Rc::new(process);
        let mut activity = IndexMap::new();
        activity.insert(time_slice.clone(), Activity::new(5.0));
        let mut demand = HashMap::new();
        demand.insert(time_slice.clone(), Flow::new(20.0));
        let mut output = NPVOutput {
            profitability_index: Dimensionless::new(1.0),
            activity,
        };
        output.update_demand(&asset, &commodity_id, &mut demand);
        // Should subtract activity * coeff from prev_demand
        let expected = 20.0 - 5.0 * 2.0;
        assert_eq!(demand[&time_slice], Flow::new(expected));
    }
}
