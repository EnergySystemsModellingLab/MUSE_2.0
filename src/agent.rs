//! Agents drive the economy of the MUSE 2.0 simulation, through relative investment in different
//! assets.
use crate::commodity::Commodity;
use crate::process::Process;
use crate::region::RegionSelection;
use crate::time_slice::TimeSliceID;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashSet;
use std::ops::RangeInclusive;
use std::rc::Rc;

/// An agent in the simulation
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    /// A unique identifier for the agent.
    pub id: Rc<str>,
    /// A text description of the agent.
    pub description: String,
    /// The commodity that the agent produces (could be a service demand too).
    pub commodity: Rc<Commodity>,
    /// The proportion of the commodity production that the agent is responsible for.
    pub commodity_portion: f64,
    /// The processes that the agent will consider investing in.
    pub search_space: SearchSpace,
    /// The decision rule that the agent uses to decide investment.
    pub decision_rule: DecisionRule,
    /// The maximum capital cost the agent will pay.
    pub capex_limit: Option<f64>,
    /// The maximum annual operating cost (fuel plus var_opex etc) that the agent will pay.
    pub annual_cost_limit: Option<f64>,
    /// The regions in which this agent operates.
    pub regions: RegionSelection,
    /// The agent's objectives.
    pub objectives: Vec<AgentObjective>,
}

/// Which processes apply to this agent
#[derive(Debug, Clone, PartialEq)]
pub enum SearchSpace {
    /// All processes are considered
    AllProcesses,
    /// Only these specific processes are considered
    Some(HashSet<Rc<str>>),
}

/// The decision rule for a particular objective
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum DecisionRule {
    /// Used when there is only a single objective
    #[string = "single"]
    Single,
    /// A simple weighting of objectives
    #[string = "weighted"]
    Weighted,
    /// Objectives are considered in a specific order
    #[string = "lexico"]
    Lexicographical,
}

/// An objective for an agent with associated parameters
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct AgentObjective {
    /// Unique agent id identifying the agent this objective belongs to
    pub agent_id: String,
    /// Acronym identifying the objective (e.g. LCOX)
    pub objective_type: ObjectiveType,
    /// For the weighted sum objective, the set of weights to apply to each objective.
    pub decision_weight: Option<f64>,
    /// The tolerance around the main objective to consider secondary objectives. This is an absolute value of maximum deviation in the units of the main objective.
    pub decision_lexico_tolerance: Option<f64>,
}

/// The type of objective for the agent
///
/// **TODO** Add more objective types
#[derive(Debug, Clone, PartialEq, DeserializeLabeledStringEnum)]
pub enum ObjectiveType {
    /// Average cost of one unit of output commodity over its lifetime
    #[string = "lcox"]
    LevelisedCostOfX,
    /// Cost of serving agent's demand for a year, considering the asset's entire lifetime
    #[string = "eac"]
    EquivalentAnnualCost,
}

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// A unique identifier for the asset
    pub id: u32,
    /// A unique identifier for the agent
    pub agent_id: Rc<str>,
    /// The [`Process`] that this asset corresponds to
    pub process: Rc<Process>,
    /// The region in which the asset is located
    pub region_id: Rc<str>,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
}

impl Asset {
    /// The last year in which this asset should be decommissioned
    pub fn decommission_year(&self) -> u32 {
        self.commission_year + self.process.parameter.lifetime
    }

    /// Get the activity limits for this asset in a particular time slice
    pub fn get_activity_limits(&self, time_slice: &TimeSliceID) -> RangeInclusive<f64> {
        let limits = self.process.capacity_fractions.get(time_slice).unwrap();
        let capacity_a = self.capacity * self.process.parameter.cap2act;

        // Multiply the fractional capacity in self.process by this asset's actual capacity
        (capacity_a * limits.start())..=(capacity_a * limits.end())
    }
}

/// A pool of [`Asset`]s
pub struct AssetPool {
    /// The pool of assets, both active and yet to be commissioned.
    ///
    /// Sorted in order of commission year.
    assets: Vec<Asset>,
    /// Current milestone year.
    current_year: u32,
}

impl AssetPool {
    /// Create a new [`AssetPool`]
    pub fn new(mut assets: Vec<Asset>) -> Self {
        // Sort in order of commission year
        assets.sort_by(|a, b| a.commission_year.cmp(&b.commission_year));

        Self {
            assets,
            current_year: 0,
        }
    }

    /// Commission new assets for the specified milestone year
    pub fn commission_new(&mut self, year: u32) {
        assert!(
            year >= self.current_year,
            "Assets have already been commissioned for year {year}"
        );
        self.current_year = year;
    }

    /// Iterate over active assets
    pub fn iter(&self) -> impl Iterator<Item = &Asset> {
        self.assets
            .iter()
            .take_while(|asset| asset.commission_year <= self.current_year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{CommodityCostMap, CommodityType, DemandMap};
    use crate::process::{FlowType, Process, ProcessCapacityMap, ProcessFlow, ProcessParameter};
    use crate::time_slice::TimeSliceLevel;
    use itertools::{assert_equal, Itertools};
    use std::iter;

    #[test]
    fn test_asset_get_activity_limits() {
        let time_slice = TimeSliceID {
            season: "winter".into(),
            time_of_day: "day".into(),
        };
        let process_param = ProcessParameter {
            process_id: "process1".into(),
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            cap2act: 3.0,
        };
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let flow = ProcessFlow {
            process_id: "id1".into(),
            commodity: Rc::clone(&commodity),
            flow: 1.0,
            flow_type: FlowType::Fixed,
            flow_cost: 1.0,
            is_pac: true,
        };
        let fraction_limits = 1.0..=f64::INFINITY;
        let capacity_fractions = iter::once((time_slice.clone(), fraction_limits)).collect();
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            capacity_fractions,
            flows: vec![flow.clone()],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let asset = Asset {
            id: 0,
            agent_id: "agent1".into(),
            process: Rc::clone(&process),
            region_id: "GBR".into(),
            capacity: 2.0,
            commission_year: 2010,
        };

        assert_eq!(asset.get_activity_limits(&time_slice), 6.0..=f64::INFINITY);
    }

    fn create_asset_pool() -> AssetPool {
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
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            capacity_fractions: ProcessCapacityMap::new(),
            flows: vec![],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let future = [2020, 2010]
            .map(|year| Asset {
                id: 0,
                agent_id: "agent1".into(),
                process: Rc::clone(&process),
                region_id: "GBR".into(),
                capacity: 1.0,
                commission_year: year,
            })
            .into_iter()
            .collect_vec();

        AssetPool::new(future)
    }

    #[test]
    fn test_asset_pool_new() {
        let assets = create_asset_pool();
        assert!(assets.current_year == 0);

        // Should be in order of commission year
        assert!(assets.assets.len() == 2);
        assert!(assets.assets[0].commission_year == 2010);
        assert!(assets.assets[1].commission_year == 2020);
    }

    #[test]
    fn test_asset_pool_commission_new() {
        // Asset to be commissioned in this year
        let mut assets = create_asset_pool();
        assets.commission_new(2010);
        assert!(assets.current_year == 2010);
        assert_equal(assets.iter(), iter::once(&assets.assets[0]));

        // Commission year has passed
        let mut assets = create_asset_pool();
        assets.commission_new(2011);
        assert!(assets.current_year == 2011);
        assert_equal(assets.iter(), iter::once(&assets.assets[0]));

        // Nothing to commission for this year
        let mut assets = create_asset_pool();
        assets.commission_new(2000);
        assert!(assets.current_year == 2000);
        assert!(assets.iter().next().is_none()); // no active assets
    }
}
