//! Provides the main entry point to the program.

use std::env;
use std::error::Error;
use std::path::Path;

mod simulation;

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Usage: muse2 <config_file_path>");
    }

    let config_file_path = Path::new(&args[1]);

    simulation::run(config_file_path)
}
