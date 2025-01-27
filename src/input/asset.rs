//! Code for reading [Asset]s from a CSV file.
use crate::agent::{Agent, Asset};
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
/// * `agents` - All agents
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A `HashMap` containing assets grouped by agent ID.
pub fn read_assets(
    model_dir: &Path,
    agents: &HashMap<Rc<str>, Rc<Agent>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<Vec<Asset>> {
    let file_path = model_dir.join(ASSETS_FILE_NAME);
    let assets_csv = read_csv(&file_path)?;
    read_assets_from_iter(assets_csv, agents, processes, region_ids)
        .with_context(|| input_err_msg(&file_path))
}

/// Process assets from an iterator.
///
/// # Arguments
///
/// * `iter` - Iterator of `AssetRaw`s
/// * `agents` - All agents
/// * `processes` - The model's processes
/// * `region_ids` - All possible region IDs
///
/// # Returns
///
/// A [`Vec`] of [`Asset`]s or an error.
fn read_assets_from_iter<I>(
    iter: I,
    agents: &HashMap<Rc<str>, Rc<Agent>>,
    processes: &HashMap<Rc<str>, Rc<Process>>,
    region_ids: &HashSet<Rc<str>>,
) -> Result<Vec<Asset>>
where
    I: Iterator<Item = AssetRaw>,
{
    iter.map(|asset| -> Result<_> {
        let agent = agents
            .get(asset.agent_id.as_str())
            .with_context(|| format!("{} is not a valid agent ID", asset.agent_id))?;
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

        Ok(Asset::new(
            Rc::clone(agent),
            Rc::clone(process),
            region_id,
            asset.capacity,
            asset.commission_year,
        ))
    })
    .try_collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{DecisionRule, SearchSpace};
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::process::ProcessParameter;
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;
    use itertools::assert_equal;
    use std::iter;

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
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let agent = Rc::new(Agent {
            id: "agent1".into(),
            description: "".into(),
            commodity,
            commodity_portion: 1.0,
            search_space: SearchSpace::AllProcesses,
            decision_rule: DecisionRule::Single,
            capex_limit: None,
            annual_cost_limit: None,
            regions: RegionSelection::default(),
            objectives: Vec::new(),
        });
        let agents = iter::once(("agent1".into(), agent.clone())).collect();
        let region_ids = ["GBR".into(), "USA".into()].into_iter().collect();

        // Valid
        let asset_in = AssetRaw {
            agent_id: "agent1".into(),
            process_id: "process1".into(),
            region_id: "GBR".into(),
            capacity: 1.0,
            commission_year: 2010,
        };
        let asset_out = Asset::new(agent, Rc::clone(&process), "GBR".into(), 1.0, 2010);
        assert_equal(
            read_assets_from_iter([asset_in].into_iter(), &agents, &processes, &region_ids)
                .unwrap(),
            iter::once(asset_out),
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
            read_assets_from_iter([asset_in].into_iter(), &agents, &processes, &region_ids)
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
            read_assets_from_iter([asset_in].into_iter(), &agents, &processes, &region_ids)
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
            read_assets_from_iter([asset_in].into_iter(), &agents, &processes, &region_ids)
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
            read_assets_from_iter([asset_in].into_iter(), &agents, &processes, &region_ids)
                .is_err()
        );
    }
}
