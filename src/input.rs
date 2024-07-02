//! Common routines for handling input data.
use nonempty_collections::*;
use serde::de::DeserializeOwned;
use std::error::Error;
use std::path::Path;

/// Read a series of type Ts from a CSV file into a Vec<T>.
///
/// # Arguments
///
/// * `csv_file_path`: Path to the CSV file
pub fn read_vec_from_csv<T: DeserializeOwned>(
    csv_file_path: &Path,
) -> Result<Vec<T>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_path(csv_file_path)?;
    let mut vec = Vec::new();
    for result in reader.deserialize() {
        let d: T = result?;
        vec.push(d)
    }

    Ok(vec)
}

/// Read a series of type Ts from a CSV file into a NEVec<T>. The CSV file cannot be empty.
///
/// # Arguments
///
/// * `csv_file_path`: Path to the CSV file
pub fn read_nevec_from_csv<T: DeserializeOwned>(
    csv_file_path: &Path,
) -> Result<NEVec<T>, Box<dyn Error>> {
    match NEVec::from_vec(read_vec_from_csv(csv_file_path)?) {
        None => Err(format!("Input file is empty: {:?}", csv_file_path))?,
        Some(nevec) => Ok(nevec),
    }
}
