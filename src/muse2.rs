//! The main crate for muse2. This contains all of MUSE's functionality.
pub mod csv;
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
    let (var_names, var_defs) = match read_variables(variables_path) {
        Ok(x) => x,
        Err(error) => panic!("Error reading variables from {}: {}", variables_path, error),
    };

    // Read constraints
    let constraints = match read_constraints(constraints_path, &var_names) {
        Ok(constraints) => constraints,
        Err(error) => panic!(
            "Error reading constraints from {}: {}",
            constraints_path, error
        ),
    };

    // Calculate solution
    let solution = solve_highs(&var_defs, &constraints, Sense::Maximise);
    println!("Calculated solution: {:?}", solution);
}
