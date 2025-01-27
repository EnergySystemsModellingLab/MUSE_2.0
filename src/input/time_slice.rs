//! Code for reading in time slice info from a CSV file.
#![allow(missing_docs)]
use crate::input::*;
use crate::time_slice::{TimeSliceID, TimeSliceInfo};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const TIME_SLICES_FILE_NAME: &str = "time_slices.csv";

/// A time slice record retrieved from a CSV file
#[derive(PartialEq, Debug, Deserialize)]
struct TimeSliceRaw {
    season: String,
    time_of_day: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    fraction: f64,
}

/// Get the specified `String` from `set` or insert if it doesn't exist
fn get_or_insert(value: String, set: &mut HashSet<Rc<str>>) -> Rc<str> {
    // Sadly there's no entry API for HashSets: https://github.com/rust-lang/rfcs/issues/1490
    match set.get(value.as_str()) {
        Some(value) => Rc::clone(value),
        None => {
            let value = Rc::from(value);
            set.insert(Rc::clone(&value));
            value
        }
    }
}

/// Read time slice information from an iterator of raw time slice records
fn read_time_slice_info_from_iter<I>(iter: I) -> Result<TimeSliceInfo>
where
    I: Iterator<Item = TimeSliceRaw>,
{
    let mut seasons: HashSet<Rc<str>> = HashSet::new();
    let mut times_of_day = HashSet::new();
    let mut fractions = HashMap::new();
    for time_slice in iter {
        let season = get_or_insert(time_slice.season, &mut seasons);
        let time_of_day = get_or_insert(time_slice.time_of_day, &mut times_of_day);
        let id = TimeSliceID {
            season,
            time_of_day,
        };

        ensure!(
            fractions.insert(id.clone(), time_slice.fraction).is_none(),
            "Duplicate time slice entry for {id}",
        );
    }

    // Validate data
    check_fractions_sum_to_one(fractions.values().cloned())
        .context("Invalid time slice fractions")?;

    Ok(TimeSliceInfo {
        seasons,
        times_of_day,
        fractions,
    })
}

/// Read time slices from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns a `TimeSliceInfo` struct or, if the file doesn't exist, a single time
/// slice covering the whole year (see `TimeSliceInfo::default()`).
pub fn read_time_slice_info(model_dir: &Path) -> Result<TimeSliceInfo> {
    let file_path = model_dir.join(TIME_SLICES_FILE_NAME);
    if !file_path.exists() {
        return Ok(TimeSliceInfo::default());
    }

    let time_slices_csv = read_csv(&file_path)?;
    read_time_slice_info_from_iter(time_slices_csv).with_context(|| input_err_msg(file_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    /// Create an example time slices file in dir_path
    fn create_time_slices_file(dir_path: &Path) {
        let file_path = dir_path.join(TIME_SLICES_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "season,time_of_day,fraction
winter,day,0.25
peak,night,0.25
summer,peak,0.25
autumn,evening,0.25"
        )
        .unwrap();
    }

    #[test]
    fn test_read_time_slice_info() {
        let dir = tempdir().unwrap();
        create_time_slices_file(dir.path());

        let info = read_time_slice_info(dir.path()).unwrap();
        assert_eq!(
            info,
            TimeSliceInfo {
                seasons: [
                    "winter".into(),
                    "peak".into(),
                    "summer".into(),
                    "autumn".into()
                ]
                .into_iter()
                .collect(),
                times_of_day: [
                    "day".into(),
                    "night".into(),
                    "peak".into(),
                    "evening".into()
                ]
                .into_iter()
                .collect(),
                fractions: [
                    (
                        TimeSliceID {
                            season: "winter".into(),
                            time_of_day: "day".into(),
                        },
                        0.25,
                    ),
                    (
                        TimeSliceID {
                            season: "peak".into(),
                            time_of_day: "night".into(),
                        },
                        0.25,
                    ),
                    (
                        TimeSliceID {
                            season: "summer".into(),
                            time_of_day: "peak".into(),
                        },
                        0.25,
                    ),
                    (
                        TimeSliceID {
                            season: "autumn".into(),
                            time_of_day: "evening".into(),
                        },
                        0.25,
                    ),
                ]
                .into_iter()
                .collect()
            }
        );
    }

    #[test]
    fn test_read_time_slice_info_non_existent() {
        let actual = read_time_slice_info(tempdir().unwrap().path());
        assert_eq!(actual.unwrap(), TimeSliceInfo::default());
    }
}
