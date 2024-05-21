//! Provides data structures and functions for performing optimisation.
use super::constraint::Constraint;
use super::variable_definition::VariableDefinition;
pub use highs::Sense;
use highs::{HighsModelStatus, RowProblem};

/// Perform an optimisation using the HIGHS solver.
///
/// # Arguments:
///
/// * `definitions`: The definitions of the variables
/// * `constraints`: The constraints for the optimisation problem
/// * `sense`: Whether this is a maximisation or minimisation problem
pub fn solve_highs(
    definitions: &[VariableDefinition],
    constraints: &[Constraint],
    sense: Sense,
) -> Result<Vec<f64>, HighsModelStatus> {
    let mut pb = RowProblem::default();

    // Add variables
    let mut vars = Vec::with_capacity(definitions.len());
    for def in definitions.iter() {
        vars.push(pb.add_column(def.coefficient, def.min..=def.max));
    }

    // Add constraints
    for constraint in constraints.iter() {
        if constraint.coefficients.len() != vars.len() {
            panic!("Wrong number of variables specified for constraint");
        }

        let mut coeffs = Vec::with_capacity(vars.len());
        for (var, coeff) in vars.iter().zip(constraint.coefficients.iter()) {
            coeffs.push((*var, *coeff));
        }

        pb.add_row(constraint.min..=constraint.max, coeffs);
    }

    let solved = pb.optimise(sense).solve();
    match solved.status() {
        HighsModelStatus::Optimal => Ok(solved.get_solution().columns().to_vec()),
        status => Err(status),
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_common::{get_example_constraints, get_example_variable_definitions};
    use super::*;

    #[test]
    fn test_solve_highs() {
        let solution = solve_highs(
            &get_example_variable_definitions(),
            &get_example_constraints(),
            Sense::Maximise,
        )
        .unwrap();

        assert_eq!(solution, &[0., 6., 0.5]);
    }
}
