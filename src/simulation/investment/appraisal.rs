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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityType};
    use crate::fixture::{asset, commodity_id, process, time_slice};
    use crate::process::{FlowType, Process, ProcessFlow};
    use crate::region::RegionID;
    use crate::time_slice::TimeSliceLevel;
    use crate::units::{Activity, FlowPerActivity, MoneyPerFlow};
    use indexmap::IndexMap;
    use rstest::rstest;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[rstest]
    fn test_lcoxoutput_comparison_metric() {
        let output = LCOXOutput {
            cost_index: MoneyPerActivity::new(42.0),
            unmet_demand: IndexMap::new(),
        };
        assert_eq!(output.comparison_metric(), 42.0);
    }

    #[rstest]
    fn test_lcoxoutput_into_unmet_demand(
        asset: Asset,
        commodity_id: CommodityID,
        time_slice: TimeSliceID,
    ) {
        let mut demand = IndexMap::new();
        demand.insert(time_slice.clone(), Flow::new(3.0));
        let demand2 = demand.clone();
        let mut output = LCOXOutput {
            cost_index: MoneyPerActivity::new(1.0),
            unmet_demand: demand.clone(),
        };
        output.update_demand(&mut demand);
        assert_eq!(demand, demand2);
    }

    #[rstest]
    fn test_npvoutput_comparison_metric() {
        let output = NPVOutput {
            profitability_index: Dimensionless::new(7.0),
            unmet_demand: IndexMap::new(),
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
        output.update_demand(&mut demand);
        // Should subtract activity * coeff from prev_demand
        let expected = 20.0 - 5.0 * 2.0;
        assert_eq!(demand[&time_slice], Flow::new(expected));
    }
}
