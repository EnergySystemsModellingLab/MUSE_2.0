//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::input::read_nevec_from_csv;
use float_cmp::approx_eq;
use nonempty_collections::*;
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

/// Represents a single time slice in the simulation
#[derive(PartialEq, Debug, Deserialize)]
pub struct TimeSlice {
    /// Which season (in the year)
    pub season: String,
    /// Time of day, as a category (e.g. night, day etc.)
    pub time_of_day: String,
    /// The fraction of the year that this combination of season and time of day occupies
    pub fraction: f64,
}

/// Read time slices from a CSV file
pub fn read_time_slices(csv_file_path: &Path) -> Result<NEVec<TimeSlice>, Box<dyn Error>> {
    let time_slices: NEVec<TimeSlice> = read_nevec_from_csv(csv_file_path)?;

    check_time_slice_fractions_in_range(time_slices.as_nonempty_slice())?;
    check_time_slice_fractions_sum_to_one(time_slices.as_nonempty_slice())?;
    Ok(time_slices)
}

/// Check that time slice fractions are all in the range 0 to 1
fn check_time_slice_fractions_in_range(
    time_slices: NESlice<TimeSlice>,
) -> Result<(), &'static str> {
    if !time_slices
        .iter()
        .all(|ts| ts.fraction >= 0.0 && ts.fraction <= 1.0)
    {
        Err("All time slice fractions must be between 0 and 1")?
    }

    Ok(())
}

/// Check that time slice fractions sum to (approximately) one
fn check_time_slice_fractions_sum_to_one(time_slices: NESlice<TimeSlice>) -> Result<(), String> {
    let sum = time_slices.iter().map(|ts| ts.fraction).sum();
    if !approx_eq!(f64, sum, 1.0, epsilon = 1e-5) {
        Err(format!(
            "Sum of time slice fractions does not equal one (actual: {})",
            sum
        ))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    use super::*;

    macro_rules! ts {
        ($fraction:expr) => {
            TimeSlice {
                season: "summer".to_string(),
                time_of_day: "day".to_string(),
                fraction: $fraction,
            }
        };
    }

    /// Create an example time slices file in dir_path
    fn create_time_slices_file(dir_path: &Path) -> PathBuf {
        let file_path = dir_path.join("time_slices.csv");
        let mut file = File::create(&file_path).unwrap();
        writeln!(
            file,
            "season,time_of_day,fraction
winter,day,0.25
peak,night,0.25
summer,peak,0.25
autumn,evening,0.25"
        )
        .unwrap();
        file_path
    }

    #[test]
    fn test_read_time_slices() {
        let dir = tempdir().unwrap();
        let file_path = create_time_slices_file(dir.path());
        let time_slices = read_time_slices(&file_path).unwrap();
        assert_eq!(
            time_slices,
            nev![
                TimeSlice {
                    season: "winter".to_string(),
                    time_of_day: "day".to_string(),
                    fraction: 0.25
                },
                TimeSlice {
                    season: "peak".to_string(),
                    time_of_day: "night".to_string(),
                    fraction: 0.25
                },
                TimeSlice {
                    season: "summer".to_string(),
                    time_of_day: "peak".to_string(),
                    fraction: 0.25
                },
                TimeSlice {
                    season: "autumn".to_string(),
                    time_of_day: "evening".to_string(),
                    fraction: 0.25
                }
            ]
        )
    }

    #[test]
    fn test_read_time_slices_empty() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("time_slices.csv");
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "season,time_of_day,fraction").unwrap();
        }

        assert!(read_time_slices(&file_path).is_err());
    }

    #[test]
    fn test_check_time_slice_fractions_in_range() {
        macro_rules! check_ok {
            ($e:expr) => {
                assert!(
                    check_time_slice_fractions_in_range(NESlice::from_slice($e).unwrap()).is_ok()
                );
            };
        }
        macro_rules! check_err {
            ($e:expr) => {
                assert!(
                    check_time_slice_fractions_in_range(NESlice::from_slice($e).unwrap()).is_err()
                );
            };
        }

        // Single inputs, valid
        check_ok!(&[ts!(0.0)]);
        check_ok!(&[ts!(0.5)]);
        check_ok!(&[ts!(1.0)]);

        // Single inputs, invalid
        check_err!(&[ts!(-1.0)]);
        check_err!(&[ts!(1.5)]);
        check_err!(&[ts!(2.0)]);

        // Multiple inputs, valid
        check_ok!(&[ts!(0.0), ts!(0.5)]);
        check_ok!(&[ts!(0.5), ts!(1.0)]);
        check_ok!(&[ts!(1.0), ts!(0.25)]);

        // Multiple inputs, invalid
        check_err!(&[ts!(-1.0), ts!(0.5)]);
        check_err!(&[ts!(1.5), ts!(-1.0)]);
        check_err!(&[ts!(2.0), ts!(1.0)]);

        // Edge cases
        check_err!(&[ts!(f64::INFINITY)]);
        check_err!(&[ts!(f64::NAN)]);
    }

    #[test]
    fn test_check_time_slice_fractions_sum_to_one() {
        macro_rules! check_ok {
            ($e:expr) => {
                assert!(
                    check_time_slice_fractions_sum_to_one(NESlice::from_slice($e).unwrap()).is_ok()
                );
            };
        }
        macro_rules! check_err {
            ($e:expr) => {
                assert!(
                    check_time_slice_fractions_sum_to_one(NESlice::from_slice($e).unwrap())
                        .is_err()
                );
            };
        }

        // Single input, valid
        check_ok!(&[ts!(1.0)]);

        // Single input, invalid
        check_err!(&[ts!(0.5)]);

        // Multiple inputs, valid
        check_ok!(&[ts!(0.4), ts!(0.6)]);

        // Multiple inputs, invalid
        check_err!(&[ts!(0.4), ts!(0.3)]);

        // Edge cases
        check_err!(&[ts!(f64::INFINITY)]);
        check_err!(&[ts!(f64::NAN)]);
    }
}
