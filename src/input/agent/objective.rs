//! Code for reading the agent objectives CSV file.
use super::super::*;
use crate::agent::{Agent, AgentMap, AgentObjective, DecisionRule};
use anyhow::{ensure, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const AGENT_OBJECTIVES_FILE_NAME: &str = "agent_objectives.csv";

define_id_getter! {Agent}

/// Read agent objective info from the agent_objectives.csv file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A map of Agents, with the agent ID as the key
pub fn read_agent_objectives(
    model_dir: &Path,
    agents: &AgentMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentObjective>>> {
    let file_path = model_dir.join(AGENT_OBJECTIVES_FILE_NAME);
    let agent_objectives_csv = read_csv(&file_path)?;
    read_agent_objectives_from_iter(agent_objectives_csv, agents, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_objectives_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, Vec<AgentObjective>>>
where
    I: Iterator<Item = AgentObjective>,
{
    let mut objectives = HashMap::new();
    for objective in iter {
        let (id, agent) = agents
            .get_key_value(objective.agent_id.as_str())
            .context("Invalid agent ID")?;

        // Check that required parameters are present and others are absent
        check_objective_parameter(&objective, &agent.decision_rule)?;

        // Check that the year is a valid milestone year
        ensure!(
            milestone_years.binary_search(&objective.year).is_ok(),
            "Invalid milestone year {}",
            objective.year
        );

        // Append to Vec with the corresponding key or create
        objectives
            .entry(Rc::clone(id))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(objective);
    }

    // Check that agents have appropriate objectives for their decision rule every year
    for (agent_id, agent) in agents {
        let agent_objectives = objectives
            .get(agent_id)
            .with_context(|| format!("Agent {} has no objectives", agent_id))?;
        for &year in milestone_years {
            let objectives_for_year: Vec<_> = agent_objectives
                .iter()
                .filter(|obj| obj.year == year)
                .collect();
            check_agent_objectives(&objectives_for_year, &agent.decision_rule, agent_id, year)?;
        }
    }

    Ok(objectives)
}

/// Check that required parameters are present and others are absent
fn check_objective_parameter(
    objective: &AgentObjective,
    decision_rule: &DecisionRule,
) -> Result<()> {
    // Check that the user hasn't supplied a value for a field we're not using
    macro_rules! check_field_none {
        ($field:ident) => {
            ensure!(
                objective.$field.is_none(),
                "Field {} should be empty for this decision rule",
                stringify!($field)
            )
        };
    }

    // Check that required fields are present
    macro_rules! check_field_some {
        ($field:ident) => {
            ensure!(
                objective.$field.is_some(),
                "Required field {} is empty",
                stringify!($field)
            )
        };
    }

    match decision_rule {
        DecisionRule::Single => {
            check_field_none!(decision_weight);
            check_field_none!(decision_lexico_order);
        }
        DecisionRule::Weighted => {
            check_field_some!(decision_weight);
            check_field_none!(decision_lexico_order);
        }
        DecisionRule::Lexicographical { tolerance: _ } => {
            check_field_none!(decision_weight);
            check_field_some!(decision_lexico_order);
        }
    };

    Ok(())
}

/// Check that a set of objectives meets the requirements of a decision rule
fn check_agent_objectives(
    objectives: &[&AgentObjective],
    decision_rule: &DecisionRule,
    agent_id: &str,
    year: u32,
) -> Result<()> {
    let count = objectives.len();
    match decision_rule {
        DecisionRule::Single => {
            ensure!(
                count == 1,
                "Agent {} has {} objectives for milestone year {} but should have exactly 1",
                agent_id,
                count,
                year
            );
        }
        DecisionRule::Weighted => {
            ensure!(
                count > 1,
                "Agent {} has {} objectives for milestone year {} but should have more than 1",
                agent_id,
                count,
                year
            );
        }
        DecisionRule::Lexicographical { tolerance: _ } => {
            let mut lexico_orders: Vec<u32> = objectives
                .iter()
                .filter_map(|obj| obj.decision_lexico_order)
                .collect();
            lexico_orders.sort_unstable();
            ensure!(
                lexico_orders == [1, 2],
                "Agent {} must have objectives with decision_lexico_order values of 1 and 2 for milestone year {}, but found {:?}",
                agent_id,
                year,
                lexico_orders
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ObjectiveType;
    use crate::commodity::{Commodity, CommodityCostMap, CommodityType, DemandMap};
    use crate::region::RegionSelection;
    use crate::time_slice::TimeSliceLevel;

    #[test]
    fn test_check_objective_parameter() {
        macro_rules! objective {
            ($decision_weight:expr, $decision_lexico_order:expr) => {
                AgentObjective {
                    agent_id: "agent".into(),
                    year: 2020,
                    objective_type: ObjectiveType::EquivalentAnnualCost,
                    decision_weight: $decision_weight,
                    decision_lexico_order: $decision_lexico_order,
                }
            };
        }

        // DecisionRule::Single
        let decision_rule = DecisionRule::Single;
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Weighted
        let decision_rule = DecisionRule::Weighted;
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Lexicographical
        let decision_rule = DecisionRule::Lexicographical { tolerance: 1.0 };
        let objective = objective!(None, Some(1));
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
    }

    #[test]
    fn test_read_agent_objectives_from_iter() {
        let agents = [(
            "agent".into(),
            Agent {
                id: "agent".into(),
                description: "".into(),
                commodities: Vec::new(),
                search_space: Vec::new(),
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::All,
                objectives: Vec::new(),
            },
        )]
        .into_iter()
        .collect();
        let milestone_years = [2020];

        // Valid
        let objective = AgentObjective {
            agent_id: "agent".into(),
            year: 2020,
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: None,
            decision_lexico_order: None,
        };
        let expected = [("agent".into(), vec![objective.clone()])]
            .into_iter()
            .collect();
        let actual = read_agent_objectives_from_iter(
            [objective.clone()].into_iter(),
            &agents,
            &milestone_years,
        )
        .unwrap();
        assert_eq!(actual, expected);

        // Missing objective for agent
        assert!(
            read_agent_objectives_from_iter([].into_iter(), &agents, &milestone_years).is_err()
        );

        // Missing objective for milestone year
        assert!(
            read_agent_objectives_from_iter([objective].into_iter(), &agents, &[2020, 2030])
                .is_err()
        );

        // Bad parameter
        let bad_objective = AgentObjective {
            agent_id: "agent".into(),
            year: 2020,
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: Some(1.0), // Should only accept None for DecisionRule::Single
            decision_lexico_order: None,
        };
        assert!(read_agent_objectives_from_iter(
            [bad_objective].into_iter(),
            &agents,
            &milestone_years
        )
        .is_err());
    }

    #[test]
    fn test_check_agent_objectives() {
        let objective1 = AgentObjective {
            agent_id: "agent".into(),
            year: 2020,
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: None,
            decision_lexico_order: Some(1),
        };
        let objective2 = AgentObjective {
            agent_id: "agent".into(),
            year: 2020,
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: None,
            decision_lexico_order: Some(2),
        };

        // DecisionRule::Single
        let decision_rule = DecisionRule::Single;
        let objectives = [&objective1];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_ok());
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_err());

        // DecisionRule::Weighted
        let decision_rule = DecisionRule::Weighted;
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_ok());
        let objectives = [&objective1];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_err());

        // DecisionRule::Lexicographical
        let decision_rule = DecisionRule::Lexicographical { tolerance: 1.0 };
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_ok());
        let objectives = [&objective1, &objective1];
        assert!(check_agent_objectives(&objectives, &decision_rule, "agent", 2020).is_err());
    }
}
