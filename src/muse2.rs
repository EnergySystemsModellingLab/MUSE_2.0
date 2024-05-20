//! The main crate for muse2. This contains all of MUSE's functionality.
mod csv;
mod solver;

use csv::{read_constraints, read_variables};
use solver::{solve_highs, Sense};

/// Run an optimisation.
///
/// Arguments:
///
/// * `variables_path`: The path to the CSV file containing variable definitions
/// * `constraints_path`: The path to the CSV file containing constraints
pub fn run(variables_path: &str, constraints_path: &str) {
    // Read variable definitions
    let vars = match read_variables(variables_path) {
        Ok(x) => x,
        Err(error) => panic!("Error reading variables from {}: {}", variables_path, error),
    };

    // Read constraints
    let constraints = match read_constraints(constraints_path, &vars) {
        Ok(constraints) => constraints,
        Err(error) => panic!(
            "Error reading constraints from {}: {}",
            constraints_path, error
        ),
    };

    // Calculate solution
    let solution = solve_highs(&vars, &constraints, Sense::Maximise)
        .unwrap_or_else(|err| panic!("Failed to calculate a solution: {:?}", err));
    println!("Calculated solution: {:?}", solution);
}
