//! Data structures representing settings files and a function for reading them.
use super::constraint::Constraint;
use super::variable_definition::VariableDefinition;
use std::path::Path;

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Represents the contents of the entire settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct Settings {
    /// The input files for the model
    input_files: InputFiles,
}

/// Represents the [input_files] section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct InputFiles {
    /// Path to CSV file containing variable definitions
    variables_file_path: PathBuf,
    /// Path to CSV file containing problem constraints
    constraints_file_path: PathBuf,
}

/// Read a settings file from the given path.
fn read_settings_file(path: &Path) -> Settings {
    let config_str = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("Failed to read file {:?}: {:?}", path, err));
    toml::from_str(&config_str)
        .unwrap_or_else(|err| panic!("Could not parse settings file: {:?}", err))
}

/// Read settings from disk.
///
/// # Arguments
///
/// * `settings_file_path`: The path to the settings TOML file (which includes paths to other
///                         configuration files)
pub fn read_settings(settings_file_path: &Path) -> (Vec<VariableDefinition>, Vec<Constraint>) {
    let config = read_settings_file(settings_file_path);

    // For paths to other files listed in the settings file, if they're relative, we treat them as
    // relative to the folder the settings file is in.
    let settings_dir = settings_file_path.parent().unwrap(); // will never fail

    // NB: If the path argument to join is absolute, it is passed through
    let var_path = settings_dir.join(config.input_files.variables_file_path);
    let vars = VariableDefinition::vec_from_csv(&var_path)
        .expect("Failed to read variable definition file");

    let constraints_path = settings_dir.join(config.input_files.constraints_file_path);
    let constraints = Constraint::vec_from_csv(&constraints_path, &vars)
        .expect("Failed to read constraints file");

    (vars, constraints)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::super::test_common::{
        get_example_constraints, get_example_path, get_example_variable_definitions,
    };
    use super::*;

    fn get_settings_file_path() -> PathBuf {
        get_example_path().join("settings.toml")
    }

    #[test]
    fn test_read_settings_file() {
        let settings = read_settings_file(&get_settings_file_path()).unwrap();

        assert_eq!(
            settings,
            Settings {
                input_files: InputFiles {
                    constraints_file_path: PathBuf::from_str("constraints.csv").unwrap(),
                    variables_file_path: PathBuf::from_str("variables.csv").unwrap(),
                }
            }
        )
    }

    #[test]
    fn test_read_settings() {
        // Check that the variable definitions and constraints load correctly. It's a bit gross that
        // we actually load the files given that we test this elsewhere, but mocking it would be a
        // faff.
        let (vars, constraints) = read_settings(&get_settings_file_path());
        assert_eq!(vars, &get_example_variable_definitions());
        assert_eq!(constraints, &get_example_constraints());
    }
}
