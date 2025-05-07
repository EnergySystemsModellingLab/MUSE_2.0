//! Fixtures for tests

use crate::process::{Process, ProcessEnergyLimitsMap, ProcessMap, ProcessParameterMap};
use crate::region::RegionID;
use indexmap::indexmap;
use rstest::fixture;
use std::collections::HashSet;

#[fixture]
pub fn region_ids() -> HashSet<RegionID> {
    ["GBR".into(), "USA".into()].into_iter().collect()
}

#[fixture]
pub fn process(region_ids: HashSet<RegionID>) -> Process {
    Process {
        id: "process1".into(),
        description: "Description".into(),
        years: 2010..=2020,
        energy_limits: ProcessEnergyLimitsMap::new(),
        flows: vec![],
        parameters: ProcessParameterMap::new(),
        regions: region_ids,
    }
}

#[fixture]
pub fn processes(process: Process) -> ProcessMap {
    indexmap! { process.id.clone() => process.into()}
}
