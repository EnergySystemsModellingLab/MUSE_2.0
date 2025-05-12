//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::asset::{Asset, AssetID, AssetPool};
use crate::commodity::{BalanceType, CommodityID};
use crate::model::Model;
use crate::process::ProcessFlow;
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{anyhow, Result};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;

mod constraints;
use constraints::{add_asset_constraints, CapacityConstraintKeys, CommodityBalanceConstraintKeys};

/// A decision variable in the optimisation
///
/// Note that this type does **not** include the value of the variable; it just refers to a
/// particular column of the problem.
type Variable = highs::Col;

/// A map for easy lookup of variables in the problem.
///
/// The entries are ordered (see [`IndexMap`]).
///
/// We use this data structure for two things:
///
/// 1. In order define constraints for the optimisation
/// 2. To keep track of the combination of parameters that each variable corresponds to, for when we
///    are reading the results of the optimisation.
#[derive(Default)]
pub struct VariableMap(IndexMap<(AssetID, CommodityID, TimeSliceID), Variable>);

impl VariableMap {
    /// Get the [`Variable`] corresponding to the given parameters.
    fn get(
        &self,
        asset_id: AssetID,
        commodity_id: &CommodityID,
        time_slice: &TimeSliceID,
    ) -> Variable {
        let key = (asset_id, commodity_id.clone(), time_slice.clone());

        *self
            .0
            .get(&key)
            .expect("No variable found for given params")
    }
}

/// The solution to the dispatch optimisation problem
pub struct Solution<'a> {
    solution: highs::Solution,
    variables: VariableMap,
    time_slice_info: &'a TimeSliceInfo,
    commodity_balance_constraint_keys: CommodityBalanceConstraintKeys,
    capacity_constraint_keys: CapacityConstraintKeys,
}

impl Solution<'_> {
    /// Iterate over the newly calculated commodity flows for assets.
    ///
    /// Note that this only includes commodity flows which relate to assets, so not every commodity
    /// in the simulation will necessarily be represented.
    ///
    /// # Returns
    ///
    /// An iterator of tuples containing an asset ID, commodity, time slice and flow.
    pub fn iter_commodity_flows_for_assets(
        &self,
    ) -> impl Iterator<Item = (AssetID, &CommodityID, &TimeSliceID, f64)> {
        self.variables
            .0
            .keys()
            .zip(self.solution.columns().iter().copied())
            .map(|((asset_id, commodity_id, time_slice), flow)| {
                (*asset_id, commodity_id, time_slice, flow)
            })
    }

    /// Keys and dual values for commodity balance constraints.
    pub fn iter_commodity_balance_duals(
        &self,
    ) -> impl Iterator<Item = (&CommodityID, &RegionID, &TimeSliceID, f64)> {
        // Each commodity balance constraint applies to a particular time slice
        // selection (depending on time slice level). Where this covers multiple timeslices,
        // we return the same dual for each individual timeslice.
        self.commodity_balance_constraint_keys
            .iter()
            .zip(self.solution.dual_rows())
            .flat_map(|((commodity_id, region_id, ts_selection), price)| {
                self.time_slice_info
                    .iter_selection(ts_selection)
                    .map(move |(ts, _)| (commodity_id, region_id, ts, *price))
            })
    }

    /// Keys and dual values for capacity constraints.
    pub fn iter_capacity_duals(&self) -> impl Iterator<Item = (AssetID, &TimeSliceID, f64)> {
        self.capacity_constraint_keys
            .iter()
            .zip(
                self.solution.dual_rows()[self.commodity_balance_constraint_keys.len()..]
                    .iter()
                    .copied(),
            )
            .map(|((asset_id, time_slice), dual)| (*asset_id, time_slice, dual))
    }
}

