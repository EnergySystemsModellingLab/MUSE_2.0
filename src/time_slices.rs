//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::input::{read_vec_from_csv, InputError};
use float_cmp::approx_eq;
use serde::Deserialize;
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
pub fn read_time_slices(file_path: &Path) -> Result<Vec<TimeSlice>, InputError> {
    let time_slices = read_vec_from_csv(file_path)?;

    check_time_slice_fractions_in_range(file_path, &time_slices)?;
    check_time_slice_fractions_sum_to_one(file_path, &time_slices)?;

    Ok(time_slices)
}

/// Check that time slice fractions are all in the range 0 to 1
fn check_time_slice_fractions_in_range(
    file_path: &Path,
    time_slices: &[TimeSlice],
) -> Result<(), InputError> {
    if time_slices
        .iter()
        .all(|ts| ts.fraction >= 0.0 && ts.fraction <= 1.0)
    {
        Ok(())
    } else {
        Err(InputError::new(
            file_path,
            "All time slice fractions must be between 0 and 1",
        ))
    }
}

/// Check that time slice fractions sum to (approximately) one
fn check_time_slice_fractions_sum_to_one(
    file_path: &Path,
    time_slices: &[TimeSlice],
) -> Result<(), InputError> {
    let sum = time_slices.iter().map(|ts| ts.fraction).sum();
    if approx_eq!(f64, sum, 1.0, epsilon = 1e-5) {
        Ok(())
    } else {
        Err(InputError::new(
            file_path,
            &format!(
                "Sum of time slice fractions does not equal one (actual: {})",
                sum
            ),
        ))
    }
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
            &[
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
        let p = PathBuf::new();

        // Check that it passes when no time slices are passed in
        assert!(check_time_slice_fractions_in_range(&p, &[]).is_ok());

        // Single inputs, valid
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(0.0)]).is_ok());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(0.5)]).is_ok());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(1.0)]).is_ok());

        // Single inputs, invalid
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(-1.0)]).is_err());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(1.5)]).is_err());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(2.0)]).is_err());

        // Multiple inputs, valid
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(0.0), ts!(0.5)]).is_ok());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(0.5), ts!(1.0)]).is_ok());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(1.0), ts!(0.25)]).is_ok());

        // Multiple inputs, invalid
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(-1.0), ts!(0.5)]).is_err());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(1.5), ts!(-1.0)]).is_err());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(2.0), ts!(1.0)]).is_err());

        // Edge cases
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(f64::INFINITY)]).is_err());
        assert!(check_time_slice_fractions_in_range(&p, &[ts!(f64::NAN)]).is_err());
    }

    #[test]
    fn test_check_time_slice_fractions_sum_to_one() {
        let p = PathBuf::new();

        // Single input, valid
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(1.0)]).is_ok());

        // Single input, invalid
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(0.5)]).is_err());

        // Multiple inputs, valid
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(0.4), ts!(0.6)]).is_ok());

        // Multiple inputs, invalid
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(0.4), ts!(0.3)]).is_err());

        // Edge cases
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(f64::INFINITY)]).is_err());
        assert!(check_time_slice_fractions_sum_to_one(&p, &[ts!(f64::NAN)]).is_err());
    }
}
