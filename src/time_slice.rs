//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::input::{deserialise_proportion_nonzero, input_panic, read_csv};
use crate::model::MODEL_FILE_NAME;

use float_cmp::approx_eq;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const TIME_SLICES_FILE_NAME: &str = "time_slices.csv";

#[derive(PartialEq, Debug)]
pub struct TimeSliceID {
    pub season: Rc<str>,
    pub time_of_day: Rc<str>,
}

#[derive(PartialEq, Debug)]
pub enum TimeSliceSelection {
    AllTimeSlices,
    SingleTimeSlice(TimeSliceID),
}

/// Ordered list of seasons and times of day for time slices
#[derive(Debug, Deserialize, PartialEq)]
pub struct TimeSliceDefinitions {
    pub seasons: Vec<Rc<str>>,
    pub times_of_day: Vec<Rc<str>>,
}

impl TimeSliceDefinitions {
    pub fn get_time_slice_id(
        &self,
        file_path: &Path,
        season: &str,
        time_of_day: &str,
    ) -> TimeSliceID {
        let season = self
            .seasons
            .iter()
            .find(|item| item.eq_ignore_ascii_case(season))
            .unwrap_or_else(|| {
                input_panic(
                    file_path,
                    &format!("Season {} not listed in {}", season, MODEL_FILE_NAME),
                )
            });
        let time_of_day = self
            .times_of_day
            .iter()
            .find(|item| item.eq_ignore_ascii_case(time_of_day))
            .unwrap_or_else(|| {
                input_panic(
                    file_path,
                    &format!(
                        "Time of day {} not listed in {}",
                        time_of_day, MODEL_FILE_NAME
                    ),
                )
            });

        TimeSliceID {
            season: Rc::clone(season),
            time_of_day: Rc::clone(time_of_day),
        }
    }

    pub fn get_time_slice_id_from_str(&self, file_path: &Path, time_slice: &str) -> TimeSliceID {
        let (season, time_of_day) = time_slice.split('.').collect_tuple().unwrap_or_else(|| {
            input_panic(
                file_path,
                "time_slice must be in the form season.time_of_day",
            )
        });

        self.get_time_slice_id(file_path, season, time_of_day)
    }
}

#[derive(PartialEq, Debug, Deserialize)]
struct TimeSliceRaw {
    /// Which season (in the year)
    season: String,
    /// Time of day, as a category (e.g. night, day etc.)
    time_of_day: String,
    /// The fraction of the year that this combination of season and time of day occupies
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    fraction: f64,
}

/// Represents a single time slice in the simulation
#[derive(PartialEq, Debug)]
pub struct TimeSlice {
    pub id: TimeSliceID,

    /// The fraction of the year that this combination of season and time of day occupies
    pub fraction: f64,
}

fn get_indexes_map<'a, I>(iter: I) -> HashMap<Rc<str>, usize>
where
    I: Iterator<Item = &'a Rc<str>>,
{
    iter.enumerate()
        .map(|(idx, value)| (Rc::clone(value), idx))
        .collect()
}

fn read_time_slices_from_iter<I>(
    iter: I,
    file_path: &Path,
    definitions: &TimeSliceDefinitions,
) -> Vec<TimeSlice>
where
    I: Iterator<Item = TimeSliceRaw>,
{
    let season_indexes = get_indexes_map(definitions.seasons.iter());
    let time_of_day_indexes = get_indexes_map(definitions.times_of_day.iter());

    macro_rules! ts_to_tuple {
        ($ts:ident) => {
            (
                season_indexes.get(&$ts.id.season).unwrap(),
                time_of_day_indexes.get(&$ts.id.time_of_day).unwrap(),
            )
        };
    }

    iter.map(|time_slice| {
        let id =
            definitions.get_time_slice_id(file_path, &time_slice.season, &time_slice.time_of_day);

        TimeSlice {
            id,
            fraction: time_slice.fraction,
        }
    })
    .sorted_by(|ts1, ts2| ts_to_tuple!(ts1).cmp(&ts_to_tuple!(ts2)))
    .collect()
}

