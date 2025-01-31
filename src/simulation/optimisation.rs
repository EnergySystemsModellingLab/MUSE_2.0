//! Code for performing dispatch optimisation.
//!
//! This is used to calculate commodity flows and prices.
use crate::agent::{Asset, AssetPool};
use crate::commodity::BalanceType;
use crate::model::Model;
use crate::process::ProcessFlow;
use crate::simulation::filter_assets;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use highs::{HighsModelStatus, RowProblem as Problem, Sense};
use indexmap::IndexMap;
use log::{error, info};
use std::iter;
use std::rc::Rc;

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
pub struct VariableMap(IndexMap<VariableMapKey, Variable>);

impl VariableMap {
    /// Get the [`Variable`] corresponding to the given parameters.
    fn get(&self, asset_id: u32, commodity_id: &Rc<str>, time_slice: &TimeSliceID) -> Variable {
        let key = VariableMapKey {
            asset_id,
            commodity_id: Rc::clone(commodity_id),
            time_slice: time_slice.clone(),
        };

        *self
            .0
            .get(&key)
            .expect("No variable found for given params")
    }
}

/// A key for a [`VariableMap`]
#[derive(Eq, PartialEq, Hash)]
pub struct VariableMapKey {
    asset_id: u32,
    commodity_id: Rc<str>,
    time_slice: TimeSliceID,
}

impl VariableMapKey {
    /// Create a new [`VariableMapKey`]
    fn new(asset_id: u32, commodity_id: Rc<str>, time_slice: TimeSliceID) -> Self {
        Self {
            asset_id,
            commodity_id,
            time_slice,
        }
    }
}

/// The solution to the dispatch optimisation problem
pub struct Solution {
    variables: VariableMap,
    solution: highs::Solution,
}

impl Solution {
    /// Iterate over the newly calculated commodity flows for assets.
    ///
    /// Note that this only includes commodity flows which relate to assets, so not every commodity
    /// in the simulation will necessarily be represented.
    pub fn iter_commodity_flows_for_assets(&self) -> impl Iterator<Item = (&VariableMapKey, f64)> {
        self.variables
            .0
            .keys()
            .zip(self.solution.columns().iter().copied())
    }

