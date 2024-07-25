//! Common routines for handling input data.
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::error::Error;
use std::fs;
use std::path::Path;

/// Read a series of type `T`s from a CSV file.
///
/// # Arguments
///
/// * `file_path` - Path to the CSV file
pub fn read_csv<'a, T: DeserializeOwned + 'a>(file_path: &'a Path) -> impl Iterator<Item = T> + 'a {
    csv::Reader::from_path(file_path)
        .unwrap_input_err(file_path)
        .into_deserialize()
        .unwrap_input_err(file_path)
}

/// Read a series of type `T`s from a CSV file into a `Vec<T>`.
///
/// # Arguments
///
/// * `file_path` - Path to the CSV file
pub fn read_csv_as_vec<T: DeserializeOwned>(file_path: &Path) -> Vec<T> {
    let vec: Vec<T> = read_csv(file_path).collect();

    if vec.is_empty() {
        input_panic(file_path, "CSV file cannot be empty");
    }

    vec
}

/// Parse a TOML file at the specified path.
///
/// # Arguments
///
/// * `file_path` - Path to the TOML file
pub fn read_toml<T: DeserializeOwned>(file_path: &Path) -> T {
    let toml_str = fs::read_to_string(file_path).unwrap_input_err(file_path);
    toml::from_str(&toml_str).unwrap_input_err(file_path)
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

/// Panic including the path to the file along with the message
pub fn input_panic(file_path: &Path, msg: &str) -> ! {
    panic!("Error reading {}: {}", file_path.to_string_lossy(), msg);
}

/// A trait allowing us to add the unwrap_input_err method to `Result`s
pub trait UnwrapInputError<T> {
    /// Maps a `Result` with an arbitrary `Error` type to an `T`
    fn unwrap_input_err(self, file_path: &Path) -> T;
}

impl<T, E: Error> UnwrapInputError<T> for Result<T, E> {
    fn unwrap_input_err(self, file_path: &Path) -> T {
        match self {
            Ok(value) => value,
            Err(err) => input_panic(file_path, &err.to_string()),
        }
    }
}

pub trait UnwrapInputErrorIter<T> {
    /// Maps an `Iterator` of `Result`s with an arbitrary `Error` type to an `Iterator<Item = T>`
    fn unwrap_input_err(self, file_path: &Path) -> impl Iterator<Item = T>;
}

impl<T, E, I> UnwrapInputErrorIter<T> for I
where
    E: Error,
    I: Iterator<Item = Result<T, E>>,
{
    fn unwrap_input_err(self, file_path: &Path) -> impl Iterator<Item = T> {
        self.map(|x| x.unwrap_input_err(file_path))
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
        let records: Vec<Record> = read_csv_as_vec(&file_path);
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
    #[should_panic]
    fn test_read_csv_as_vec_empty() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "a,b\n");
        read_csv_as_vec::<Record>(&file_path);
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
            read_toml::<Record>(&file_path),
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