/// Perform the dispatch optimisation.
///
/// For a detailed description, please see the [dispatch optimisation formulation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html
///
/// # Arguments
///
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns
///
/// A solution containing new commodity flows for assets and prices for (some) commodities.
pub fn perform_dispatch_optimisation<'a>(
    model: &'a Model,
    assets: &AssetPool,
    year: u32,
) -> Result<Solution<'a>> {
    // Set up problem
    let mut problem = Problem::default();
    let variables = add_variables(&mut problem, model, assets, year);

    // Add constraints
    let (commodity_balance_constraint_keys, capacity_constraint_keys) =
        add_asset_constraints(&mut problem, &variables, model, assets, year);

    // Solve problem
    let mut highs_model = problem.optimise(Sense::Minimise);

    // **HACK**: Dump output of HiGHS solver to stdout. Among other things, this includes the
    // objective value for the solution. Sadly it doesn't go via our logger, so this information
    // will not be included in the log file.
    //
    // Should be removed when we write the objective value to output data properly. See:
    //   https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/428
    enable_highs_logging(&mut highs_model);

    // Solve model
    let solution = highs_model.solve();
    match solution.status() {
        HighsModelStatus::Optimal => Ok(Solution {
            solution: solution.get_solution(),
            variables,
            time_slice_info: &model.time_slice_info,
            commodity_balance_constraint_keys,
            capacity_constraint_keys,
        }),
        status => Err(anyhow!("Could not solve: {status:?}")),
    }
}

/// Enable logging for the HiGHS solver
fn enable_highs_logging(model: &mut highs::Model) {
    // **HACK**: Skip this step if logging is disabled (e.g. when running tests)
    if let Ok(log_level) = std::env::var("MUSE2_LOG_LEVEL") {
        if log_level.eq_ignore_ascii_case("off") {
            return;
        }
    }

    model.set_option("log_to_console", true);
    model.set_option("output_flag", true);
}

/// Add variables to the optimisation problem.
///
/// # Arguments
///
/// * `problem` - The optimisation problem
/// * `model` - The model
/// * `assets` - The asset pool
/// * `year` - Current milestone year
///
/// # Returns
///
/// A [`VariableMap`] with the problem's variables as values.
fn add_variables(
    problem: &mut Problem,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) -> VariableMap {
    let mut variables = VariableMap::default();

    for asset in assets.iter() {
        for flow in asset.iter_flows() {
            for time_slice in model.time_slice_info.iter_ids() {
                let coeff = calculate_cost_coefficient(asset, flow, year, time_slice);

                // var's value must be <= 0 for inputs and >= 0 for outputs
                let var = if flow.flow < 0.0 {
                    problem.add_column(coeff, ..=0.0)
                } else {
                    problem.add_column(coeff, 0.0..)
                };

                let key = (asset.id, flow.commodity.id.clone(), time_slice.clone());
                let existing = variables.0.insert(key, var).is_some();
                assert!(!existing, "Duplicate entry for var");
            }
        }
    }

    variables
}

