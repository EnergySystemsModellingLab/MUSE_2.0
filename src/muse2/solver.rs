//! Provides data structures and functions for performing optimisation.
pub use highs::Sense;
use highs::{HighsModelStatus, RowProblem};

/// The definition of a variable to be optimised.
///
/// The coefficients represent the multiplying factors in the objective function to maximise or
/// minimise, i.e. the Cs in:
///
/// f = c1*x1 + c2*x2 + ...
///
/// with x1, x2... taking values between min and max.
#[derive(PartialEq, Debug)]
pub struct VariableDefinition {
    /// The variable's name
    pub name: String,
    /// The variable's minimum value
    pub min: f64,
    /// The variable's maximum value
    pub max: f64,
    /// The coefficient of the variable in the objective
    pub coefficient: f64,
}

/// A constraint for an optimisation.
///
/// Each constraint adds an inequality equation to the problem to solve of the form:
///
/// min <= a1*x1 + a2*x2 + ... <= max
///
/// Often, constraints will impose only a min or a max value, with the other set to infinity or
/// minus infinity.
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
    use std::f64::INFINITY;

    use super::*;

    #[test]
    fn test_solve_highs() {
        let var_defs = [
            VariableDefinition {
                name: "x".to_string(),
                min: 0.,
                max: INFINITY,
                coefficient: 1.,
            },
            VariableDefinition {
                name: "y".to_string(),
                min: 0.,
                max: INFINITY,
                coefficient: 2.,
            },
            VariableDefinition {
                name: "z".to_string(),
                min: 0.,
                max: INFINITY,
                coefficient: 1.,
            },
        ];
        let constraints = [
            Constraint {
                min: -INFINITY,
                max: 6.,
                coefficients: vec![3., 1., 0.],
            },
            Constraint {
                min: -INFINITY,
                max: 7.,
                coefficients: vec![0., 1., 2.],
            },
        ];

        let solution = solve_highs(&var_defs, &constraints, Sense::Maximise).unwrap();
        assert_eq!(solution, &[0., 6., 0.5]);
    }
}
