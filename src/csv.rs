use csv::Reader;
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::Path;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Deserialize)]
/// Represents an agent with various attributes including name, description, and decision rules.
struct Agent {
    /// The name of the agent (required).
    name: String,
    /// A description of the agent (optional).
    description: Option<String>,
    /// The name of the commodity the agent deals with (required).
    commodity_name: String,
    /// The portion of the commodity the agent handles, ranging from 0 to 1 (required).
    commodity_portion: f32, // f32 is sufficient for values between 0 and 1
    /// The search space for the agent, defaulting to "all" if not specified.
    #[serde(default = "default_search_space")]
    search_space: String,
    /// The decision rule the agent follows, can be "single", "weighted", or "lexico" (required).
    decision_rule: String,
    /// The capital expenditure limit for the agent (optional, defaults to no limit).
    capex_limit: Option<f64>,
    /// The annual cost limit for the agent (optional, defaults to no limit).
    annual_cost_limit: Option<f64>,
}

fn default_search_space() -> String {
    "all".to_string()
}

/// Reads agents from a CSV file specified by the given file path.
/// 
/// # Arguments
/// 
/// * `file_path` - A reference to a `Path` that holds the file path of the CSV file.
/// 
/// # Returns
/// 
/// * `Result<Vec<Agent>, Box<dyn Error>>` - A result containing a vector of agents or an error.
/// 
/// # Example
/// 
/// ```
/// let agents = read_agents_from_csv(Path::new("agents.csv")).expect("Failed to read agents");
/// ```
fn read_agents_from_csv(file_path: &Path) -> Result<Vec<Agent>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut rdr = Reader::from_reader(file);
    let mut agents = Vec::new();

    for result in rdr.deserialize() {
        let agent: Agent = result?;
        agents.push(agent);
    }

    Ok(agents)
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = Path::new("agents.csv");
    let agents = read_agents_from_csv(file_path)?;

    for agent in agents {
        println!("{:?}", agent);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_agents_from_csv() {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        writeln!(file, "name,description,commodity_name,commodity_portion,search_space,decision_rule,capex_limit,annual_cost_limit")
            .expect("Failed to write header");
        writeln!(file, "John Doe,Senior Agent,Gold,0.75,Process1;Process2,single,100000,50000")
            .expect("Failed to write record");
        writeln!(file, "Jane Smith,,Silver,0.50,,weighted,,100000")
            .expect("Failed to write record");

        let agents = read_agents_from_csv(file.path()).expect("Failed to read agents");

        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].name, "John Doe");
        assert_eq!(agents[0].description, Some("Senior Agent".to_string()));
        assert_eq!(agents[0].commodity_name, "Gold");
        assert_eq!(agents[0].commodity_portion, 0.75);
        assert_eq!(agents[0].search_space, "Process1;Process2".to_string());
        assert_eq!(agents[0].decision_rule, "single");
        assert_eq!(agents[0].capex_limit, Some(100000.0));
        assert_eq!(agents[0].annual_cost_limit, Some(50000.0));

        assert_eq!(agents[1].name, "Jane Smith");
        assert_eq!(agents[1].description, None);
        assert_eq!(agents[1].commodity_name, "Silver");
        assert_eq!(agents[1].commodity_portion, 0.50);
        assert_eq!(agents[1].search_space, "all".to_string());
        assert_eq!(agents[1].decision_rule, "weighted");
        assert_eq!(agents[1].capex_limit, None);
        assert_eq!(agents[1].annual_cost_limit, Some(100000.0));
    }
}