/// Calculate the cost coefficient for a decision variable
fn calculate_cost_coefficient(
    asset: &Asset,
    flow: &ProcessFlow,
    year: u32,
    time_slice: &TimeSliceID,
) -> f64 {
    // Cost per unit flow
    let mut coeff = flow.flow_cost;

    // Only applies if commodity is PAC
    if flow.is_pac {
        coeff += asset
            .process
            .parameters
            .get(&(asset.region_id.clone(), asset.commission_year))
            .unwrap()
            .variable_operating_cost
    }

    // If there is a user-provided cost for this commodity, include it
    if !flow.commodity.costs.is_empty() {
        let cost = flow
            .commodity
            .costs
            .get(&(asset.region_id.clone(), year, time_slice.clone()))
            .unwrap();
        let apply_cost = match cost.balance_type {
            BalanceType::Net => true,
            BalanceType::Consumption => flow.flow < 0.0,
            BalanceType::Production => flow.flow > 0.0,
        };

        if apply_cost {
            coeff += cost.value;
        }
    }

    // If flow is negative (representing an input), we multiply by -1 to ensure impact of
    // coefficient on objective function is a positive cost
    if flow.flow > 0.0 {
        coeff
    } else {
        -coeff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityCost, CommodityCostMap, CommodityType, DemandMap};
    use crate::process::{
        FlowType, Process, ProcessEnergyLimitsMap, ProcessFlowsMap, ProcessParameter,
        ProcessParameterMap,
    };
    use crate::time_slice::TimeSliceLevel;
    use float_cmp::assert_approx_eq;
    use std::collections::HashSet;
    use std::rc::Rc;

    fn get_cost_coeff_args(
        flow: f64,
        is_pac: bool,
        costs: CommodityCostMap,
    ) -> (Asset, ProcessFlow) {
        let process_param = Rc::new(ProcessParameter {
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 1.0,
        });
        let mut process_parameter_map = ProcessParameterMap::new();
        process_parameter_map.insert(("GBR".into(), 2010), process_param.clone());
        process_parameter_map.insert(("GBR".into(), 2020), process_param.clone());
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs,
            demand: DemandMap::new(),
        });
        let flow = ProcessFlow {
            commodity: Rc::clone(&commodity),
            flow,
            flow_type: FlowType::Fixed,
            flow_cost: 1.0,
            is_pac,
        };
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            years: vec![2010, 2020],
            energy_limits: ProcessEnergyLimitsMap::new(),
            flows: ProcessFlowsMap::new(),
            parameters: process_parameter_map,
            regions: HashSet::from([RegionID("GBR".into())]),
        });
        let asset = Asset::new(
            "agent1".into(),
            Rc::clone(&process),
            "GBR".into(),
            1.0,
            2010,
        )
        .unwrap();

        (asset, flow)
    }

    #[test]
    fn test_calculate_cost_coefficient() {
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };

        macro_rules! check_coeff {
            ($flow:expr, $is_pac:expr, $costs:expr, $expected:expr) => {
                let (asset, flow) = get_cost_coeff_args($flow, $is_pac, $costs);
                assert_approx_eq!(
                    f64,
                    calculate_cost_coefficient(&asset, &flow, 2010, &time_slice),
                    $expected
                );
            };
        }

        // not PAC, no commodity cost
        check_coeff!(1.0, false, CommodityCostMap::new(), 1.0);
        check_coeff!(-1.0, false, CommodityCostMap::new(), -1.0);

        // PAC, no commodity cost
        check_coeff!(1.0, true, CommodityCostMap::new(), 2.0);
        check_coeff!(-1.0, true, CommodityCostMap::new(), -2.0);

        // not PAC, commodity cost for output
        let cost = CommodityCost {
            balance_type: BalanceType::Production,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert(("GBR".into(), 2010, time_slice.clone()), cost);
        check_coeff!(1.0, false, costs.clone(), 3.0);
        check_coeff!(-1.0, false, costs, -1.0);

        // not PAC, commodity cost for output and input
        let cost = CommodityCost {
            balance_type: BalanceType::Net,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert(("GBR".into(), 2010, time_slice.clone()), cost);
        check_coeff!(1.0, false, costs.clone(), 3.0);
        check_coeff!(-1.0, false, costs, -3.0);

        // not PAC, commodity cost for input
        let cost = CommodityCost {
            balance_type: BalanceType::Consumption,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert(("GBR".into(), 2010, time_slice.clone()), cost);
        check_coeff!(1.0, false, costs.clone(), 1.0);
        check_coeff!(-1.0, false, costs, -3.0);

        // PAC, commodity cost for output
        let cost = CommodityCost {
            balance_type: BalanceType::Production,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert(("GBR".into(), 2010, time_slice.clone()), cost);
        check_coeff!(1.0, true, costs.clone(), 4.0);
        check_coeff!(-1.0, true, costs, -2.0);
    }
}
