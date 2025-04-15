//! Code for reading process flows file
use super::super::*;
use crate::commodity::{CommodityID, CommodityMap};
use crate::id::IDCollection;
use crate::process::{FlowType, ProcessFlow, ProcessID};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessFlowRaw {
    process_id: String,
    commodity_id: String,
    flow: f64,
    #[serde(default)]
    flow_type: FlowType,
    flow_cost: Option<f64>,
    is_pac: bool,
}

/// Read process flows from a CSV file
pub fn read_process_flows(
    model_dir: &Path,
    process_ids: &HashSet<ProcessID>,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, Vec<ProcessFlow>>> {
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let process_flow_csv = read_csv(&file_path)?;
    read_process_flows_from_iter(process_flow_csv, process_ids, commodities)
        .with_context(|| input_err_msg(&file_path))
}

/// Read 'ProcessFlowRaw' records from an iterator and convert them into 'ProcessFlow' records.
fn read_process_flows_from_iter<I>(
    iter: I,
    process_ids: &HashSet<ProcessID>,
    commodities: &CommodityMap,
) -> Result<HashMap<ProcessID, Vec<ProcessFlow>>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    let mut flows = HashMap::new();
    for flow in iter {
        let commodity = commodities
            .get(flow.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &flow.commodity_id))?;

        ensure!(flow.flow != 0.0, "Flow cannot be zero");

        // Check that flow is not infinity, nan, etc.
        ensure!(
            flow.flow.is_normal(),
            "Invalid value for flow ({})",
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

        // Create ProcessFlow object
        let process_id = process_ids.get_id(&flow.process_id)?;
        let process_flow = ProcessFlow {
            process_id: flow.process_id,
            commodity: Rc::clone(commodity),
            flow: flow.flow,
            flow_type: flow.flow_type,
            flow_cost: flow.flow_cost.unwrap_or(0.0),
            is_pac: flow.is_pac,
        };

        // Insert into the map
        flows
            .entry(process_id)
            .or_insert_with(Vec::new)
            .push(process_flow);
    }

    validate_flows(&flows)?;
    validate_pac_flows(&flows)?;

    Ok(flows)
}

/// Validate that no process has multiple flows for the same commodity.
///
/// # Arguments
/// * `flows` - A map of process IDs to process flows
///
/// # Returns
/// An `Ok(())` if the check is successful, or an error.
fn validate_flows(flows: &HashMap<ProcessID, Vec<ProcessFlow>>) -> Result<()> {
    for (process_id, flows) in flows.iter() {
        let mut commodities: HashSet<CommodityID> = HashSet::new();

        for flow in flows.iter() {
            let commodity_id = &flow.commodity.id;
            ensure!(
                commodities.insert(commodity_id.clone()),
                "Process {process_id} has multiple flows for commodity {commodity_id}",
            );
        }
    }

    Ok(())
}

/// Validate that the PACs for each process are either all inputs or all outputs.
///
/// # Arguments
///
/// * `flows` - A map of process IDs to process flows
///
/// # Returns
/// An `Ok(())` if the check is successful, or an error.
fn validate_pac_flows(flows: &HashMap<ProcessID, Vec<ProcessFlow>>) -> Result<()> {
    for (process_id, flows) in flows.iter() {
        let mut flow_sign: Option<bool> = None; // False for inputs, true for outputs

        for flow in flows.iter().filter(|flow| flow.is_pac) {
            // Check that flow sign is consistent
            let current_flow_sign = flow.flow > 0.0;
            if let Some(flow_sign) = flow_sign {
                ensure!(
                    current_flow_sign == flow_sign,
                    "PACs for process {process_id} are a mix of inputs and outputs",
                );
            }
            flow_sign = Some(current_flow_sign);
        }

        ensure!(
            flow_sign.is_some(),
            "No PACs defined for process {process_id}"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::time_slice::TimeSliceLevel;
    use std::iter;

    #[test]
    fn test_read_process_flows_from_iter_good() {
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities: CommodityMap = ["commodity1", "commodity2"]
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

                (commodity.id.clone(), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: false,
            },
            ProcessFlowRaw {
                process_id: "id2".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
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
                        is_pac: true,
                    },
                    ProcessFlow {
                        process_id: "id1".into(),
                        commodity: commodities.get("commodity2").unwrap().clone(),
                        flow: 1.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                        is_pac: false,
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
                    is_pac: true,
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

                (commodity.id.clone(), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity3".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: false,
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
            demand: DemandMap::new(),
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
                    is_pac: true,
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
    fn test_read_process_flows_from_iter_bad_pacs() {
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

                (commodity.id.clone(), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
                flow: -1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
            },
        ];

        assert!(
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .is_err()
        );
    }

    #[test]
    fn test_read_process_flows_from_iter_no_pacs() {
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

                (commodity.id.clone(), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: false,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: false,
            },
        ];

        assert!(
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .is_err()
        );
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
            demand: DemandMap::new(),
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
                    is_pac: true,
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

    #[test]
    fn test_read_process_flows_from_iter_duplicate_flow() {
        let process_ids = iter::once("id1".into()).collect();
        let commodities = ["commodity1"]
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

                (commodity.id.clone(), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: true,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: Some(1.0),
                is_pac: false,
            },
        ];

        assert!(
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .is_err()
        );
    }
}
