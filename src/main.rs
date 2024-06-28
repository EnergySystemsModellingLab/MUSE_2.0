use std::env;
use std::path::Path;
mod simulation;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <demand.csv>", args[0]);
        std::process::exit(1);
    }
    simulation::initialize_simulation(Path::new(&args[1]));
}
