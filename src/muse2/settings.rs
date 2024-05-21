//! Data structures representing settings files and a function for reading them.
use super::constraint::Constraint;
use super::variable_definition::VariableDefinition;
use std::path::Path;

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use toml;

/// Represents the contents of the entire settings file.
#[derive(Deserialize)]
struct Settings {
    /// The input files for the model
    input_files: InputFiles,
}

/// Represents the [input_files] section of the settings file.
#[derive(Deserialize)]
struct InputFiles {
    /// Path to CSV file containing variable definitions
    variables_file_path: PathBuf,
    /// Path to CSV file containing problem constraints
    constraints_file_path: PathBuf,
}

/// Read a settings file from the given path.
fn read_settings_file(path: &str) -> Result<Settings, toml::de::Error> {
    let data = fs::read_to_string(path).expect(&format!("Failed to read file: {}", path));
    toml::from_str(&data)
}

/// Read settings from disk.
///
/// # Arguments
///
/// * `settings_file_path`: The path to the settings TOML file (which includes paths to other
///                         configuration files)
pub fn read_settings(settings_file_path: &str) -> (Vec<VariableDefinition>, Vec<Constraint>) {
    let config = read_settings_file(settings_file_path).expect("Could not parse settings file");

    // For paths to other files listed in the settings file, if they're relative, we treat them as
    // relative to the folder the settings file is in.
    let settings_dir = Path::new(settings_file_path).parent().unwrap(); // will never fail

    let var_path = settings_dir.join(config.input_files.variables_file_path);
    let vars = VariableDefinition::vec_from_csv(&var_path)
        .expect("Failed to read variable definition file");

    let constraints_path = settings_dir.join(config.input_files.constraints_file_path);
    let constraints = Constraint::vec_from_csv(&constraints_path, &vars)
        .expect("Failed to read constraints file");

    (vars, constraints)
}
