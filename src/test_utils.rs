use crate::agent::{Agent, AgentCommodity, AgentObjective, AgentSearchSpace, DecisionRule};
use crate::commodity::Commodity;
use crate::process::{FlowType, Process, ProcessFlow, ProcessParameter};
use crate::region::RegionSelection;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::rc::Rc;

impl Default for Agent {
    fn default() -> Self {
        Self {
            id: Rc::from("agent1"),
            description: "An agent".into(),
            commodities: Vec::new(),
            search_space: Vec::new(),
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        }
    }
}

impl Default for Commodity {
    fn default() -> Self {
        Self {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: crate::commodity::CommodityType::SupplyEqualsDemand,
            time_slice_level: crate::time_slice::TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        }
    }
}

impl Default for Process {
    fn default() -> Self {
        Self {
            id: "process1".into(),
            description: "Description".into(),
            activity_limits: ActivityLimitsMap::new(),
            flows: vec![],
            parameter: ProcessParameter::default(),
            regions: RegionSelection::default(),
        }
    }
}

impl Default for ProcessParameter {
    fn default() -> Self {
        Self {
            process_id: "process1".into(),
            years: 2010..=2020,
            capital_cost: 5.0,
            fixed_operating_cost: 2.0,
            variable_operating_cost: 1.0,
            lifetime: 5,
            discount_rate: 0.9,
            capacity_to_activity: 1.0,
        }
    }
}
