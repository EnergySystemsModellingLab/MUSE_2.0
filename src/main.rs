mod demand;
mod input;
mod settings;
mod simulation;
mod time_slices;

use std::env;
use std::path::Path;

/// The main entry point to the program
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model configuration TOML file.");
    }

    // Initialize the simulation with demand data
    demand::initialize_simulation();

    simulation::run(Path::new(&args[1]))
}
