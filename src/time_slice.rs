//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::input::*;
use float_cmp::approx_eq;
use log::warn;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Display;
use std::path::Path;
use std::rc::Rc;

const TIME_SLICES_FILE_NAME: &str = "time_slices.csv";

pub enum TimeSliceSelection {
    AllYear,
    Some(TimeSliceID),
}

impl TimeSliceSelection {
    pub fn contains(&self, time_slice: &TimeSliceID) -> bool {
        match self {
            Self::AllYear => true,
            Self::Some(id) => id == time_slice,
        }
    }
}

#[derive(PartialEq, Debug, Deserialize)]
struct TimeSliceRaw {
    /// Which season (in the year)
    pub season: String,
    /// Time of day, as a category (e.g. night, day etc.)
    pub time_of_day: String,
    /// The fraction of the year that this combination of season and time of day occupies
    pub fraction: f64,
}

/// Represents a single time slice in the simulation
#[derive(PartialEq, Eq, Hash, Debug, Clone, Deserialize)]
pub struct TimeSliceID {
    /// Which season (in the year)
    pub season: Rc<str>,
    /// Time of day, as a category (e.g. night, day etc.)
    pub time_of_day: Rc<str>,
}

impl Display for TimeSliceID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.season, self.time_of_day)
    }
}

#[derive(Debug, PartialEq)]
pub struct TimeSlice {
    pub id: TimeSliceID,

    // The total fraction of the year occupied by this time slice and all the ones before it
    pub cumulative_fraction: f64,
}

#[derive(Debug, PartialEq)]
pub struct TimeSliceInfo {
    pub ids: HashSet<TimeSliceID>,
    pub fractions: Vec<TimeSlice>,
}

/// Read time slices from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// Information about time slices. If no time slice file is provided, a single time slice is used
/// for the whole year.
pub fn read_time_slices(model_dir: &Path) -> TimeSliceInfo {
    let file_path = model_dir.join(TIME_SLICES_FILE_NAME);
    if !file_path.exists() {
        // If there is no time slice file provided, use a default time slice which covers the
        // whole year and the whole day
        warn!("No time slices CSV file provided; using a single time slice");

        let id = TimeSliceID {
            season: "all-year".into(),
            time_of_day: "all-day".into(),
        };
        let ids = HashSet::from_iter([id.clone()]);
        let fractions = vec![TimeSlice {
            id,
            cumulative_fraction: 1.0,
        }];

        return TimeSliceInfo { ids, fractions };
    }

    let mut cum_sum = 0.0; // cumulative sum of fractions
    let mut ids = HashSet::new();
    let mut fractions = Vec::new();
    for record in read_csv::<TimeSliceRaw>(&file_path) {
        let id = TimeSliceID {
            season: record.season.into(),
            time_of_day: record.time_of_day.into(),
        };

        if !ids.insert(id.clone()) {
            input_panic(
                &file_path,
                &format!("Duplicate entry for time slice found: {}", id),
            );
        }

        if record.fraction <= 0.0 || record.fraction > 1.0 {
            input_panic(&file_path, "fraction must be > 0.0 and <= 1.0");
        }

        cum_sum += record.fraction;
        fractions.push(TimeSlice {
            id,
            cumulative_fraction: cum_sum,
        });
    }

    if ids.is_empty() {
        input_panic(&file_path, "CSV file cannot be empty");
    }

    if !approx_eq!(f64, cum_sum, 1.0, epsilon = 1e-5) {
        input_panic(
            &file_path,
            &format!(
                "Sum of time slice fractions does not equal one (actual: {})",
                cum_sum
            ),
        )
    }

    // Round the value to 1.0 for clarity when outputting these values
    fractions.last_mut().unwrap().cumulative_fraction = 1.0;

    TimeSliceInfo { ids, fractions }
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
    fn test_read_time_slices() {
        let dir = tempdir().unwrap();
        create_time_slices_file(dir.path());
        let time_slices = read_time_slices(dir.path());
        let fractions = [
            TimeSlice {
                id: TimeSliceID {
                    season: "winter".into(),
                    time_of_day: "day".into(),
                },
                cumulative_fraction: 0.25,
            },
            TimeSlice {
                id: TimeSliceID {
                    season: "peak".into(),
                    time_of_day: "night".into(),
                },
                cumulative_fraction: 0.5,
            },
            TimeSlice {
                id: TimeSliceID {
                    season: "summer".into(),
                    time_of_day: "peak".into(),
                },
                cumulative_fraction: 0.75,
            },
            TimeSlice {
                id: TimeSliceID {
                    season: "autumn".into(),
                    time_of_day: "evening".into(),
                },
                cumulative_fraction: 1.0,
            },
        ];
        let ids = HashSet::from_iter(fractions.iter().map(|fraction| fraction.id.clone()));
        assert_eq!(time_slices.ids, ids);
        assert_eq!(time_slices.fractions, fractions);
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

        read_time_slices(dir.path());
    }
}
