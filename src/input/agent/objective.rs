//! Code for reading the agent objectives CSV file.
use super::super::*;
use crate::agent::{AgentID, AgentMap, AgentObjectiveMap, DecisionRule, ObjectiveType};
use crate::year::parse_year_str;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

const AGENT_OBJECTIVES_FILE_NAME: &str = "agent_objectives.csv";

/// An objective for an agent with associated parameters
#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AgentObjectiveRaw {
    /// Unique agent id identifying the agent this objective belongs to
    agent_id: AgentID,
    /// The year(s) the objective is relevant for
    year: String,
    /// Acronym identifying the objective (e.g. LCOX)
    objective_type: ObjectiveType,
    /// For the weighted sum decision rule, the set of weights to apply to each objective.
    decision_weight: Option<f64>,
    /// For the lexico decision rule, the order in which to consider objectives.
    decision_lexico_order: Option<u32>,
}

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
) -> Result<HashMap<AgentID, AgentObjectiveMap>> {
    let file_path = model_dir.join(AGENT_OBJECTIVES_FILE_NAME);
    let agent_objectives_csv = read_csv(&file_path)?;
    read_agent_objectives_from_iter(agent_objectives_csv, agents, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_objectives_from_iter<I>(
    iter: I,
    agents: &AgentMap,
    milestone_years: &[u32],
) -> Result<HashMap<AgentID, AgentObjectiveMap>>
where
    I: Iterator<Item = AgentObjectiveRaw>,
{
    let mut all_objectives = HashMap::new();
    for objective in iter {
        let (id, agent) = agents
            .get_key_value(&objective.agent_id)
            .context("Invalid agent ID")?;

        // Check that required parameters are present and others are absent
        check_objective_parameter(&objective, &agent.decision_rule)?;

        let agent_objectives = all_objectives
            .entry(id.clone())
            .or_insert_with(AgentObjectiveMap::new);
        for year in parse_year_str(&objective.year, milestone_years)? {
            try_insert(agent_objectives, year, objective.objective_type).with_context(|| {
                format!(
                    "Duplicate agent objective entry for agent {} and year {}",
                    id, year
                )
            })?;
        }
    }

    // Check that agents have one objective per milestone year
    for agent_id in agents.keys() {
        let agent_objectives = all_objectives
            .get(agent_id)
            .with_context(|| format!("Agent {} has no objectives", agent_id))?;

        let missing_years = milestone_years
            .iter()
            .filter(|year| !agent_objectives.contains_key(year))
            .collect_vec();
        ensure!(
            missing_years.is_empty(),
            "Agent {} is missing objectives for the following milestone years: {:?}",
            agent_id,
            missing_years
        );
    }

    Ok(all_objectives)
}

/// Check that required parameters are present and others are absent
fn check_objective_parameter(
    objective: &AgentObjectiveRaw,
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

/// Check that a set of objectives meets the requirements of a decision rule.
///
/// NB: Unused for now as we only support the "single" decision rule.
#[cfg(test)]
fn check_agent_objectives(
    objectives: &[&AgentObjectiveRaw],
    decision_rule: &DecisionRule,
    agent_id: &AgentID,
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
    use std::iter;

    use super::*;
    use crate::agent::{Agent, AgentCommodityPortionsMap, AgentCostLimitsMap, ObjectiveType};

    #[test]
    fn test_check_objective_parameter() {
        macro_rules! objective {
            ($decision_weight:expr, $decision_lexico_order:expr) => {
                AgentObjectiveRaw {
                    agent_id: "agent".into(),
                    year: "2020".into(),
                    objective_type: ObjectiveType::LevelisedCostOfX,
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
                commodity_portions: AgentCommodityPortionsMap::new(),
                search_space: Vec::new(),
                decision_rule: DecisionRule::Single,
                cost_limits: AgentCostLimitsMap::new(),
                regions: HashSet::new(),
                objectives: AgentObjectiveMap::new(),
            },
        )]
        .into_iter()
        .collect();
        let milestone_years = [2020];

        // Valid
        let objective = AgentObjectiveRaw {
            agent_id: "agent".into(),
            year: "2020".into(),
            objective_type: ObjectiveType::LevelisedCostOfX,
            decision_weight: None,
            decision_lexico_order: None,
        };
        let expected = iter::once((
            "agent".into(),
            iter::once((2020, objective.objective_type)).collect(),
        ))
        .collect();
        let actual = read_agent_objectives_from_iter(
            iter::once(objective.clone()),
            &agents,
            &milestone_years,
        )
        .unwrap();
        assert_eq!(actual, expected);

        // Missing objective for agent
        assert!(read_agent_objectives_from_iter(iter::empty(), &agents, &milestone_years).is_err());

        // Missing objective for milestone year
        assert!(
            read_agent_objectives_from_iter(iter::once(objective), &agents, &[2020, 2030]).is_err()
        );

        // Bad parameter
        let bad_objective = AgentObjectiveRaw {
            agent_id: "agent".into(),
            year: "2020".into(),
            objective_type: ObjectiveType::LevelisedCostOfX,
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
        let agent_id = AgentID::new("agent");
        let objective1 = AgentObjectiveRaw {
            agent_id: agent_id.clone(),
            year: "2020".into(),
            objective_type: ObjectiveType::LevelisedCostOfX,
            decision_weight: None,
            decision_lexico_order: Some(1),
        };
        let objective2 = AgentObjectiveRaw {
            agent_id: agent_id.clone(),
            year: "2020".into(),
            objective_type: ObjectiveType::LevelisedCostOfX,
            decision_weight: None,
            decision_lexico_order: Some(2),
        };

        // DecisionRule::Single
        let decision_rule = DecisionRule::Single;
        let objectives = [&objective1];

        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_ok());
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_err());

        // DecisionRule::Weighted
        let decision_rule = DecisionRule::Weighted;
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_ok());
        let objectives = [&objective1];
        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_err());

        // DecisionRule::Lexicographical
        let decision_rule = DecisionRule::Lexicographical { tolerance: 1.0 };
        let objectives = [&objective1, &objective2];
        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_ok());
        let objectives = [&objective1, &objective1];
        assert!(check_agent_objectives(&objectives, &decision_rule, &agent_id, 2020).is_err());
    }
}
