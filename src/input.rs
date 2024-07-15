//! Common routines for handling input data.
use serde::de::DeserializeOwned;
use std::error::Error;
use std::fmt;
use std::path::Path;

/// Read a series of type Ts from a CSV file into a Vec<T>.
///
/// # Arguments
///
/// * `csv_file_path`: Path to the CSV file
pub fn read_vec_from_csv<T: DeserializeOwned>(csv_file_path: &Path) -> Result<Vec<T>, InputError> {
    let mut reader = csv::Reader::from_path(csv_file_path)
        .map_err(|err| InputError::new(csv_file_path, &err.to_string()))?;

    let mut vec = Vec::new();
    for result in reader.deserialize() {
        let d: T = result.map_err(|err| InputError::new(csv_file_path, &err.to_string()))?;
        vec.push(d)
    }

    if vec.is_empty() {
        Err(InputError::new(csv_file_path, "CSV file cannot be empty"))?;
    }

    Ok(vec)
}

/// Indicates that an error occurred while loading a settings file.
#[derive(Debug, Clone)]
pub struct InputError {
    message: String,
}

impl InputError {
    pub fn new(file_path: &Path, message: &str) -> InputError {
        InputError {
            message: format!("Error reading {}: {}", file_path.to_string_lossy(), message),
        }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// This is needed so that InputError can be treated like standard errors are.
impl Error for InputError {}
