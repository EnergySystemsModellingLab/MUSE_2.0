//! Provides the main entry point to the muse2 program.

mod muse2;

use std::env;

/// The main entry point to the program
fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        panic!("Must provide path to variables.csv and constraints.csv files");
    }

    muse2::run(&args[1], &args[2])
}
