//! Provides functionality for optimisation constraints.
use super::variable_definition::VariableDefinition;
use polars::prelude::*;

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

impl Constraint {
    /// Read constraints from the specified path.
    ///
    /// Returns a vector of constraints or an error.
    ///
    /// # Arguments:
    ///
    /// * `path`: The path to the constraints CSV file
    pub fn vec_from_csv(
        path: &str,
        vars: &[VariableDefinition],
    ) -> Result<Vec<Constraint>, PolarsError> {
        // Read in the data
        let df = CsvReader::from_path(path)?.has_header(true).finish()?;

        // Get min and max values for constraint
        let mins = df.column("min")?.f64()?.into_no_null_iter();
        let maxes = df.column("max")?.f64()?.into_no_null_iter();

        // Get coefficients
        let mut coeff_cols: Vec<Vec<f64>> = Vec::with_capacity(vars.len());
        for var in vars.iter() {
            let col_name = format!("coeff_{}", var.name);
            coeff_cols.push(df.column(&col_name)?.f64()?.into_no_null_iter().collect());
        }

        // Create vector of constraints
        let mut constraints = Vec::with_capacity(df.shape().0);
        for (i, (min, max)) in mins.zip(maxes).enumerate() {
            // Get variable coefficients
            let mut coeffs = Vec::with_capacity(vars.len());
            for col in coeff_cols.iter() {
                coeffs.push(col[i]);
            }

            constraints.push(Constraint {
                min,
                max,
                coefficients: coeffs,
            })
        }

        Ok(constraints)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_common::get_example_path;
    use super::*;
    use std::f64::INFINITY;

    /// Get the path to the example constraints.csv file.
    fn get_constraints_file_path() -> String {
        get_example_path()
            .join("constraints.csv")
            .to_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn test_read_constraints() {
        let vars = [
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
        let constraints = Constraint::vec_from_csv(&get_constraints_file_path(), &vars).unwrap();
        assert_eq!(
            constraints,
            &[
                Constraint {
                    min: -INFINITY,
                    max: 6.,
                    coefficients: vec![3., 1., 0.]
                },
                Constraint {
                    min: -INFINITY,
                    max: 7.,
                    coefficients: vec![0., 1., 2.]
                }
            ]
        );
    }
}
