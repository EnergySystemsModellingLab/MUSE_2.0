//! Code for reading the agent objectives CSV file.
use super::super::*;
use crate::agent::{Agent, AgentObjective, DecisionRule};
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
    agents: &HashMap<Rc<str>, Agent>,
) -> Result<HashMap<Rc<str>, Vec<AgentObjective>>> {
    let file_path = model_dir.join(AGENT_OBJECTIVES_FILE_NAME);
    let agent_objectives_csv = read_csv(&file_path)?;
    read_agent_objectives_from_iter(agent_objectives_csv, agents)
        .with_context(|| input_err_msg(&file_path))
}

fn read_agent_objectives_from_iter<I>(
    iter: I,
    agents: &HashMap<Rc<str>, Agent>,
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

        // Append to Vec with the corresponding key or create
        objectives
            .entry(Rc::clone(id))
            .or_insert_with(|| Vec::with_capacity(1))
            .push(objective);
    }

    ensure!(
        objectives.len() >= agents.len(),
        "All agents must have at least one objective"
    );

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
            check_field_none!(decision_lexico_tolerance);
        }
        DecisionRule::Weighted => {
            check_field_none!(decision_lexico_tolerance);
            check_field_some!(decision_weight);
        }
        DecisionRule::Lexicographical => {
            check_field_none!(decision_weight);
            check_field_some!(decision_lexico_tolerance);
        }
    };

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{ObjectiveType, SearchSpace};
    use crate::region::RegionSelection;

    #[test]
    fn test_check_objective_parameter() {
        macro_rules! objective {
            ($decision_weight:expr, $decision_lexico_tolerance:expr) => {
                AgentObjective {
                    agent_id: "agent".into(),
                    objective_type: ObjectiveType::EquivalentAnnualCost,
                    decision_weight: $decision_weight,
                    decision_lexico_tolerance: $decision_lexico_tolerance,
                }
            };
        }

        // DecisionRule::Single
        let decision_rule = DecisionRule::Single;
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Weighted
        let decision_rule = DecisionRule::Weighted;
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());

        // DecisionRule::Lexicographical
        let decision_rule = DecisionRule::Lexicographical;
        let objective = objective!(None, Some(1.0));
        assert!(check_objective_parameter(&objective, &decision_rule).is_ok());
        let objective = objective!(None, None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
        let objective = objective!(Some(1.0), None);
        assert!(check_objective_parameter(&objective, &decision_rule).is_err());
    }

    #[test]
    fn test_read_agent_objectives_from_iter() {
        let agents: HashMap<_, _> = [(
            "agent".into(),
            Agent {
                id: "agent".into(),
                description: "".into(),
                commodity_id: "".into(),
                commodity_portion: 1.0,
                search_space: SearchSpace::AllProcesses,
                decision_rule: DecisionRule::Single,
                capex_limit: None,
                annual_cost_limit: None,
                regions: RegionSelection::All,
                objectives: Vec::new(),
                assets: Vec::new(),
            },
        )]
        .into_iter()
        .collect();

        // Valid
        let objective = AgentObjective {
            agent_id: "agent".into(),
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: None,
            decision_lexico_tolerance: None,
        };
        let expected = [("agent".into(), vec![objective.clone()])]
            .into_iter()
            .collect();
        let actual = read_agent_objectives_from_iter([objective].into_iter(), &agents).unwrap();
        assert_eq!(actual, expected);

        // Missing objective for agent
        assert!(read_agent_objectives_from_iter([].into_iter(), &agents).is_err());

        // Bad parameter
        let objective = AgentObjective {
            agent_id: "agent".into(),
            objective_type: ObjectiveType::EquivalentAnnualCost,
            decision_weight: Some(1.0),
            decision_lexico_tolerance: None,
        };
        assert!(read_agent_objectives_from_iter([objective].into_iter(), &agents).is_err());
    }
}
