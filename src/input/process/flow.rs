//! Code for reading process flows file
use super::define_process_id_getter;
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::{FlowType, ProcessFlow};
use anyhow::{Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";

define_process_id_getter! {ProcessFlow}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessFlowRaw {
    process_id: String,
    commodity_id: String,
    flow: f64,
    #[serde(default)]
    flow_type: FlowType,
    flow_cost: Option<f64>,
}
define_process_id_getter! {ProcessFlowRaw}

/// Read process flows from a CSV file
pub fn read_process_flows(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
) -> Result<HashMap<Rc<str>, Vec<ProcessFlow>>> {
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let process_flow_csv = read_csv(&file_path)?;
    read_process_flows_from_iter(process_flow_csv, process_ids, commodities)
        .with_context(|| input_err_msg(&file_path))
}

/// Read 'ProcessFlowRaw' records from an iterator and convert them into 'ProcessFlow' records.
fn read_process_flows_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
) -> Result<HashMap<Rc<str>, Vec<ProcessFlow>>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    iter.map(|flow_raw| -> Result<ProcessFlow> {
        let commodity = commodities
            .get(flow_raw.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &flow_raw.commodity_id))?;

        Ok(ProcessFlow {
            process_id: flow_raw.process_id,
            commodity: Rc::clone(commodity),
            flow: flow_raw.flow,
            flow_type: flow_raw.flow_type,
            flow_cost: flow_raw.flow_cost.unwrap_or(0.0),
        })
    })
    .process_results(|iter| iter.into_id_map(process_ids))?
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::commodity::{CommodityCostMap, CommodityType, DemandMap};
    use crate::time_slice::TimeSliceLevel;

    #[test]
    fn test_read_process_flows_from_iter_good() {
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities: HashMap<Rc<str>, Rc<Commodity>> = ["commodity1", "commodity2"]
            .into_iter()
            .map(|id| {
                let commodity = Commodity {
                    id: id.into(),
                    description: "Some description".into(),
                    kind: CommodityType::InputCommodity,
                    time_slice_level: TimeSliceLevel::Annual,
                    costs: CommodityCostMap::new(),
                    demand: DemandMap::new(),
                };

                (Rc::clone(&commodity.id), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
            },
            ProcessFlowRaw {
                process_id: "id2".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
            },
        ];

        let expected = HashMap::from([
            (
                "id1".into(),
                vec![
                    ProcessFlow {
                        process_id: "id1".into(),
                        commodity: commodities.get("commodity1").unwrap().clone(),
                        flow: 1.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                    },
                    ProcessFlow {
                        process_id: "id1".into(),
                        commodity: commodities.get("commodity2").unwrap().clone(),
                        flow: 1.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                    },
                ],
            ),
            (
                "id2".into(),
                vec![ProcessFlow {
                    process_id: "id2".into(),
                    commodity: commodities.get("commodity1").unwrap().clone(),
                    flow: 1.0,
                    flow_type: FlowType::Fixed,
                    flow_cost: 1.0,
                }],
            ),
        ]);

        let actual =
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_read_process_flows_from_iter_bad_commodity_id() {
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities = ["commodity1", "commodity2"]
            .into_iter()
            .map(|id| {
                let commodity = Commodity {
                    id: id.into(),
                    description: "Some description".into(),
                    kind: CommodityType::InputCommodity,
                    time_slice_level: TimeSliceLevel::Annual,
                    costs: CommodityCostMap::new(),
                    demand: DemandMap::new(),
                };

                (Rc::clone(&commodity.id), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity3".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
            },
        ];

        assert!(
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .is_err()
        );
    }
}
