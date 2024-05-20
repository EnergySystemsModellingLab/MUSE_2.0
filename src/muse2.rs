//! The main crate for muse2. This contains all of MUSE's functionality.
pub mod constraint;
mod solver;
pub mod variable_definition;

#[cfg(test)]
pub mod test_common;

use constraint::Constraint;
use solver::{solve_highs, Sense};
use variable_definition::VariableDefinition;

/// Run an optimisation.
///
/// Arguments:
///
/// * `variables_path`: The path to the CSV file containing variable definitions
/// * `constraints_path`: The path to the CSV file containing constraints
pub fn run(variables_path: &str, constraints_path: &str) {
    // Read variable definitions
    let vars = match VariableDefinition::vec_from_csv(variables_path) {
        Ok(x) => x,
        Err(error) => panic!("Error reading variables from {}: {}", variables_path, error),
    };

    // Read constraints
    let constraints = match Constraint::vec_from_csv(constraints_path, &vars) {
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
