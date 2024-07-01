//! Common routines for handling input data.
use serde::de::DeserializeOwned;
use std::error::Error;
use std::path::Path;

/// Read a series of type Ts from a CSV file into a Vec<T>.
pub fn read_vec_from_csv<T: DeserializeOwned>(
    csv_file_path: &Path,
) -> Result<Vec<T>, Box<dyn Error>> {
    let mut reader = csv::Reader::from_path(csv_file_path)?;
    let mut data = Vec::new();
    for result in reader.deserialize() {
        let d: T = result?;
        data.push(d)
    }
    Ok(data)
}
