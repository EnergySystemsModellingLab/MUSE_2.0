//! Code for reading the agent commodities CSV file.
use super::super::*;
use crate::agent::{AgentCommodity, AgentID, AgentMap};
use crate::commodity::{CommodityMap, CommodityType};
use crate::region::RegionID;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const AGENT_COMMODITIES_FILE_NAME: &str = "agent_commodities.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct AgentCommodityRaw {
    /// Unique agent id identifying the agent.
    agent_id: String,
    /// The commodity that the agent is responsible for.
    commodity_id: String,
    /// The year the commodity portion applies to.
    year: u32,
    /// The proportion of the commodity production that the agent is responsible for.
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    commodity_portion: f64,
}

impl AgentCommodityRaw {
    fn to_agent_commodity(
        &self,
        commodities: &CommodityMap,
        milestone_years: &[u32],
    ) -> Result<AgentCommodity> {
        // Get commodity
        let commodity = commodities
            .get(self.commodity_id.as_str())
            .context("Invalid commodity ID")?;

        // Check that the year is a valid milestone year
        ensure!(
            milestone_years.binary_search(&self.year).is_ok(),
            "Invalid milestone year {}",
            self.year
        );

        // Create AgentCommodity
        Ok(AgentCommodity {
            year: self.year,
            commodity: Rc::clone(commodity),
            commodity_portion: self.commodity_portion,
        })
    }
}

/// Read agent objective info from the agent_commodities.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agent_commodities(
    model_dir: &Path,
    agents: &AgentMap,
    commodities: &CommodityMap,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, Vec<AgentCommodity>>> {
    let file_path = model_dir.join(AGENT_COMMODITIES_FILE_NAME);
    let agent_commodities_csv = read_csv(&file_path)?;
    read_agent_commodities_from_iter(
        agent_commodities_csv,
        agents,
        commodities,
        region_ids,
        milestone_years,
    )
    .with_context(|| input_err_msg(&file_path))
}