/// Read time slices from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns either `Some(Vec<TimeSlice>)` with the parsed time slices or, if the time
/// slice CSV file does not exist, `None` will be returned.
pub fn read_time_slices(model_dir: &Path, definitions: &TimeSliceDefinitions) -> Vec<TimeSlice> {
    let file_path = model_dir.join(TIME_SLICES_FILE_NAME);
    let time_slices = read_time_slices_from_iter(read_csv(&file_path), &file_path, definitions);
    check_time_slice_fractions_sum_to_one(&file_path, &time_slices);

    time_slices
}

/// Check that time slice fractions sum to (approximately) one
fn check_time_slice_fractions_sum_to_one(file_path: &Path, time_slices: &[TimeSlice]) {
    let sum = time_slices.iter().map(|ts| ts.fraction).sum();
    if !approx_eq!(f64, sum, 1.0, epsilon = 1e-5) {
        input_panic(
            file_path,
            &format!(
                "Sum of time slice fractions does not equal one (actual: {})",
                sum
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::panic::catch_unwind;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    macro_rules! ts {
        ($fraction:expr) => {
            TimeSlice {
                id: TimeSliceID {
                    season: "summer".into(),
                    time_of_day: "day".into(),
                },
                fraction: $fraction,
            }
        };
    }

    /// Create an example time slices file in dir_path
    fn create_time_slices_file(dir_path: &Path) {
        let file_path = dir_path.join(TIME_SLICES_FILE_NAME);
        let mut file = File::create(file_path).unwrap();

        // Note these are out of order. Should be sorted by read_time_slices().
        writeln!(
            file,
            "season,time_of_day,fraction
peak,night,0.25
summer,peak,0.25
autumn,evening,0.25
winter,day,0.25"
        )
        .unwrap();
    }

    #[test]
    fn test_read_time_slices() {
        let dir = tempdir().unwrap();
        create_time_slices_file(dir.path());
        let definitions = TimeSliceDefinitions {
            seasons: vec![
                "winter".into(),
                "peak".into(),
                "summer".into(),
                "autumn".into(),
            ],
            times_of_day: vec![
                "day".into(),
                "night".into(),
                "peak".into(),
                "evening".into(),
            ],
        };
        let time_slices = read_time_slices(dir.path(), &definitions);
        assert_eq!(
            time_slices,
            &[
                TimeSlice {
                    id: TimeSliceID {
                        season: "winter".into(),
                        time_of_day: "day".into()
                    },
                    fraction: 0.25
                },
                TimeSlice {
                    id: TimeSliceID {
                        season: "peak".into(),
                        time_of_day: "night".into(),
                    },
                    fraction: 0.25
                },
                TimeSlice {
                    id: TimeSliceID {
                        season: "summer".into(),
                        time_of_day: "peak".into(),
                    },
                    fraction: 0.25
                },
                TimeSlice {
                    id: TimeSliceID {
                        season: "autumn".into(),
                        time_of_day: "evening".into(),
                    },
                    fraction: 0.25
                }
            ]
        )
    }

    #[test]
    #[should_panic]
    fn test_read_time_slices_empty() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("time_slices.csv");
        {
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "season,time_of_day,fraction").unwrap();
        }

        let definitions = TimeSliceDefinitions {
            seasons: vec![
                "winter".into(),
                "peak".into(),
                "summer".into(),
                "autumn".into(),
            ],
            times_of_day: vec![
                "day".into(),
                "night".into(),
                "peak".into(),
                "evening".into(),
            ],
        };
        read_time_slices(dir.path(), &definitions);
    }

    #[test]
    fn test_check_time_slice_fractions_sum_to_one_ok() {
        let p = PathBuf::new();

        // Single input, valid
        check_time_slice_fractions_sum_to_one(&p, &[ts!(1.0)]);

        // Multiple inputs, valid
        check_time_slice_fractions_sum_to_one(&p, &[ts!(0.4), ts!(0.6)]);
    }

    #[test]
    fn test_check_time_slice_fractions_sum_to_one_err() {
        let p = PathBuf::new();

        macro_rules! check_panic {
            ($ts:expr) => {
                assert!(catch_unwind(|| check_time_slice_fractions_sum_to_one(&p, $ts)).is_err())
            };
        }

        // Single input, invalid
        check_panic!(&[ts!(0.5)]);

        // Multiple inputs, invalid
        check_panic!(&[ts!(0.4), ts!(0.3)]);

        // Edge cases
        check_panic!(&[ts!(f64::INFINITY)]);
        check_panic!(&[ts!(f64::NAN)]);
    }
}
