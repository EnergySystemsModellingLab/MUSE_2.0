//! Provides functionality for reading data from CSV files.
use super::solver::{Constraint, VariableDefinition};
use itertools::izip;
use polars::prelude::*;

/// Read a CSV file from the specified path.
pub fn read_csv(path: &str) -> Result<DataFrame, PolarsError> {
    CsvReader::from_path(path)?.has_header(true).finish()
}

/// Read variable definitions from the specified path.
///
/// Returns a vector of variable names and a vector of variable definitions or an error.
///
/// # Arguments:
///
/// * `path`: The path to the variable definitions CSV file
pub fn read_variables(path: &str) -> Result<(Vec<String>, Vec<VariableDefinition>), PolarsError> {
    // Read in the data
    let df = read_csv(path)?;

    // Extract the relevant columns from the dataframe
    let cols = df.columns(["name", "coefficient", "min", "max"])?;
    let names = cols[0].str()?.into_no_null_iter();
    let coeffs = cols[1].f64()?.into_no_null_iter();
    let mins = cols[2].f64()?.into_no_null_iter();
    let maxes = cols[3].f64()?.into_no_null_iter();

    // Create a vector of VariableDefinitions
    let mut vars = Vec::with_capacity(df.shape().0);
    let mut var_names = Vec::with_capacity(df.shape().0);
    for (name, coeff, min, max) in izip!(names, coeffs, mins, maxes) {
        let name = name.to_string();
        if var_names.contains(&name) {
            panic!("Variable {} defined more than once", name);
        }
        var_names.push(name);

        vars.push(VariableDefinition {
            min,
            max,
            coefficient: coeff,
        })
    }
    Ok((var_names, vars))
}

/// Read constraints from the specified path.
///
/// Returns a vector of constraints or an error.
///
/// # Arguments:
///
/// * `path`: The path to the constrains CSV file
pub fn read_constraints(path: &str, var_names: &[String]) -> Result<Vec<Constraint>, PolarsError> {
    // Read in the data
    let df = read_csv(path)?;

    // Get min and max values for constraint
    let mins = df.column("min")?.f64()?.into_no_null_iter();
    let maxes = df.column("max")?.f64()?.into_no_null_iter();

    // Get coefficients
    let mut coeff_cols: Vec<Vec<f64>> = Vec::with_capacity(var_names.len());
    for var_name in var_names.iter() {
        let col_name = format!("coeff_{}", var_name);
        coeff_cols.push(df.column(&col_name)?.f64()?.into_no_null_iter().collect());
    }

    // Create vector of constraints
    let mut constraints = Vec::with_capacity(df.shape().0);
    for (i, (min, max)) in mins.zip(maxes).enumerate() {
        // Get variable coefficients
        let mut coeffs = Vec::with_capacity(var_names.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::INFINITY;
    use std::path::{Path, PathBuf};

    /// Get the path to the example folder in this repository.
    fn get_example_path() -> PathBuf {
        Path::new(file!())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("example")
    }

    /// Get the path to the example variables.csv file.
    fn get_variables_file_path() -> String {
        get_example_path()
            .join("variables.csv")
            .to_str()
            .unwrap()
            .to_owned()
    }

    /// Get the path to the example constraints.csv file.
    fn get_constraints_file_path() -> String {
        get_example_path()
            .join("constraints.csv")
            .to_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn test_read_variables() {
        let (var_names, var_defs) = read_variables(&get_variables_file_path()).unwrap();
        assert_eq!(var_names, &["x", "y", "z"]);
        assert_eq!(
            var_defs,
            &[
                VariableDefinition {
                    min: 0.,
                    max: INFINITY,
                    coefficient: 1.
                },
                VariableDefinition {
                    min: 0.,
                    max: INFINITY,
                    coefficient: 2.
                },
                VariableDefinition {
                    min: 0.,
                    max: INFINITY,
                    coefficient: 1.
                }
            ]
        );
    }

    #[test]
    fn test_read_constraints() {
        let (var_names, _) = read_variables(&get_variables_file_path()).unwrap();
        let constraints = read_constraints(&get_constraints_file_path(), &var_names).unwrap();
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
