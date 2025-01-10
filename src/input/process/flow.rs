//! Code for reading process flows file
use super::define_process_id_getter;
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::{FlowType, ProcessFlow};
use anyhow::{ensure, Context, Result};
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
    iter.map(|flow| -> Result<ProcessFlow> {
        let commodity = commodities
            .get(flow.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &flow.commodity_id))?;

        // Check that flow is not zero, infinity, etc.
        ensure!(
            flow.flow.is_normal(),
            "Invalid value for flow: {}",
            flow.flow
        );

        // **TODO**: https://github.com/EnergySystemsModellingLab/MUSE_2.0/issues/300
        ensure!(
            flow.flow_type == FlowType::Fixed,
            "Commodity flexible assets are not currently supported"
        );

        if let Some(flow_cost) = flow.flow_cost {
            ensure!(
                (0.0..f64::INFINITY).contains(&flow_cost),
                "Invalid value for flow cost ({flow_cost}). Must be >=0."
            )
        }

        Ok(ProcessFlow {
            process_id: flow.process_id,
            commodity: Rc::clone(commodity),
            flow: flow.flow,
            flow_type: flow.flow_type,
            flow_cost: flow.flow_cost.unwrap_or(0.0),
        })
    })
    .process_results(|iter| iter.into_id_map(process_ids))?
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::commodity::{CommodityCostMap, CommodityType};
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

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
                    demand_by_region: HashMap::new(),
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
                    demand_by_region: HashMap::new(),
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

    #[test]
    fn test_read_process_flows_from_iter_bad_flow() {
        let process_ids = iter::once("id1".into()).collect();
        let commodities = iter::once(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand_by_region: HashMap::new(),
        })
        .map(|c| (c.id.clone(), Rc::new(c)))
        .collect();

        macro_rules! check_bad_flow {
            ($flow:expr) => {
                let flow = ProcessFlowRaw {
                    process_id: "id1".into(),
                    commodity_id: "commodity1".into(),
                    flow: $flow,
                    flow_type: FlowType::Fixed,
                    flow_cost: Some(1.0),
                };
                assert!(
                    read_process_flows_from_iter(iter::once(flow), &process_ids, &commodities)
                        .is_err()
                );
            };
        }

        check_bad_flow!(0.0);
        check_bad_flow!(f64::NEG_INFINITY);
        check_bad_flow!(f64::INFINITY);
        check_bad_flow!(f64::NAN);
    }

    #[test]
    fn test_read_process_flows_from_iter_flow_cost() {
        let process_ids = iter::once("id1".into()).collect();
        let commodities = iter::once(Commodity {
            id: "commodity1".into(),
            description: "Some description".into(),
            kind: CommodityType::InputCommodity,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand_by_region: HashMap::new(),
        })
        .map(|c| (c.id.clone(), Rc::new(c)))
        .collect();

        macro_rules! is_flow_cost_ok {
            ($flow_cost:expr) => {{
                let flow = ProcessFlowRaw {
                    process_id: "id1".into(),
                    commodity_id: "commodity1".into(),
                    flow: 1.0,
                    flow_type: FlowType::Fixed,
                    flow_cost: Some($flow_cost),
                };

                read_process_flows_from_iter(iter::once(flow), &process_ids, &commodities).is_ok()
            }};
        }

        assert!(is_flow_cost_ok!(0.0));
        assert!(is_flow_cost_ok!(1.0));
        assert!(is_flow_cost_ok!(100.0));
        assert!(!is_flow_cost_ok!(f64::NEG_INFINITY));
        assert!(!is_flow_cost_ok!(f64::INFINITY));
        assert!(!is_flow_cost_ok!(f64::NAN));
    }
}
