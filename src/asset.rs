//! Code for working with [Asset]s.
//!
//! For a description of what assets are, please see the glossary.
use crate::input::*;
use crate::process::Process;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
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

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// The [Process] that this asset corresponds to
    pub process: Rc<Process>,
    /// The region in which the asset is located
    pub region_id: String,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
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
) -> Result<HashMap<Rc<str>, Vec<Asset>>, Box<dyn Error>>
where
    I: Iterator<Item = AssetRaw>,
{
    let map: HashMap<Rc<str>, _> = iter
        .map(|asset| -> Result<_, Box<dyn Error>> {
            let process = processes
                .get(asset.process_id.as_str())
                .ok_or(format!("Invalid process ID: {}", &asset.process_id))?;

            Ok((
                asset.agent_id.into(),
                Asset {
                    process: Rc::clone(process),
                    region_id: asset.region_id,
                    capacity: asset.capacity,
                    commission_year: asset.commission_year,
                },
            ))
        })
        .process_results(|iter| iter.into_group_map())?;

    for agent_id in map.keys() {
        if !agent_ids.contains(agent_id) {
            Err(format!("Invalid agent ID: {}", agent_id))?;
        }
    }

    for asset in map.values().flatten() {
        if !region_ids.contains(asset.region_id.as_str()) {
            Err(format!("Invalid region ID: {}", asset.region_id))?;
        }
    }

    Ok(map)
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
pub fn read_assets(
    model_dir: &Path,
    agent_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, Vec<Asset>> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    read_assets_from_iter(read_csv(&file_path), agent_ids, processes, region_ids)
        .unwrap_input_err(&file_path)
}

#[cfg(test)]
mod tests {
    use std::vec;

    use crate::process::ProcessParameter;

    use super::*;

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
            parameter: process_param,
            regions: crate::region::RegionSelection::All,
        });
        let processes = [(Rc::clone(&process.id), Rc::clone(&process))]
            .into_iter()
            .collect();
        let agent_ids = ["agent1".into()].into_iter().collect();
        let region_ids = ["GBR".into()].into_iter().collect();

        // Valid
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process1".into(),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        let asset_out = Asset {
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

        // Bad region ID
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
    }
}
