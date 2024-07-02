//! Provides the main entry point to the program.

mod input;
mod settings;
mod simulation;
mod time_slices;

use std::env;
use std::path::Path;

/// The main entry point to the program
fn main() {
    // Log at the info level by default, unless the user has specified otherwise
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info")
    }

    // Initialise logger
    env_logger::init();

    // Parse command-line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model configuration TOML file.");
    }

    // Run simulation
    simulation::run(Path::new(&args[1]))
}
