//! Provides the main entry point to the muse2 program.

mod muse2;

use std::env;
use std::path::Path;

/// The main entry point to the program
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("Must provide path to model configuration TOML file.");
    }

    muse2::run(Path::new(&args[1]))
}
