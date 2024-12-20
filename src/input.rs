//! Common routines for handling input data.
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::de::{Deserialize, DeserializeOwned, Deserializer};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub mod agent;
pub use agent::read_agents;
pub mod commodity;
pub use commodity::read_commodities;
pub mod region;
pub use region::read_regions;

/// Read a series of type `T`s from a CSV file.
///
/// # Arguments
///
/// * `file_path` - Path to the CSV file
pub fn read_csv<'a, T: DeserializeOwned + 'a>(
    file_path: &'a Path,
) -> Result<impl Iterator<Item = T> + 'a> {
    let vec = csv::Reader::from_path(file_path)
        .with_context(|| input_err_msg(file_path))?
        .into_deserialize()
        .process_results(|iter| iter.collect_vec())
        .with_context(|| input_err_msg(file_path))?;

    Ok(vec.into_iter())
}

/// Parse a TOML file at the specified path.
///
/// # Arguments
///
/// * `file_path` - Path to the TOML file
///
/// # Returns
///
/// * The deserialised TOML data or an error if the file could not be read or parsed.
pub fn read_toml<T: DeserializeOwned>(file_path: &Path) -> Result<T> {
    let toml_str = fs::read_to_string(file_path).with_context(|| input_err_msg(file_path))?;
    let toml_data = toml::from_str(&toml_str).with_context(|| input_err_msg(file_path))?;
    Ok(toml_data)
}

/// Read an f64, checking that it is between 0 and 1
pub fn deserialise_proportion_nonzero<'de, D>(deserialiser: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Deserialize::deserialize(deserialiser)?;
    if !(value > 0.0 && value <= 1.0) {
        Err(serde::de::Error::custom("Value must be > 0 and <= 1"))?
    }

    Ok(value)
}

/// Format an error message to include the file path. To be used with `anyhow::Context`.
pub fn input_err_msg<P: AsRef<Path>>(file_path: P) -> String {
    format!("Error reading {}", file_path.as_ref().to_string_lossy())
}

/// Indicates that the struct has an ID field
pub trait HasID {
    /// Get a string representation of the struct's ID
    fn get_id(&self) -> &str;
}

/// Implement the `HasID` trait for the given type, assuming it has a field called `id`
macro_rules! define_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.id
            }
        }
    };
}

pub(crate) use define_id_getter;

/// A data structure containing a set of IDs
pub trait IDCollection {
    /// Get the ID after checking that it exists this collection.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID to look up
    ///
    /// # Returns
    ///
    /// A copy of the `Rc<str>` in `self` or an error if not found.
    fn get_id(&self, id: &str) -> Result<Rc<str>>;
}

impl IDCollection for HashSet<Rc<str>> {
    fn get_id(&self, id: &str) -> Result<Rc<str>> {
        let id = self
            .get(id)
            .with_context(|| format!("Unknown ID {id} found"))?;
        Ok(Rc::clone(id))
    }
}

/// Read a CSV file of items with IDs.
///
/// This is like `read_csv_grouped_by_id`, with the difference that it is to be used on the "main"
/// CSV file for a record type, so it assumes that all IDs encountered are valid.
pub fn read_csv_id_file<T>(file_path: &Path) -> Result<HashMap<Rc<str>, T>>
where
    T: HasID + DeserializeOwned,
{
    fn fill_and_validate_map<T>(file_path: &Path) -> Result<HashMap<Rc<str>, T>>
    where
        T: HasID + DeserializeOwned,
    {
        let mut map = HashMap::new();
        for record in read_csv::<T>(file_path)? {
            let id = record.get_id();

            ensure!(!map.contains_key(id), "Duplicate ID found: {id}");

            map.insert(id.into(), record);
        }
        ensure!(!map.is_empty(), "CSV file is empty");

        Ok(map)
    }

    fill_and_validate_map(file_path).with_context(|| input_err_msg(file_path))
}

/// Trait for converting an iterator into a [`HashMap`] grouped by IDs.
pub trait IntoIDMap<T> {
    /// Convert into a [`HashMap`] grouped by IDs.
    fn into_id_map(self, ids: &HashSet<Rc<str>>) -> Result<HashMap<Rc<str>, Vec<T>>>;
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
    /// `ids` - The set of valid IDs to check against.
    fn into_id_map(self, ids: &HashSet<Rc<str>>) -> Result<HashMap<Rc<str>, Vec<T>>> {
        let map = self
            .map(|item| -> Result<_> {
                let id = ids.get_id(item.get_id())?;
                Ok((id, item))
            })
            .process_results(|iter| iter.into_group_map())?;

        ensure!(!map.is_empty(), "CSV file is empty");

        Ok(map)
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
    fn test_read_csv() {
        let dir = tempdir().unwrap();
        let file_path = create_csv_file(dir.path(), "id,value\nhello,1\nworld,2\n");
        let records: Vec<Record> = read_csv(&file_path).unwrap().collect();
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

        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "bad toml syntax").unwrap();
        }

        assert!(read_toml::<Record>(&file_path).is_err());
    }

    /// Deserialise value with deserialise_proportion_nonzero()
    fn deserialise_f64(value: f64) -> Result<f64, ValueError> {
        let deserialiser: F64Deserializer<ValueError> = value.into_deserializer();
        deserialise_proportion_nonzero(deserialiser)
    }

    #[test]
    fn test_deserialise_proportion_nonzero() {
        // Valid inputs
        assert_eq!(deserialise_f64(0.01), Ok(0.01));
        assert_eq!(deserialise_f64(0.5), Ok(0.5));
        assert_eq!(deserialise_f64(1.0), Ok(1.0));

        // Invalid inputs
        assert!(deserialise_f64(0.0).is_err());
        assert!(deserialise_f64(-1.0).is_err());
        assert!(deserialise_f64(2.0).is_err());
        assert!(deserialise_f64(f64::NAN).is_err());
        assert!(deserialise_f64(f64::INFINITY).is_err());
    }
}
