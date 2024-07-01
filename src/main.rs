//! Provides the main entry point to the program.

mod settings;
mod simulation;
mod time_slices;

use std::env;
use std::path::Path;

/// The main entry point to the program
fn main() {
    let args: Vec<String> = env::args().collect();
    // NB: We also currently require a path to a time_slices.csv file, but this is just a stopgap
    if args.len() != 3 {
        panic!("Must provide path to model configuration TOML file.");
    }

    simulation::run(Path::new(&args[1]), Path::new(&args[2]))
}
