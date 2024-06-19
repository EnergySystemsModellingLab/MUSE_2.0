use csv::Reader;
use serde::Deserialize;
use std::error::Error;
use serde_with::{serde_as, DisplayFromStr};

#[serde_as]
#[derive(Debug, Deserialize)]
struct Agent {
    #[serde(rename = "agent.name")]
    name: String,
    #[serde(rename = "agent.description")]
    description: Option<String>,
    #[serde(rename = "agent.commodity_name")]
    commodity_name: String,
    #[serde(rename = "agent.commodity_portion")]
    #[serde_as(as = "DisplayFromStr")]
    commodity_portion: f32,  // f32 is sufficient for values between 0 and 1
    #[serde(rename = "agent.search_space", default = "default_search_space")]
    search_space: Option<String>,
    #[serde(rename = "agent.decision_rule")]
    decision_rule: String,
    #[serde(rename = "agent.capex_limit", default)]
    capex_limit: Option<f64>,
    #[serde(rename = "agent.annual_cost_limit", default)]
    annual_cost_limit: Option<f64>,
}

fn default_search_space() -> Option<String> {
    Some(String::from("all"))
}

fn read_agents_from_csv(file_path: &str) -> Result<Vec<Agent>, Box<dyn Error>> {
    let mut rdr = Reader::from_path(file_path)?;
    let mut agents = Vec::new();

    for result in rdr.deserialize() {
        let agent: Agent = result?;
        agents.push(agent);
    }

    Ok(agents)
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = "agents.csv";
    let agents = read_agents_from_csv(file_path)?;

    for agent in agents {
        println!("{:?}", agent);
    }

    Ok(())
}