fn read_agent_commodities_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    commodities: &CommodityMap,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, Vec<AgentCommodity>>>
where
    I: Iterator<Item = AgentCommodityRaw>,
{
    let mut agent_commodities = HashMap::new();
    for agent_commodity_raw in iter {
        let agent_commodity =
            agent_commodity_raw.to_agent_commodity(commodities, milestone_years)?;

        let (id, _agent) = agents
            .get_key_value(agent_commodity_raw.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Append to Vec with the corresponding key or create
        agent_commodities
            .entry(id.clone())
            .or_insert_with(|| Vec::with_capacity(1))
            .push(agent_commodity);
    }

    validate_agent_commodities(
        &agent_commodities,
        agents,
        commodities,
        region_ids,
        milestone_years,
    )?;

    Ok(agent_commodities)
}

fn validate_agent_commodities(
    agent_commodities: &HashMap<AgentID, Vec<AgentCommodity>>,
    agents: &AgentMap,
    commodities: &CommodityMap,
    region_ids: &HashSet<RegionID>,
    milestone_years: &[u32],
) -> Result<()> {
    // CHECK 1: For each agent there must be at least one commodity for all years
    for (id, agent_commodities) in agent_commodities.iter() {
        let mut years = HashSet::new();
        for agent_commodity in agent_commodities {
            years.insert(agent_commodity.year);
        }
        for year in milestone_years {
            ensure!(
                years.contains(year),
                "Agent {} does not have a commodity for year {}",
                id,
                year
            );
        }
    }

    // CHECK 2: Total portions for each commodity/year/region must sum to 1
    // First step is to create a map with the key as (commodity_id, year, region_id), and the value
    // as the sum of the portions for that key across all agents
    let mut summed_portions = HashMap::new();
    for (id, agent_commodities) in agent_commodities.iter() {
        let agent = agents.get(id).context("Invalid agent ID")?;
        for agent_commodity in agent_commodities {
            let commodity_id = agent_commodity.commodity.get_id();
            let portion = agent_commodity.commodity_portion;
            for region in region_ids {
                if agent.regions.contains(region) {
                    let key = (commodity_id, agent_commodity.year, region);
                    summed_portions
                        .entry(key)
                        .and_modify(|v| *v += portion)
                        .or_insert(portion);
                }
            }
        }
    }

    // We then check the map to ensure values for each key are 1
    for (key, portion) in summed_portions.iter() {
        ensure!(
            approx_eq!(f64, *portion, 1.0, epsilon = 1e-5),
            "Commodity {} in year {} and region {} does not sum to 1.0",
            key.0,
            key.1,
            key.2
        );
    }

    // CHECK 3: All commodities of SVD or SED type must be covered for all regions and years
    // This checks the same summed_portions map as above, just checking the keys
    // We first need to create a list of SVD and SED commodities to check against
    let svd_and_sed_commodities = commodities
        .iter()
        .filter(|(_, commodity)| {
            matches!(
                commodity.kind,
                CommodityType::SupplyEqualsDemand | CommodityType::ServiceDemand
            )
        })
        .map(|(id, _)| id.clone());

    // Check that summed_portions contains all SVD/SED commodities for all regions and milestone
    // years
    for commodity_id in svd_and_sed_commodities {
        for year in milestone_years {
            for region in region_ids {
                let key = (&commodity_id, *year, region);
                ensure!(
                    summed_portions.contains_key(&key),
                    "Commodity {} in year {} and region {} is not covered",
                    commodity_id,
                    year,
                    region
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, DecisionRule};
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;

    use std::iter;

    #[test]
    fn test_agent_commodity_raw_to_agent_commodity() {
        let milestone_years = vec![2020, 2021, 2022];
        let commodity = Rc::new(Commodity {
            id: "commodity1".into(),
            description: "A commodity".into(),
            kind: CommodityType::SupplyEqualsDemand,
            time_slice_level: TimeSliceLevel::Annual,
            costs: CommodityCostMap::new(),
            demand: DemandMap::new(),
        });
        let commodities = iter::once(("commodity1".into(), Rc::clone(&commodity))).collect();

        // Valid case
        let raw = AgentCommodityRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            year: 2020,
            commodity_portion: 1.0,
        };
        assert!(raw
            .to_agent_commodity(&commodities, &milestone_years)
            .is_ok());

        // Invalid case: year not in milestone years
        let raw = AgentCommodityRaw {
            agent_id: "agent1".into(),
            commodity_id: "commodity1".into(),
            year: 2019,
            commodity_portion: 1.0,
        };
        assert!(raw
            .to_agent_commodity(&commodities, &milestone_years)
            .is_err());

        // Invalid case: invalid commodity ID
        let raw = AgentCommodityRaw {
            agent_id: "agent1".into(),
            commodity_id: "invalid_commodity".into(),
            year: 2020,
            commodity_portion: 1.0,
        };
        assert!(raw
            .to_agent_commodity(&commodities, &milestone_years)
            .is_err());
    }

    #[test]
    fn test_validate_agent_commodities() {
        let agents = IndexMap::from([(
            Rc::from("agent1"),
            Agent {
                id: Rc::from("agent1"),
                description: "An agent".into(),
                commodities: Vec::new(),
                search_space: Vec::new(),
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::default(),
                objectives: Vec::new(),
            },
        )]);
        let mut commodities = IndexMap::from([(
            Rc::from("commodity1"),
            Rc::new(Commodity {
                id: "commodity1".into(),
                description: "A commodity".into(),
                kind: CommodityType::SupplyEqualsDemand,
                time_slice_level: TimeSliceLevel::Annual,
                costs: CommodityCostMap::new(),
                demand: DemandMap::new(),
            }),
        )]);
        let region_ids = HashSet::from([Rc::from("region1")]);
        let milestone_years = vec![2020];

        // Valid case
        let agent_commodity = AgentCommodity {
            year: 2020,
            commodity: Rc::clone(commodities.get("commodity1").unwrap()),
            commodity_portion: 1.0,
        };
        let agent_commodities = HashMap::from([(Rc::from("agent1"), vec![agent_commodity])]);
        assert!(validate_agent_commodities(
            &agent_commodities,
            &agents,
            &commodities,
            &region_ids,
            &milestone_years
        )
        .is_ok());

        // Invalid case: portions do not sum to 1
        let agent_commodity_v2 = AgentCommodity {
            year: 2020,
            commodity: Rc::clone(commodities.get("commodity1").unwrap()),
            commodity_portion: 0.5,
        };
        let agent_commodities_v2 = HashMap::from([(Rc::from("agent1"), vec![agent_commodity_v2])]);
        assert!(validate_agent_commodities(
            &agent_commodities_v2,
            &agents,
            &commodities,
            &region_ids,
            &milestone_years
        )
        .is_err());

        // Invalid case: SED commodity without associated commodity portions
        commodities.insert(
            Rc::from("commodity2"),
            Rc::new(Commodity {
                id: "commodity2".into(),
                description: "Another commodity".into(),
                kind: CommodityType::SupplyEqualsDemand,
                time_slice_level: TimeSliceLevel::Annual,
                costs: CommodityCostMap::new(),
                demand: DemandMap::new(),
            }),
        );
        assert!(validate_agent_commodities(
            &agent_commodities,
            &agents,
            &commodities,
            &region_ids,
            &milestone_years
        )
        .is_err());
    }
}
