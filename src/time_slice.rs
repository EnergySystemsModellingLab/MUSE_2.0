//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
#![allow(missing_docs)]
use crate::input::*;
use anyhow::{ensure, Context, Result};
use float_cmp::approx_eq;
use itertools::Itertools;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::iter;
use std::path::Path;
use std::rc::Rc;

const TIME_SLICES_FILE_NAME: &str = "time_slices.csv";

/// An ID describing season and time of day
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct TimeSliceID {
    /// The name of each season.
    pub season: Rc<str>,
    /// The name of each time slice within a day.
    pub time_of_day: Rc<str>,
}

impl Display for TimeSliceID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.season, self.time_of_day)
    }
}

/// Represents a time slice read from an input file, which can be all
#[derive(PartialEq, Debug)]
pub enum TimeSliceSelection {
    /// All year and all day
    Annual,
    /// Only applies to one season
    Season(Rc<str>),
    /// Only applies to a single time slice
    Single(TimeSliceID),
}

/// Information about the time slices in the simulation, including names and fractions
#[derive(PartialEq, Debug)]
pub struct TimeSliceInfo {
    /// Names of seasons
    pub seasons: HashSet<Rc<str>>,
    /// Names of times of day (e.g. "evening")
    pub times_of_day: HashSet<Rc<str>>,
    /// The fraction of the year that this combination of season and time of day occupies
    pub fractions: HashMap<TimeSliceID, f64>,
}

impl Default for TimeSliceInfo {
    /// The default `TimeSliceInfo` is a single time slice covering the whole year
    fn default() -> Self {
        let id = TimeSliceID {
            season: "all-year".into(),
            time_of_day: "all-day".into(),
        };
        let fractions = [(id.clone(), 1.0)].into_iter().collect();

        Self {
            seasons: [id.season].into_iter().collect(),
            times_of_day: [id.time_of_day].into_iter().collect(),
            fractions,
        }
    }
}

impl TimeSliceInfo {
    /// Get the `TimeSliceID` corresponding to the `time_slice`.
    ///
    /// `time_slice` must be in the form "season.time_of_day".
    pub fn get_time_slice_id_from_str(&self, time_slice: &str) -> Result<TimeSliceID> {
        let (season, time_of_day) = time_slice
            .split('.')
            .collect_tuple()
            .context("Time slice must be in the form season.time_of_day")?;
        let season = self
            .seasons
            .iter()
            .find(|item| item.eq_ignore_ascii_case(season))
            .with_context(|| format!("{} is not a known season", season))?;
        let time_of_day = self
            .times_of_day
            .iter()
            .find(|item| item.eq_ignore_ascii_case(time_of_day))
            .with_context(|| format!("{} is not a known time of day", time_of_day))?;

        Ok(TimeSliceID {
            season: Rc::clone(season),
            time_of_day: Rc::clone(time_of_day),
        })
    }

    /// Get a `TimeSliceSelection` from the specified string.
    ///
    /// If the string is empty, the default value is `TimeSliceSelection::Annual`.
    pub fn get_selection(&self, time_slice: &str) -> Result<TimeSliceSelection> {
        if time_slice.is_empty() || time_slice.eq_ignore_ascii_case("annual") {
            Ok(TimeSliceSelection::Annual)
        } else if time_slice.contains('.') {
            let time_slice = self.get_time_slice_id_from_str(time_slice)?;
            Ok(TimeSliceSelection::Single(time_slice))
        } else {
            let season = self.seasons.get_id(time_slice)?;
            Ok(TimeSliceSelection::Season(season))
        }
    }

    /// Iterate over all [`TimeSliceID`]s.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter(&self) -> impl Iterator<Item = &TimeSliceID> {
        self.fractions.keys()
    }

    /// Iterate over the subset of [`TimeSliceID`] indicated by `selection`.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter_selection<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
    ) -> Box<dyn Iterator<Item = &TimeSliceID> + 'a> {
        match selection {
            TimeSliceSelection::Annual => Box::new(self.iter()),
            TimeSliceSelection::Season(season) => {
                Box::new(self.iter().filter(move |ts| ts.season == *season))
            }
            TimeSliceSelection::Single(ts) => Box::new(iter::once(ts)),
        }
    }
}

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
    check_time_slice_fractions_sum_to_one(fractions.values().cloned())?;

    Ok(TimeSliceInfo {
        seasons,
        times_of_day,
        fractions,
    })
}

/// Refers to a particular aspect of a time slice
#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum TimeSliceLevel {
    #[string = "annual"]
    Annual,
    #[string = "season"]
    Season,
    #[string = "daynight"]
    DayNight,
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
pub fn read_time_slice_info(model_dir: &Path) -> TimeSliceInfo {
    let file_path = model_dir.join(TIME_SLICES_FILE_NAME);
    if !file_path.exists() {
        return TimeSliceInfo::default();
    }

    read_time_slice_info_from_iter(read_csv(&file_path)).unwrap_input_err(&file_path)
}

/// Check that time slice fractions sum to (approximately) one
fn check_time_slice_fractions_sum_to_one<I>(fractions: I) -> Result<()>
where
    I: Iterator<Item = f64>,
{
    let sum = fractions.sum();
    ensure!(
        approx_eq!(f64, sum, 1.0, epsilon = 1e-5),
        "Sum of time slice fractions does not equal one (actual: {sum})"
    );

    Ok(())
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

        let info = read_time_slice_info(dir.path());
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
        assert_eq!(actual, TimeSliceInfo::default());
    }

    #[test]
    fn test_iter_selection() {
        let slices = [
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            TimeSliceID {
                season: "summer".into(),
                time_of_day: "night".into(),
            },
        ];
        let ts_info = TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            fractions: [(slices[0].clone(), 0.5), (slices[1].clone(), 0.5)]
                .into_iter()
                .collect(),
        };

        assert_eq!(
            HashSet::<&TimeSliceID>::from_iter(ts_info.iter_selection(&TimeSliceSelection::Annual)),
            HashSet::from_iter(slices.iter())
        );
        itertools::assert_equal(
            ts_info.iter_selection(&TimeSliceSelection::Season("winter".into())),
            iter::once(&slices[0]),
        );
        let ts = ts_info.get_time_slice_id_from_str("summer.night").unwrap();
        itertools::assert_equal(
            ts_info.iter_selection(&TimeSliceSelection::Single(ts)),
            iter::once(&slices[1]),
        );
    }

    #[test]
    fn test_check_time_slice_fractions_sum_to_one() {
        // Single input, valid
        assert!(check_time_slice_fractions_sum_to_one([1.0].into_iter()).is_ok());

        // Multiple inputs, valid
        assert!(check_time_slice_fractions_sum_to_one([0.4, 0.6].into_iter()).is_ok());

        // Single input, invalid
        assert!(check_time_slice_fractions_sum_to_one([0.5].into_iter()).is_err());

        // Multiple inputs, invalid
        assert!(check_time_slice_fractions_sum_to_one([0.4, 0.3].into_iter()).is_err());

        // Edge cases
        assert!(check_time_slice_fractions_sum_to_one([f64::INFINITY].into_iter()).is_err());
        assert!(check_time_slice_fractions_sum_to_one([f64::NAN].into_iter()).is_err());
    }
}