    /// Iterate over the newly calculated commodity prices.
    ///
    /// Note that there may only be prices for a subset of the commodities; the rest will need to be
    /// calculated in another way.
    pub fn iter_commodity_prices(&self) -> impl Iterator<Item = (&Rc<str>, f64)> {
        // **PLACEHOLDER**
        iter::empty()
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
pub fn perform_dispatch_optimisation(model: &Model, assets: &AssetPool, year: u32) -> Solution {
    info!("Performing dispatch optimisation...");

    // Set up problem
    let mut problem = Problem::default();
    let variables = add_variables(&mut problem, model, assets, year);

    // Add constraints
    add_asset_contraints(&mut problem, &variables, model, assets, year);

    // Solve problem
    let solution = problem.optimise(Sense::Minimise).solve();

    let status = solution.status();
    if status != HighsModelStatus::Optimal {
        // **TODO**: Make this a hard error once the problem is actually solvable
        error!("Could not solve: {status:?}");
    }

    Solution {
        variables,
        solution: solution.get_solution(),
    }
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
    info!("Adding variables to problem...");
    let mut variables = VariableMap::default();

    for asset in filter_assets(assets, year) {
        for flow in asset.process.flows.iter() {
            for time_slice in model.time_slice_info.iter_ids() {
                let coeff = calculate_cost_coefficient(asset, flow, year, time_slice);

                // var's value must be <= 0 for inputs and >= 0 for outputs
                let var = if flow.flow < 0.0 {
                    problem.add_column(coeff, ..=0.0)
                } else {
                    problem.add_column(coeff, 0.0..)
                };

                let key = VariableMapKey::new(
                    asset.id,
                    Rc::clone(&flow.commodity.id),
                    time_slice.clone(),
                );

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
        coeff += asset.process.parameter.variable_operating_cost
    }

    // If there is a user-provided commodity cost for this combination of parameters, include it
    if let Some(cost) = flow.commodity.costs.get(&asset.region_id, year, time_slice) {
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
    coeff.copysign(flow.flow)
}

/// Add asset-level constraints
fn add_asset_contraints(
    problem: &mut Problem,
    variables: &VariableMap,
    model: &Model,
    assets: &AssetPool,
    year: u32,
) {
    add_commodity_balance_constraints(problem, variables, model, assets, year);

    // **TODO**: Currently it's safe to assume all process flows are non-flexible, as we enforce
    // this when reading data in. Once we've added support for flexible process flows, we will
    // need to add different constraints for assets with flexible and non-flexible flows.
    //
    // See: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/360
    add_fixed_asset_constraints(problem, variables, assets, year, &model.time_slice_info);

    add_asset_capacity_constraints(problem, variables, model, assets, year);
}

/// Add asset-level input-output commodity balances
fn add_commodity_balance_constraints(
    _problem: &mut Problem,
    _variables: &VariableMap,
    _model: &Model,
    _assets: &AssetPool,
    _year: u32,
) {
    info!("Adding commodity balance constraints...");

    // Sanity check: we rely on the first n values of the dual row values corresponding to the
    // commodity constraints, so these must be the first rows
    assert!(
        _problem.num_rows() == 0,
        "Commodity balance constraints must be added before other constraints"
    );
}

/// Add constraints for non-flexible assets.
///
/// Non-flexible assets are those which have a fixed ratio between inputs and outputs.
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#non-flexible-assets
fn add_fixed_asset_constraints(
    problem: &mut Problem,
    variables: &VariableMap,
    assets: &AssetPool,
    year: u32,
    time_slice_info: &TimeSliceInfo,
) {
    info!("Adding constraints for non-flexible assets...");

    for asset in filter_assets(assets, year) {
        // Get first PAC. unwrap is safe because all processes have at least one PAC.
        let pac1 = asset.process.iter_pacs().next().unwrap();

        for time_slice in time_slice_info.iter_ids() {
            let pac_var = variables.get(asset.id, &pac1.commodity.id, time_slice);
            let pac_term = (pac_var, -1.0 / pac1.flow);

            for flow in asset.process.flows.iter() {
                // Don't add a constraint for the PAC itself
                if Rc::ptr_eq(&flow.commodity, &pac1.commodity) {
                    continue;
                }

                // We are enforcing that (var / flow) - (pac_var / pac_flow) = 0
                let var = variables.get(asset.id, &flow.commodity.id, time_slice);
                problem.add_row(0.0..=0.0, [(var, 1.0 / flow.flow), pac_term]);
            }
        }
    }
}

/// Add asset-level capacity and availability constraints
///
/// See description in [the dispatch optimisation documentation][1].
///
/// [1]: https://energysystemsmodellinglab.github.io/MUSE_2.0/dispatch_optimisation.html#asset-level-capacity-and-availability-constraints
fn add_asset_capacity_constraints(
    _problem: &mut Problem,
    _variables: &VariableMap,
    _model: &Model,
    _assets: &AssetPool,
    _year: u32,
) {
    info!("Adding asset-level capacity and availability constraints...");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityCost, CommodityCostMap, CommodityType, DemandMap};
    use crate::process::{FlowType, Process, ProcessCapacityMap, ProcessParameter};
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;
    use float_cmp::assert_approx_eq;
    use std::rc::Rc;

    fn get_cost_coeff_args(
        flow: f64,
        is_pac: bool,
        costs: CommodityCostMap,
    ) -> (Asset, ProcessFlow) {
        let process_param = ProcessParameter {
            process_id: "process1".into(),
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            cap2act: 1.0,
        };
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs,
            demand: DemandMap::new(),
        });
        let flow = ProcessFlow {
            process_id: "id1".into(),
            commodity: Rc::clone(&commodity),
            flow,
            flow_type: FlowType::Fixed,
            flow_cost: 1.0,
            is_pac,
        };
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            capacity_fractions: ProcessCapacityMap::new(),
            flows: vec![flow.clone()],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let asset = Asset {
            id: 0,
            agent_id: "agent1".into(),
            process: Rc::clone(&process),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };

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
        costs.insert("GBR".into(), 2010, time_slice.clone(), cost);
        check_coeff!(1.0, false, costs.clone(), 3.0);
        check_coeff!(-1.0, false, costs, -1.0);

        // not PAC, commodity cost for output and input
        let cost = CommodityCost {
            balance_type: BalanceType::Net,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert("GBR".into(), 2010, time_slice.clone(), cost);
        check_coeff!(1.0, false, costs.clone(), 3.0);
        check_coeff!(-1.0, false, costs, -3.0);

        // not PAC, commodity cost for input
        let cost = CommodityCost {
            balance_type: BalanceType::Consumption,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert("GBR".into(), 2010, time_slice.clone(), cost);
        check_coeff!(1.0, false, costs.clone(), 1.0);
        check_coeff!(-1.0, false, costs, -3.0);

        // PAC, commodity cost for output
        let cost = CommodityCost {
            balance_type: BalanceType::Production,
            value: 2.0,
        };
        let mut costs = CommodityCostMap::new();
        costs.insert("GBR".into(), 2010, time_slice.clone(), cost);
        check_coeff!(1.0, true, costs.clone(), 4.0);
        check_coeff!(-1.0, true, costs, -2.0);
    }
}
