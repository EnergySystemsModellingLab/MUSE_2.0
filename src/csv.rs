use csv::Reader;
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct Agent {
    name: String,
    description: Option<String>,
    commodity_name: String,
    commodity_portion: f64,
    search_space: String,
    decision_rule: DecisionRule,
    capex_limit: Option<f64>,
    annual_cost_limit: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")] // Changed to uppercase
pub enum DecisionRule {
    Single,
    Weighted,
    Lexico,
}

fn default_search_space() -> String {
    "all".to_string()
}

pub fn read_agents_from_csv(file_path: &Path) -> Result<Vec<Agent>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut rdr = Reader::from_reader(file);
    let mut agents = Vec::new();

    for result in rdr.deserialize() {
        let agent: Agent = result?;
        agents.push(agent);
    }

    Ok(agents)
}
