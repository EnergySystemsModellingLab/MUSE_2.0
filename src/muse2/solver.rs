///! Provides data structures and functions for performing optimisation.
pub use highs::Sense;
use highs::{HighsModelStatus, RowProblem};

/// The definition of a variable to be optimised
#[derive(PartialEq, Debug)]
pub struct VariableDefinition {
    /// The variable's minimum value
    pub min: f64,
    /// The variable's maximum value
    pub max: f64,
    /// The coefficient of the variable in the objective
    pub coefficient: f64,
}

/// A constraint for an optimisation
#[derive(PartialEq, Debug)]
pub struct Constraint {
    /// The minimum value for the constraint
    pub min: f64,
    /// The maximum value for the constraint
    pub max: f64,
    /// The coefficients for each of the variables. Must be the same length as the number of
    /// variables.
    pub coefficients: Vec<f64>,
}

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
) -> Vec<f64> {
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
    assert_eq!(solved.status(), HighsModelStatus::Optimal);

    solved.get_solution().columns().to_vec()
}

#[cfg(test)]
mod tests {
    use super::super::csv::*;
    use super::*;

    #[test]
    fn test_solve_highs() {
        let (var_names, var_defs) = read_variables(&get_variables_file_path()).unwrap();
        let constraints = read_constraints(&get_constraints_file_path(), &var_names).unwrap();

        let solution = solve_highs(&var_defs, &constraints, Sense::Maximise);
        assert_eq!(solution, &[0., 6., 0.5]);
    }
}
