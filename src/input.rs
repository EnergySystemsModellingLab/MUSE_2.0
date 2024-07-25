//! Common routines for handling input data.
use itertools::Itertools;
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::Path;
use std::rc::Rc;

/// Read a series of type `T`s from a CSV file.
///
/// # Arguments
///
/// * `file_path` - Path to the CSV file
pub fn read_csv<'a, T: DeserializeOwned + 'a>(
    file_path: &'a Path,
) -> InputResult<impl Iterator<Item = InputResult<T>> + 'a> {
    Ok(csv::Reader::from_path(file_path)
        .map_input_err(file_path)?
        .into_deserialize()
        .map(|record| record.map_input_err(file_path)))
}

/// Read a series of type `T`s from a CSV file into a `Vec<T>`.
///
/// # Arguments
///
/// * `file_path` - Path to the CSV file
pub fn read_csv_as_vec<T: DeserializeOwned>(file_path: &Path) -> InputResult<Vec<T>> {
    let vec: Vec<T> = read_csv(file_path)?.try_collect()?;

    if vec.is_empty() {
        Err(InputError::new(file_path, "CSV file cannot be empty"))?;
    }

    Ok(vec)
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
pub trait MapInputError<'a, T> {
    /// Maps a `Result` with an arbitrary `Error` type to an `InputResult<T>`
    fn map_input_err(self, file_path: &'a Path) -> InputResult<T>;
}

impl<'a, T, E: Error> MapInputError<'a, T> for Result<T, E> {
    fn map_input_err(self, file_path: &'a Path) -> InputResult<T> {
        self.map_err(|err| InputError::new(file_path, &err.to_string()))
    }
}

/// Indicates that the struct has an ID field
pub trait HasID {
    /// Get a string representation of the struct's ID
    fn get_id(&self) -> &str;
}

/// Implement the `HasID` trait for the given type, assuming it has a field called `id`
#[macro_export]
macro_rules! define_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.id
            }
        }
    };
}

/// Read a CSV file of items with IDs also returned as a separate `HashSet`.
///
/// This is like `read_csv_grouped_by_id`, with the difference that it is to be used on the "main"
/// CSV file for a record type, so it assumes that all IDs encountered are valid. It returns a
/// `HashSet` of IDs along with the `HashMap`, so that it can be used to validate the IDs in other
/// files.
pub fn read_csv_id_file<T>(file_path: &Path) -> InputResult<(HashSet<Rc<str>>, HashMap<Rc<str>, T>)>
where
    T: HasID + DeserializeOwned,
{
    let mut map = HashMap::new();
    let mut ids = HashSet::new();
    for record in read_csv(file_path)? {
        let record: T = record?;
        let id = record.get_id();

        if map.contains_key(id) {
            Err(InputError::new(
                file_path,
                &format!("Duplicate ID found: {id}"),
            ))?;
        }

        let id = Rc::from(id);
        ids.insert(Rc::clone(&id));
        map.insert(id, record);
    }
    if ids.is_empty() {
        Err(InputError::new(file_path, "CSV file is empty"))?;
    }

    Ok((ids, map))
}

/// Convert the specified iterator into an iterator of pairs containing an ID + item.
pub fn into_id_pair<'a, T, U>(
    iter: T,
    file_path: &'a Path,
    ids: &'a HashSet<Rc<str>>,
) -> impl Iterator<Item = InputResult<(Rc<str>, U)>> + 'a
where
    T: Iterator<Item = InputResult<U>> + 'a,
    U: HasID + DeserializeOwned,
{
    iter.map(|elem| {
        let elem: U = elem?;
        let elem_id = elem.get_id();
        let id = match ids.get(elem_id) {
            None => Err(InputError::new(
                file_path,
                &format!("Unknown ID {elem_id} found"),
            ))?,
            Some(id) => Rc::clone(id),
        };

        Ok((id, elem))
    })
}

/// Read a CSV file, grouping the entries by ID
///
/// # Arguments
///
/// * `file_path` - Path to CSV file
/// * `ids` - All possible IDs that will be encountered
///
/// # Returns
///
/// A HashMap with ID as a key and a vector of CSV data as a value.
pub fn read_csv_grouped_by_id<T>(
    file_path: &Path,
    ids: &HashSet<Rc<str>>,
) -> InputResult<HashMap<Rc<str>, Vec<T>>>
where
    T: HasID + DeserializeOwned,
{
    // process_results checks for errors across the iterator
    let map = into_id_pair(read_csv(file_path)?, file_path, ids)
        .process_results(|iter| iter.into_group_map())?;
    if map.is_empty() {
        Err(InputError::new(file_path, "CSV file is empty"))?;
    }

    Ok(map)
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
        id: String,
        value: u32,
    }

    impl HasID for Record {
        fn get_id(&self) -> &str {
            &self.id
        }
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
        let file_path = create_csv_file(dir.path(), "id,value\nhello,1\nworld,2\n");
        let records: Vec<Record> = read_csv_as_vec(&file_path).unwrap();
        assert_eq!(
            records,
            &[
                Record {
                    id: "hello".to_string(),
                    value: 1,
                },
                Record {
                    id: "world".to_string(),
                    value: 2,
                }
            ]
        );
    }

    /// Empty CSV files should yield an error
    #[test]
    fn test_read_csv_as_vec_empty() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "id,value\n");
        assert!(read_csv_as_vec::<Record>(&file_path).is_err());
    }

    #[test]
    fn test_read_toml() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.toml");
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "id = \"hello\"\nvalue = 1").unwrap();
        }

        assert_eq!(
            read_toml::<Record>(&file_path).unwrap(),
            Record {
                id: "hello".to_string(),
                value: 1,
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

    fn create_ids() -> HashSet<Rc<str>> {
        HashSet::from(["A".into(), "B".into()])
    }

    #[test]
    fn test_read_csv_grouped_by_id() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "id,value\nA,1\nB,2\nA,3").unwrap();
        }

        let expected = HashMap::from([
            (
                "A".into(),
                vec![
                    Record {
                        id: "A".to_string(),
                        value: 1,
                    },
                    Record {
                        id: "A".to_string(),
                        value: 3,
                    },
                ],
            ),
            (
                "B".into(),
                vec![Record {
                    id: "B".to_string(),
                    value: 2,
                }],
            ),
        ]);
        let process_ids = create_ids();
        let file_path = dir.path().join("data.csv");
        let map = read_csv_grouped_by_id::<Record>(&file_path, &process_ids).unwrap();
        assert_eq!(expected, map);
    }

    #[test]
    fn test_read_csv_grouped_by_id_duplicate() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();

            // NB: Process ID "C" isn't valid
            writeln!(file, "process_id,value\nA,1\nB,2\nC,3").unwrap();
        }

        // Check that it fails if a non-existent process ID is provided
        let process_ids = create_ids();
        assert!(read_csv_grouped_by_id::<Record>(&file_path, &process_ids).is_err());
    }
}
