//! Provides the main entry point to the program.

use std::error::Error;
use std::path::Path;
mod csv;
use csv::read_agents_from_csv;

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = Path::new("agents.csv");
    let agents = read_agents_from_csv(file_path)?;

    for agent in agents {
        println!("{:?}", agent);
    }

    Ok(())
