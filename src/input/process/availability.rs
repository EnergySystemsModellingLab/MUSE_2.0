//! Code for reading process availabilities CSV file
use crate::input::*;
use crate::process::ProcessCapacityMap;
use crate::time_slice::TimeSliceInfo;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";

/// Represents a row of the process availabilities CSV file
#[derive(Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    limit_type: LimitType,
    time_slice: String,
    value: f64,
}

/// The type of limit given for availability
#[derive(DeserializeLabeledStringEnum)]
enum LimitType {
    #[string = "lo"]
    LowerBound,
    #[string = "up"]
    UpperBound,
    #[string = "fx"]
    Equality,
}

/// Read the process availabilities CSV file.
///
/// This file contains information about the availability of processes over the course of a year as
/// a proportion of their maximum capacity.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - The possible valid process IDs
/// * `time_slice_info` - Information about seasons and times of day
///
/// # Returns
///
/// A [`HashMap`] with process IDs as the keys and [`ProcessCapacityMap`]s as the values or an
/// error.
pub fn read_process_availabilities(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, ProcessCapacityMap>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(process_availabilities_csv, process_ids, time_slice_info)
        .with_context(|| input_err_msg(&file_path))
}

/// Process raw process availabilities input data into [`ProcessCapacityMap`]s
fn read_process_availabilities_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, ProcessCapacityMap>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let mut map = HashMap::new();

    for record in iter {
        let process_id = process_ids.get_id(&record.process_id)?;

        ensure!(
            record.value >= 0.0 && record.value <= 1.0,
            "value for availability must be between 0 and 1 inclusive"
        );

        let ts_selection = time_slice_info.get_selection(&record.time_slice)?;

        let map = map
            .entry(process_id)
            .or_insert_with(ProcessCapacityMap::new);

        for (time_slice, ts_length) in time_slice_info.iter_selection(&ts_selection) {
            // Calculate fraction of annual capacity as availability multiplied by time slice length
            let value = record.value * ts_length;
            let bounds = match record.limit_type {
                LimitType::LowerBound => value..=f64::INFINITY,
                LimitType::UpperBound => f64::NEG_INFINITY..=value,
                LimitType::Equality => value..=value,
            };

            let existing = map.insert(time_slice.clone(), bounds.clone()).is_some();
            ensure!(
                !existing,
                "Process availability entry covered by more than one time slice \
                (process: {}, time slice: {})",
                record.process_id,
                time_slice
            )
        }
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_slice::TimeSliceID;
    use itertools::assert_equal;
    use std::iter;

    fn get_time_slice_info() -> TimeSliceInfo {
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

        TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            fractions: [(slices[0].clone(), 0.5), (slices[1].clone(), 0.5)]
                .into_iter()
                .collect(),
        }
    }

    fn check_succeeds(
        process_id: &str,
        limit_type: LimitType,
        time_slice: &str,
        value: f64,
    ) -> bool {
        let process_ids = iter::once("process1".into()).collect();
        let time_slice_info = get_time_slice_info();

        let avail = ProcessAvailabilityRaw {
            process_id: process_id.into(),
            limit_type,
            time_slice: time_slice.into(),
            value,
        };
        read_process_availabilities_from_iter(iter::once(avail), &process_ids, &time_slice_info)
            .is_ok()
    }

    #[test]
    fn test_read_process_availabilities_from_iter_success() {
        let process_ids = iter::once("process1".into()).collect();
        let time_slice_info = get_time_slice_info();

        let value = 0.5;
        macro_rules! check_with_limit_type {
            ($limit_type: expr, $range: expr) => {
                let avail = ProcessAvailabilityRaw {
                    process_id: "process1".into(),
                    limit_type: $limit_type,
                    time_slice: "winter".into(),
                    value,
                };
                let time_slice = time_slice_info
                    .get_time_slice_id_from_str("winter.day")
                    .unwrap();
                let expected_map = iter::once((time_slice, $range)).collect();
                let expected = iter::once(("process1".into(), expected_map));
                assert_equal(
                    read_process_availabilities_from_iter(
                        iter::once(avail),
                        &process_ids,
                        &time_slice_info,
                    )
                    .unwrap(),
                    expected,
                );
            };
        }

        let result = value * 0.5; // time slice lengths are 0.5
        check_with_limit_type!(LimitType::LowerBound, result..=f64::INFINITY);
        check_with_limit_type!(LimitType::UpperBound, f64::NEG_INFINITY..=result);
        check_with_limit_type!(LimitType::Equality, result..=result);
    }

    #[test]
    fn test_read_process_availabilities_from_iter_values() {
        macro_rules! succeeds_with_value {
            ($value:expr) => {{
                check_succeeds("process1", LimitType::Equality, "winter.day", $value)
            }};
        }

        // Good values
        assert!(succeeds_with_value!(0.0));
        assert!(succeeds_with_value!(0.5));
        assert!(succeeds_with_value!(1.0));

        // Bad values
        assert!(!succeeds_with_value!(-1.0));
        assert!(!succeeds_with_value!(2.0));
        assert!(!succeeds_with_value!(f64::NAN));
        assert!(!succeeds_with_value!(f64::NEG_INFINITY));
        assert!(!succeeds_with_value!(f64::INFINITY));
    }

    #[test]
    fn test_read_process_availabilities_from_iter_bad_overlapping_time_slices() {
        let process_ids = iter::once("process1".into()).collect();
        let time_slice_info = get_time_slice_info();

        let value = 0.5;
        let avail = ["winter", "winter.day"].map(|time_slice| ProcessAvailabilityRaw {
            process_id: "process1".into(),
            limit_type: LimitType::Equality,
            time_slice: time_slice.into(),
            value,
        });
        assert!(read_process_availabilities_from_iter(
            avail.into_iter(),
            &process_ids,
            &time_slice_info
        )
        .is_err());
    }

    #[test]
    fn test_read_process_availabilities_from_iter_bad_process_id() {
        assert!(!check_succeeds(
            "MADEUP",
            LimitType::Equality,
            "winter",
            0.5
        ));
    }

    #[test]
    fn test_read_process_availabilities_from_iter_bad_time_slice() {
        assert!(!check_succeeds(
            "process1",
            LimitType::Equality,
            "MADEUP",
            0.5
        ));
    }
}
