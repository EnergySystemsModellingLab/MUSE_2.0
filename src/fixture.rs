//! Fixtures for tests

use crate::agent::{
    Agent, AgentCommodityPortionsMap, AgentCostLimitsMap, AgentID, AgentMap, AgentObjectiveMap,
    AgentSearchSpaceMap, DecisionRule,
};
use crate::asset::{Asset, AssetPool};
use crate::commodity::{Commodity, CommodityID, CommodityLevyMap, CommodityType, DemandMap};
use crate::process::{
    Process, ProcessActivityLimitsMap, ProcessFlowsMap, ProcessMap, ProcessParameter,
    ProcessParameterMap,
};
use crate::region::RegionID;
use crate::time_slice::{TimeSliceID, TimeSliceInfo, TimeSliceLevel};
use indexmap::indexmap;
use itertools::Itertools;
use rstest::fixture;
use std::collections::{HashMap, HashSet};
use std::iter;
use std::rc::Rc;

/// Assert that an error with the given message occurs
macro_rules! assert_error {
    ($result:expr, $msg:expr) => {
        assert_eq!(
            $result.unwrap_err().chain().next().unwrap().to_string(),
            $msg
        );
    };
}
pub(crate) use assert_error;

#[fixture]
pub fn region_id() -> RegionID {
    "GBR".into()
}

#[fixture]
pub fn commodity_ids() -> HashSet<CommodityID> {
    iter::once("commodity1".into()).collect()
}

#[fixture]
pub fn region_ids() -> HashSet<RegionID> {
    ["GBR".into(), "USA".into()].into_iter().collect()
}

#[fixture]
pub fn agent_id() -> AgentID {
    "agent1".into()
}

#[fixture]
pub fn commodity_id() -> CommodityID {
    "commodity1".into()
}

#[fixture]
pub fn svd_commodity() -> Commodity {
    Commodity {
        id: "commodity1".into(),
        description: "".into(),
        kind: CommodityType::ServiceDemand,
        time_slice_level: TimeSliceLevel::DayNight,
        levies: CommodityLevyMap::new(),
        demand: DemandMap::new(),
    }
}

pub fn get_svd_map(commodity: &Commodity) -> HashMap<CommodityID, &Commodity> {
    iter::once((commodity.id.clone(), commodity)).collect()
}

#[fixture]
pub fn asset(process: Process) -> Asset {
    let region_id: RegionID = "GBR".into();
    let agent_id = "agent1".into();
    let commission_year = 2015;
    Asset::new(agent_id, process.into(), region_id, 2.0, commission_year).unwrap()
}

#[fixture]
pub fn assets(asset: Asset) -> AssetPool {
    let year = asset.commission_year;
    let mut assets = AssetPool::new(iter::once(asset).collect());
    assets.commission_new(year);
    assets
}

#[fixture]
pub fn process_parameter_map(region_ids: HashSet<RegionID>) -> ProcessParameterMap {
    let parameter = Rc::new(ProcessParameter {
        capital_cost: 0.0,
        fixed_operating_cost: 0.0,
        variable_operating_cost: 0.0,
        lifetime: 1,
        discount_rate: 1.0,
        capacity_to_activity: 0.0,
    });

    region_ids
        .into_iter()
        .cartesian_product(2010..=2020)
        .map(|(region_id, year)| ((region_id, year), parameter.clone()))
        .collect()
}

#[fixture]
pub fn process(
    region_ids: HashSet<RegionID>,
    process_parameter_map: ProcessParameterMap,
) -> Process {
    Process {
        id: "process1".into(),
        description: "Description".into(),
        years: vec![2010, 2020],
        activity_limits: ProcessActivityLimitsMap::new(),
        flows: ProcessFlowsMap::new(),
        parameters: process_parameter_map,
        regions: region_ids,
    }
}

#[fixture]
pub fn processes(process: Process) -> ProcessMap {
    indexmap! { process.id.clone() => process.into()}
}

#[fixture]
pub fn agents() -> AgentMap {
    iter::once((
        "agent1".into(),
        Agent {
            id: "agent1".into(),
            description: "".into(),
            commodity_portions: AgentCommodityPortionsMap::new(),
            search_space: AgentSearchSpaceMap::new(),
            decision_rule: DecisionRule::Single,
            cost_limits: AgentCostLimitsMap::new(),
            regions: HashSet::new(),
            objectives: AgentObjectiveMap::new(),
        },
    ))
    .collect()
}

#[fixture]
pub fn time_slice() -> TimeSliceID {
    TimeSliceID {
        season: "winter".into(),
        time_of_day: "day".into(),
    }
}

#[fixture]
pub fn time_slice_info() -> TimeSliceInfo {
    TimeSliceInfo {
        times_of_day: iter::once("day".into()).collect(),
        seasons: iter::once(("winter".into(), 1.0)).collect(),
        time_slices: [(
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            1.0,
        )]
        .into_iter()
        .collect(),
    }
}
