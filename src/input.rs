//! Common routines for handling input data.
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Read the contents of a single CSV file into `vec`.
fn read_csv_as_vec_file<T: DeserializeOwned>(
    file_path: &Path,
    vec: &mut Vec<T>,
) -> InputResult<()> {
    let reader = csv::Reader::from_path(file_path).map_input_err(file_path)?;

    for record in reader.into_deserialize() {
        let record = record.map_input_err(file_path)?;
        vec.push(record);
    }

    if vec.is_empty() {
        Err(InputError::new(file_path, "CSV file cannot be empty"))?;
    }

    Ok(())
}

/// Read the contents of multiple CSV files in `dir_path` into `vec`.
fn read_csv_as_vec_dir<T: DeserializeOwned>(dir_path: &Path, vec: &mut Vec<T>) -> InputResult<()> {
    let dir = fs::read_dir(dir_path).map_input_err(dir_path)?;
    for entry in dir {
        let entry = entry.map_err(|err| InputError::new(dir_path, &err.to_string()))?;
        let path = entry.path();
        if path.extension() == Some(OsStr::new(".csv")) && path.is_file() {
            read_csv_as_vec_file(&path, vec)?;
        }
    }

    Ok(())
}

/// Read a series of type `T`s from one or more CSV files into a `Vec<T>`.
///
/// # Arguments
///
/// * `path_prefix` - Path to the CSV file
pub fn read_csv_as_vec<T: DeserializeOwned>(path_prefix: &Path) -> InputResult<Vec<T>> {
    match read_csv_as_vec_optional(path_prefix)? {
        None => {
            let name = path_prefix
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            Err(InputError::new(
                path_prefix,
                &format!("Could not find a file {name}.csv or a folder {name}"),
            ))
        }
        Some(vec) => Ok(vec),
    }
}

/// Read a series of type `T`s from one or more CSV files into a `Vec<T>`.
///
/// If no CSV files are found, Ok(None) is returned.
///
/// # Arguments
///
/// * `path_prefix` - Path to the CSV file
pub fn read_csv_as_vec_optional<T: DeserializeOwned>(
    path_prefix: &Path,
) -> InputResult<Option<Vec<T>>> {
    let dir_path = path_prefix;
    let file_path = {
        // Append extension
        let mut file_path_str: OsString = path_prefix.into();
        file_path_str.push(".csv");
        PathBuf::from(file_path_str)
    };

    let file_exists = file_path.is_file();
    let dir_exists = dir_path.is_dir();
    if file_exists && dir_exists {
        Err(InputError::new(
            path_prefix,
            "Cannot provide a CSV file and directory",
        ))?;
    }
    if !file_exists && !dir_exists {
        return Ok(None);
    }

    let mut vec = Vec::new();
    if file_exists {
        read_csv_as_vec_file(&file_path, &mut vec)?;
    } else {
        read_csv_as_vec_dir(dir_path, &mut vec)?;
    }

    Ok(Some(vec))
}

/// Parse a TOML file at the specified path.
///
/// # Arguments
///
/// * `file_path` - Path to the TOML file
pub fn read_toml<T: DeserializeOwned>(file_path: &Path) -> InputResult<T> {
    let toml_str = fs::read_to_string(file_path).map_input_err(file_path)?;
    toml::from_str(&toml_str).map_input_err(file_path)
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

/// Type alias for the result of input-related functions
pub type InputResult<T> = Result<T, InputError>;

/// A trait allowing us to add the map_input_err method to `Result`s
pub trait MapInputError<T> {
    /// Maps a `Result` with an arbitrary `Error` type to an `InputResult<T>`
    fn map_input_err(self, file_path: &Path) -> InputResult<T>;
}

impl<T, E: Error> MapInputError<T> for Result<T, E> {
    fn map_input_err(self, file_path: &Path) -> InputResult<T> {
        self.map_err(|err| InputError::new(file_path, &err.to_string()))
    }
}

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
    fn test_read_csv_as_vec() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "a,b\n1,hello\n2,world\n");
        let records: Vec<Record> = read_csv_as_vec(&file_path).unwrap();
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
    fn test_read_csv_as_vec_empty() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "a,b\n");
        assert!(read_csv_as_vec::<Record>(&file_path).is_err());
    }

    #[test]
    fn test_read_toml() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.toml");
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "a = 1\nb = \"hello\"").unwrap();
        }

        assert_eq!(
            read_toml::<Record>(&file_path).unwrap(),
            Record {
                a: 1,
                b: "hello".to_string()
            }
        );
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
