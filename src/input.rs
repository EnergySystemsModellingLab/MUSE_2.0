//! Common routines for handling input data.
use itertools::Itertools;
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::HashMap;
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::rc::Rc;

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

/// Read a CSV file of items with IDs.
///
/// This is like `read_csv_grouped_by_id`, with the difference that it is to be used on the "main"
/// CSV file for a record type, so it assumes that all IDs encountered are valid.
pub fn read_csv_id_file<T>(file_path: &Path) -> HashMap<Rc<str>, T>
where
    T: HasID + DeserializeOwned,
{
    let mut map = HashMap::new();
    for record in read_csv::<T>(file_path) {
        let id = record.get_id();

        if map.contains_key(id) {
            input_panic(file_path, &format!("Duplicate ID found: {id}"));
        }

        map.insert(id.into(), record);
    }
    if map.is_empty() {
        input_panic(file_path, "CSV file is empty");
    }

    map
}

pub trait IntoIDMap<T> {
    fn into_id_map(self, file_path: &Path, ids: &HashSet<Rc<str>>) -> HashMap<Rc<str>, Vec<T>>;
}

impl<T, I> IntoIDMap<T> for I
where
    T: HasID,
    I: Iterator<Item = T>,
{
    /// Convert the specified iterator into a `HashMap` of the items grouped by ID.
    ///
    /// # Arguments
    ///
    /// `file_path` - The path to the CSV file this relates to
    /// `ids` - The set of valid IDs to check against.
    fn into_id_map(self, file_path: &Path, ids: &HashSet<Rc<str>>) -> HashMap<Rc<str>, Vec<T>> {
        let map = self.into_group_map_by(|elem| {
            let elem_id = elem.get_id();
            let id = match ids.get(elem_id) {
                None => input_panic(file_path, &format!("Unknown ID {elem_id} found")),
                Some(id) => id,
            };

            Rc::clone(id)
        });
        if map.is_empty() {
            input_panic(file_path, "CSV file is empty");
        }

        map
    }
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
) -> HashMap<Rc<str>, Vec<T>>
where
    T: HasID + DeserializeOwned,
{
    read_csv(file_path).into_id_map(file_path, ids)
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
        let records: Vec<Record> = read_csv_as_vec(&file_path);
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
    #[should_panic]
    fn test_read_csv_as_vec_empty() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "id,value\n");
        read_csv_as_vec::<Record>(&file_path);
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
            read_toml::<Record>(&file_path),
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
        let map = read_csv_grouped_by_id::<Record>(&file_path, &process_ids);
        assert_eq!(expected, map);
    }

    #[test]
    #[should_panic]
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
        read_csv_grouped_by_id::<Record>(&file_path, &process_ids);
    }
}
