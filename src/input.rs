//! Common routines for handling input data.
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
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

/// Read an f64, checking that it is between 0 and 1
pub fn deserialise_proportion<'de, D>(deserialiser: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserialiser)?;
    if !(0.0..=1.0).contains(&value) {
        Err(serde::de::Error::custom("Value is not between 0 and 1"))?
    }

    Ok(value)
}

#[derive(PartialEq, Debug, SerializeLabeledStringEnum, DeserializeLabeledStringEnum)]
pub enum LimitType {
    #[string = "lo"]
    LowerBound,
    #[string = "up"]
    UpperBound,
    #[string = "fx"]
    Equality,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::value::{Error as ValueError, F64Deserializer};
    use serde::de::IntoDeserializer;
    use serde::Deserialize;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Record {
        a: u32,
        b: String,
    }

    /// Create an example CSV file in dir_path
    fn create_csv_file(dir_path: &Path, contents: &str) -> PathBuf {
        let file_path = dir_path.join("test.csv");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "{}", contents).unwrap();
        file_path
    }

    /// Test a normal read
    #[test]
    fn test_read_vec_from_csv() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "a,b\n1,hello\n2,world\n");
        let records: Vec<Record> = read_vec_from_csv(&file_path).unwrap();
        assert_eq!(
            records,
            &[
                Record {
                    a: 1,
                    b: "hello".to_string()
                },
                Record {
                    a: 2,
                    b: "world".to_string()
                }
            ]
        );
    }

    /// Empty CSV files should yield an error
    #[test]
    fn test_read_vec_from_csv_empty() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "a,b\n");
        assert!(read_vec_from_csv::<Record>(&file_path).is_err());
    }

    /// Deserialise value with deserialise_proportion()
    fn deserialise_f64(value: f64) -> Result<f64, ValueError> {
        let deserialiser: F64Deserializer<ValueError> = value.into_deserializer();
        deserialise_proportion(deserialiser)
    }

    #[test]
    fn test_deserialise_proportion() {
        // Valid inputs
        assert_eq!(deserialise_f64(0.0), Ok(0.0));
        assert_eq!(deserialise_f64(0.5), Ok(0.5));
        assert_eq!(deserialise_f64(1.0), Ok(1.0));

        // Invalid inputs
        assert!(deserialise_f64(-1.0).is_err());
        assert!(deserialise_f64(2.0).is_err());
        assert!(deserialise_f64(f64::NAN).is_err());
        assert!(deserialise_f64(f64::INFINITY).is_err());
    }
}
