//! The main crate for muse2. This contains all of MUSE's functionality.
pub mod constraint;
mod settings;
mod solver;
pub mod variable_definition;

#[cfg(test)]
pub mod test_common;

use settings::read_settings;
use solver::{solve_highs, Sense};

/// Run an optimisation.
///
/// Arguments:
///
/// * `settings_file_path`: The path to the TOML file containing the model's configuration
pub fn run(settings_file_path: &str) {
    // Read input files
    let (definitions, constraints) = read_settings(&settings_file_path);

    // Calculate solution
    let solution = solve_highs(&definitions, &constraints, Sense::Maximise)
        .unwrap_or_else(|err| panic!("Failed to calculate a solution: {:?}", err));
    println!("Calculated solution: {:?}", solution);
}
