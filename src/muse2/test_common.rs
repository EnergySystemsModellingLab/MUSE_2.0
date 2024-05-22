//! Common functionality used by tests.
use super::constraint::Constraint;
use super::variable_definition::VariableDefinition;
use std::f64::INFINITY;
use std::path::{Path, PathBuf};

/// Get the path to the example folder in this repository.
pub fn get_example_path() -> PathBuf {
    Path::new(file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("example")
}

pub fn get_example_variable_definitions() -> [VariableDefinition; 3] {
    [
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
    ]
}

pub fn get_example_constraints() -> [Constraint; 2] {
    [
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
    ]
}
