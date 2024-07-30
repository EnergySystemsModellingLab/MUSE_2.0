use crate::input::{deserialise_proportion_nonzero, input_panic, read_csv};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const AGENT_FILE_NAME: &str = "agents.csv";

#[derive(Debug, PartialEq)]
pub enum SearchSpace {
    AllProcesses,
    Some(HashSet<String>),
}

impl<'de> Deserialize<'de> for SearchSpace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Option::<&str>::deserialize(deserializer)?;
        match value {
            None => Ok(SearchSpace::AllProcesses),
            Some(processes_str) => {
                let processes = HashSet::from_iter(processes_str.split(';').map(String::from));
                Ok(SearchSpace::Some(processes))
            }
        }
    }
}

#[derive(Debug, PartialEq, DeserializeLabeledStringEnum)]
pub enum DecisionRule {
    #[string = "single"]
    Single,
    #[string = "weighted"]
    Weighted,
    #[string = "lexico"]
    Lexicographical,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Agent {
    pub id: Rc<str>,
    pub description: String,
    pub commodity_id: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    pub commodity_portion: f64,
    pub search_space: SearchSpace,
    pub decision_rule: DecisionRule,
    pub capex_limit: Option<f64>,
    pub annual_cost_limit: Option<f64>,
}

/// Read agents info from a CSV file.
pub fn read_agents(model_dir: &Path, process_ids: &HashSet<Rc<str>>) -> HashMap<Rc<str>, Agent> {
    let file_path = model_dir.join(AGENT_FILE_NAME);
    let mut agents = HashMap::new();
    for agent in read_csv::<Agent>(&file_path) {
        if let SearchSpace::Some(ref search_space) = agent.search_space {
            // Check process IDs are all valid
            if !search_space
                .iter()
                .all(|id| process_ids.contains(id.as_str()))
            {
                input_panic(&file_path, "Invalid process ID");
            }
        }

        if agents.insert(Rc::clone(&agent.id), agent).is_some() {
            input_panic(&file_path, "Duplicate agent ID");
        }
    }

    agents
}
