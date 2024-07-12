//! High level functionality for launching a simulation.
use std::error::Error;
use std::path::Path;
use crate::csv_reader::read_agents_from_csv;

pub fn run(config_file_path: &Path, csv_file_path: &Path) -> Result<(), Box<dyn Error>> {
    // Read and print agents
    let agents = read_agents_from_csv(csv_file_path)?;

    for agent in agents {
        println!("{:?}", agent);
    }

    // Additional simulation code using `config_file_path`
    // ...

    Ok(())
}

