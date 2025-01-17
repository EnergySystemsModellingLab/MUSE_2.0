//! Code for reading [Asset]s from a CSV file.
use crate::agent::Asset;
use crate::input::*;
use crate::process::Process;
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const ASSETS_FILE_NAME: &str = "assets.csv";

#[derive(Deserialize, PartialEq)]
struct AssetRaw {
    process_id: String,
    region_id: String,
    agent_id: String,
    capacity: f64,
    commission_year: u32,
}

/// Read assets CSV file from model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `agent_ids` - All possible process IDs
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
pub fn read_agent_assets(
    model_dir: &Path,
    agent_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Vec<Asset>>> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    let assets_csv = read_csv(&file_path)?;
    read_assets_from_iter(assets_csv, agent_ids, processes, region_ids)
        .with_context(|| input_err_msg(&file_path))
}

/// Process assets from an iterator.
///
/// # Arguments
///
/// * `iter` - Iterator of `AssetRaw`s
/// * `agent_ids` - All possible process IDs
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID or an error.
fn read_assets_from_iter<I>(
    iter: I,
    agent_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<HashMap<Rc<str>, Vec<Asset>>>
where
    I: Iterator<Item = AssetRaw>,
{
    iter.map(|asset| -> Result<_> {
        let agent_id = agent_ids.get_id(&asset.agent_id)?;
        let process = processes
            .get(asset.process_id.as_str())
            .with_context(|| format!("Invalid process ID: {}", &asset.process_id))?;
        let region_id = region_ids.get_id(&asset.region_id)?;
        ensure!(
            process.regions.contains(&region_id),
            "Region {} is not one of the regions in which process {} operates",
            region_id,
            process.id
        );

        Ok((
            Rc::clone(&agent_id),
            Asset {
                agent_id,
                process: Rc::clone(process),
                region_id,
                capacity: asset.capacity,
                commission_year: asset.commission_year,
            },
        ))
    })
    .process_results(|iter| iter.into_group_map())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::ProcessParameter;
    use crate::region::RegionSelection;
    use std::vec;

    #[test]
    fn test_read_assets_from_iter() {
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
        let processes = [(Rc::clone(&process.id), Rc::clone(&process))]
            .into_iter()
            .collect();
        let agent_ids = ["agent1".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "USA".into()].into_iter().collect();

        // Valid
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process1".into(),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        let asset_out = Asset {
            agent_id: "agent1".into(),
            process: Rc::clone(&process),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        let expected = [("agent1".into(), vec![asset_out])].into_iter().collect();
        assert!(
            read_assets_from_iter([asset_in].into_iter(), &agent_ids, &processes, &region_ids)
                .unwrap()
                == expected
        );

        // Bad process ID
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process2".into(),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        assert!(
            read_assets_from_iter([asset_in].into_iter(), &agent_ids, &processes, &region_ids)
                .is_err()
        );

        // Bad agent ID
        let asset_in = AssetRaw {
            agent_id: "agent2".into(),
            process_id: "process1".into(),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        assert!(
            read_assets_from_iter([asset_in].into_iter(), &agent_ids, &processes, &region_ids)
                .is_err()
        );

        // Bad region ID: not in region_ids
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process1".into(),
            region_id: "FRA".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        assert!(
            read_assets_from_iter([asset_in].into_iter(), &agent_ids, &processes, &region_ids)
                .is_err()
        );

        // Bad region ID: process not active there
        let process = Rc::new(Process {
            id: "process1".into(),
            description: "Description".into(),
            availabilities: vec![],
            flows: vec![],
            pacs: vec![],
            parameter: process_param,
            regions: RegionSelection::Some(["GBR".into()].into_iter().collect()),
        });
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process1".into(),
            region_id: "USA".into(), // NB: In region_ids, but not in process.regions
            capacity: 1.0,
            commission_year: 2010,
        };
        let processes = [(Rc::clone(&process.id), Rc::clone(&process))]
            .into_iter()
            .collect();
        assert!(
            read_assets_from_iter([asset_in].into_iter(), &agent_ids, &processes, &region_ids)
                .is_err()
        );
    }
}
