//! Code for reading and working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::input::{deserialise_proportion, read_csv_as_vec, UnwrapInputError};
use float_cmp::approx_eq;
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

const TIME_SLICES_FILE_NAME: &str = "time_slices.csv";

/// Represents a single time slice in the simulation
#[derive(PartialEq, Debug, Deserialize)]
pub struct TimeSlice {
    /// Which season (in the year)
    pub season: String,
    /// Time of day, as a category (e.g. night, day etc.)
    pub time_of_day: String,
    /// The fraction of the year that this combination of season and time of day occupies
    #[serde(deserialize_with = "deserialise_proportion")]
    pub fraction: f64,
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
pub fn read_time_slices(model_dir: &Path) -> Option<Vec<TimeSlice>> {
    let file_path = model_dir.join(TIME_SLICES_FILE_NAME);
    if !file_path.exists() {
        return None;
    }

    let time_slices = read_csv_as_vec(&file_path);
    check_time_slice_fractions_sum_to_one(&time_slices).unwrap_input_err(&file_path);

    Some(time_slices)
}

/// Check that time slice fractions sum to (approximately) one
fn check_time_slice_fractions_sum_to_one(time_slices: &[TimeSlice]) -> Result<(), Box<dyn Error>> {
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
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

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
        let time_slices = read_time_slices(dir.path()).unwrap();
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

    #[test]
    fn test_check_time_slice_fractions_sum_to_one() {
        // Single input, valid
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(1.0)]).is_ok());

        // Multiple inputs, valid
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(0.4), ts!(0.6)]).is_ok());

        // Single input, invalid
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(0.5)]).is_err());

        // Multiple inputs, invalid
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(0.4), ts!(0.3)]).is_err());

        // Edge cases
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(f64::INFINITY)]).is_err());
        assert!(check_time_slice_fractions_sum_to_one(&[ts!(f64::NAN)]).is_err());
    }
}
