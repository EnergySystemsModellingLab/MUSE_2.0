//! Provides the main entry point to the example program.

mod example_code;

use std::env;
use std::path::Path;

/// The main entry point to the program
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model configuration TOML file.");
    }

    example_code::run(Path::new(&args[1]))
}
