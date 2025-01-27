//! Agents drive the economy of the MUSE 2.0 simulation, through relative investment in different
//! assets.
use crate::commodity::Commodity;
use crate::process::Process;
use crate::region::RegionSelection;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashSet, VecDeque};
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

/// A unique identifier for an asset
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AssetID(u32);

impl AssetID {
    /// Sentinel value indicating that the asset is not active
    pub const INVALID: AssetID = AssetID(u32::MAX);
}

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// A unique identifier for the asset
    pub id: AssetID,
    /// The [`Agent`] which owns this asset
    pub agent: Rc<Agent>,
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
    /// Create a new [`Asset`].
    ///
    /// The `id` field is initially set to [`AssetID::INVALID`], but is changed to a unique value
    /// when the asset is commissioned.
    pub fn new(
        agent: Rc<Agent>,
        process: Rc<Process>,
        region_id: Rc<str>,
        capacity: f64,
        commission_year: u32,
    ) -> Self {
        Self {
            id: AssetID::INVALID,
            agent,
            process,
            region_id,
            capacity,
            commission_year,
        }
    }

    /// The last year in which this asset should be decommissioned
    pub fn decommission_year(&self) -> u32 {
        self.commission_year + self.process.parameter.lifetime
    }
}

/// A pool of [`Asset`]s
pub struct AssetPool {
    /// The pool of assets yet to be commissioned sorted by commission year
    future: VecDeque<Asset>,
    /// The pool of active assets
    active: Vec<Asset>,
    /// Internal counter to ensure asset IDs are unique
    id_count: u32,
}

impl AssetPool {
    /// Create a new [`AssetPool`]
    pub fn new(mut assets: Vec<Asset>) -> Self {
        // Sort in order of commission year
        assets.sort_by(|a, b| a.commission_year.cmp(&b.commission_year));

        Self {
            future: assets.into(),
            active: Vec::new(),
            id_count: 0,
        }
    }

    /// Commission new assets for the specified milestone year
    pub fn commission_new(&mut self, year: u32) {
        // Count the number of assets in `future` which come online on or before `year`
        let count = self
            .future
            .iter()
            .take_while(|asset| asset.commission_year <= year)
            .count();

        // Move these assets out of `future` and give each a unique ID
        let new_assets = self.future.drain(0..count).map(|mut asset| {
            asset.id = AssetID(self.id_count);
            self.id_count += 1;
            asset
        });

        // Put new assets into `active`
        self.active.extend(new_assets);
    }

    /// Decommission old assets for the specified milestone year
    pub fn decomission_old(&mut self, year: u32) {
        self.active
            .retain(|asset| asset.decommission_year() <= year);
    }

    /// Iterate over active assets
    pub fn iter(&self) -> impl Iterator<Item = &Asset> {
        self.active.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{CommodityCostMap, CommodityType, DemandMap};
    use crate::process::ProcessParameter;
    use crate::time_slice::TimeSliceLevel;
    use itertools::Itertools;

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
            availabilities: vec![],
            flows: vec![],
            pacs: vec![],
            parameter: process_param.clone(),
            regions: RegionSelection::All,
        });
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let agent = Rc::new(Agent {
            id: "agent1".into(),
            description: "".into(),
            commodity,
            commodity_portion: 1.0,
            search_space: SearchSpace::AllProcesses,
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        });
        let future = [2010, 2020]
            .map(|year| {
                Asset::new(
                    Rc::clone(&agent),
                    Rc::clone(&process),
                    "GBR".into(),
                    1.0,
                    year,
                )
            })
            .into_iter()
            .collect_vec();

        AssetPool::new(future)
    }

    #[test]
    fn test_asset_pool_new() {
        let assets = create_asset_pool();
        assert!(assets.future.len() == 2);
        assert!(assets.active.is_empty());
    }

    #[test]
    fn test_asset_pool_commission_new() {
        // Asset to be commissioned in this year
        let mut assets = create_asset_pool();
        assets.commission_new(2010);
        assert!(assets.future.len() == 1);
        assert!(assets.future[0].commission_year == 2020);
        assert!(assets.active.len() == 1);
        assert!(assets.active[0].commission_year == 2010);

        // Commission year has passed
        let mut assets = create_asset_pool();
        assets.commission_new(2011);
        assert!(assets.future.len() == 1);
        assert!(assets.future[0].commission_year == 2020);
        assert!(assets.active.len() == 1);
        assert!(assets.active[0].commission_year == 2010);

        // Nothing to commission for this year
        let mut assets = create_asset_pool();
        assets.commission_new(2000);
        assert!(assets.future.len() == 2);
        assert!(assets.active.is_empty());

        // Check that assets are given unique IDs
        let mut assets = create_asset_pool();
        assets.commission_new(2020);
        assert!(assets.future.is_empty());
        assert!(assets.active.len() == 2);
        assert!(assets.active[0].id == AssetID(0));
        assert!(assets.active[1].id == AssetID(1));
    }

    #[test]
    fn test_asset_pool_decommission_old() {
        let mut assets = create_asset_pool();
        assets.commission_new(2020);
        assert!(assets.active.len() == 2);
        assets.decomission_old(2020); // should decommission first asset (lifetime == 5)
        assert!(assets.active.len() == 1);
        assets.decomission_old(2022); // nothing to decommission
        assert!(assets.active.len() == 1);
        assets.decomission_old(2025); // should decommission second asset
        assert!(assets.active.len() == 1);
    }
}
