//! Fixtures for tests

use crate::process::{
    Process, ProcessEnergyLimitsMap, ProcessFlowsMap, ProcessMap, ProcessParameter,
    ProcessParameterMap,
};
use crate::region::RegionID;
use indexmap::indexmap;
use itertools::Itertools;
use rstest::fixture;
use std::collections::HashSet;
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
pub fn region_ids() -> HashSet<RegionID> {
    ["GBR".into(), "USA".into()].into_iter().collect()
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
        energy_limits: ProcessEnergyLimitsMap::new(),
        flows: ProcessFlowsMap::new(),
        parameters: process_parameter_map,
        regions: region_ids,
    }
}

#[fixture]
pub fn processes(process: Process) -> ProcessMap {
    indexmap! { process.id.clone() => process.into()}
}
