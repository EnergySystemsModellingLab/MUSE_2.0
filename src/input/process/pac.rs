//! Code for reading Primary Activity Commodities (PACs) file
use super::define_process_id_getter;
use crate::commodity::Commodity;
use crate::input::*;
use crate::process::ProcessFlow;
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_PACS_FILE_NAME: &str = "process_pacs.csv";

/// Primary Activity Commodity
#[derive(PartialEq, Clone, Eq, Hash, Debug, Deserialize)]
struct ProcessPAC {
    process_id: String,
    commodity_id: String,
}
define_process_id_getter! {ProcessPAC}

/// Read process Primary Activity Commodities (PACs) from the specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - All possible process IDs
/// * `commodities` - Commodities for the model
pub fn read_process_pacs(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<HashMap<Rc<str>, Vec<Rc<Commodity>>>> {
    let file_path = model_dir.join(PROCESS_PACS_FILE_NAME);
    let process_pacs_csv = read_csv(&file_path)?;
    read_process_pacs_from_iter(process_pacs_csv, process_ids, commodities, flows)
        .with_context(|| input_err_msg(&file_path))
}

/// Read process Primary Activity Commodities (PACs) from an iterator.
///
/// # Arguments
///
/// * `iter` - An iterator of `ProcessPAC`s
/// * `process_ids` - All possible process IDs
/// * `commodities` - Commodities for the model
///
/// # Returns
///
/// A `HashMap` with process IDs as keys and `Vec`s of commodities as values or an error.
fn read_process_pacs_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<HashMap<Rc<str>, Vec<Rc<Commodity>>>>
where
    I: Iterator<Item = ProcessPAC>,
{
    // Keep track of previous PACs so we can check for duplicates
    let mut existing_pacs = HashSet::new();

    // Build hashmap of process ID to PAC commodities
    let pacs = iter
        .map(|pac| {
            let process_id = process_ids.get_id(&pac.process_id)?;
            let commodity = commodities
                .get(pac.commodity_id.as_str())
                .with_context(|| format!("{} is not a valid commodity ID", &pac.commodity_id))?;

            // Check that commodity is valid and PAC is not a duplicate
            ensure!(existing_pacs.insert(pac), "Duplicate PACs found");
            Ok((process_id, Rc::clone(commodity)))
        })
        .process_results(|iter| iter.into_group_map())?;

    // Check that PACs for each process are either all inputs or all outputs
    validate_pac_flows(&pacs, flows)?;

    // Return result
    Ok(pacs)
}

/// Validate that the PACs for each process are either all inputs or all outputs.
///
/// # Arguments
///
/// * `pacs` - A map of process IDs to PAC commodities
/// * `flows` - A map of process IDs to process flows
///
/// # Returns
/// An `Ok(())` if the check is successful, or an error.
fn validate_pac_flows(
    pacs: &HashMap<Rc<str>, Vec<Rc<Commodity>>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<()> {
    for (process_id, pacs) in pacs.iter() {
        // Get the flows for the process (unwrap is safe as every process has associated flows)
        let flows = flows.get(process_id).unwrap();

        let mut flow_sign: Option<bool> = None; // False for inputs, true for outputs
        for pac in pacs.iter() {
            // Find the flow associated with the PAC
            let flow = flows
                .iter()
                .find(|item| *item.commodity.id == *pac.id)
                .with_context(|| {
                    format!(
                        "PAC {} for process {} must have an associated flow",
                        pac.id, process_id
                    )
                })?;

            // Check that flow sign is consistent
            let current_flow_sign = flow.flow > 0.0;
            if let Some(flow_sign) = flow_sign {
                ensure!(
                    current_flow_sign == flow_sign,
                    "PACs for process {} are a mix of inputs and outputs",
                    process_id
                );
            }
            flow_sign = Some(current_flow_sign);
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::commodity::{CommodityCostMap, CommodityType};
    use crate::demand::DemandMap;
    use crate::process::FlowType;
    use crate::time_slice::TimeSliceLevel;

    #[test]
    fn test_read_process_pacs_from_iter() {
        // Prepare test data
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
        let flows: HashMap<Rc<str>, Vec<ProcessFlow>> = ["id1", "id2"]
            .into_iter()
            .map(|process_id| {
                (
                    process_id.into(),
                    ["commodity1", "commodity2"]
                        .into_iter()
                        .map(|commodity_id| ProcessFlow {
                            process_id: process_id.into(),
                            commodity: commodities.get(commodity_id).unwrap().clone(),
                            flow: 1.0,
                            flow_type: FlowType::Fixed,
                            flow_cost: 1.0,
                        })
                        .collect(),
                )
            })
            .collect();

        // duplicate PAC
        let pac = ProcessPAC {
            process_id: "id1".into(),
            commodity_id: "commodity1".into(),
        };
        let pacs = [pac.clone(), pac];
        assert!(
            read_process_pacs_from_iter(pacs.into_iter(), &process_ids, &commodities, &flows)
                .is_err()
        );

        // invalid commodity ID
        let bad_pac = ProcessPAC {
            process_id: "id1".into(),
            commodity_id: "other_commodity".into(),
        };
        assert!(read_process_pacs_from_iter(
            [bad_pac].into_iter(),
            &process_ids,
            &commodities,
            &flows
        )
        .is_err());

        // Valid
        let pacs = [
            ProcessPAC {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
            },
            ProcessPAC {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
            },
            ProcessPAC {
                process_id: "id2".into(),
                commodity_id: "commodity1".into(),
            },
        ];
        let expected = [
            (
                "id1".into(),
                [
                    commodities.get("commodity1").unwrap(),
                    commodities.get("commodity2").unwrap(),
                ]
                .into_iter()
                .cloned()
                .collect(),
            ),
            (
                "id2".into(),
                [commodities.get("commodity1").unwrap()]
                    .into_iter()
                    .cloned()
                    .collect(),
            ),
        ]
        .into_iter()
        .collect();
        assert!(
            read_process_pacs_from_iter(
                pacs.clone().into_iter(),
                &process_ids,
                &commodities,
                &flows
            )
            .unwrap()
                == expected
        );

        // Invalid flows
        // Making commodity1 an input so the PACs for process id1 are a mix of inputs and outputs
        let mut flows = flows.clone();
        flows
            .get_mut(&Rc::from("id1"))
            .unwrap()
            .iter_mut()
            .find(|flow| flow.commodity.id == "commodity1".into())
            .unwrap()
            .flow = -1.0;
        assert!(
            read_process_pacs_from_iter(pacs.into_iter(), &process_ids, &commodities, &flows)
                .is_err()
        );
    }
}
