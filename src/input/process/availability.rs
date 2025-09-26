//! Code for reading process availabilities CSV file
use super::super::{format_items_with_cap, input_err_msg, read_csv, try_insert};
use crate::process::{ProcessActivityLimitsMap, ProcessID, ProcessMap};
use crate::region::parse_region_str;
use crate::time_slice::TimeSliceInfo;
use crate::units::{Dimensionless, Year};
use crate::year::parse_year_str;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::path::Path;

const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";

/// Represents a row of the process availabilities CSV file
#[derive(Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    regions: String,
    years: String,
    time_slice: String,
    limit_type: LimitType,
    value: Dimensionless,
}

impl ProcessAvailabilityRaw {
    fn validate(&self) -> Result<()> {
        // Check availability value
        ensure!(
            self.value >= Dimensionless(0.0) && self.value <= Dimensionless(1.0),
            "Value for availability must be between 0 and 1 inclusive"
        );

        Ok(())
    }

    /// Calculate fraction of annual energy as availability multiplied by time slice length.
    ///
    /// The resulting limits are max/min energy produced/consumed in each timeslice per
    /// `capacity_to_activity` units of capacity.
    fn to_bounds(&self, ts_length: Year) -> RangeInclusive<Dimensionless> {
        // We know ts_length also represents a fraction of a year, so this is ok.
        let value = self.value * ts_length / Year(1.0);
        match self.limit_type {
            LimitType::LowerBound => value..=Dimensionless(f64::INFINITY),
            LimitType::UpperBound => Dimensionless(0.0)..=value,
            LimitType::Equality => value..=value,
        }
    }
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
/// * `processes` - Map of processes
/// * `time_slice_info` - Information about seasons and times of day
///
/// # Returns
///
/// A [`HashMap`] with process IDs as the keys and [`ProcessActivityLimitsMap`]s as the values or an
/// error.
pub fn read_process_availabilities(
    model_dir: &Path,
    processes: &ProcessMap,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<ProcessID, ProcessActivityLimitsMap>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(process_availabilities_csv, processes, time_slice_info)
        .with_context(|| input_err_msg(&file_path))
}

/// Process raw process availabilities input data into [`ProcessActivityLimitsMap`]s
fn read_process_availabilities_from_iter<I>(
    iter: I,
    processes: &ProcessMap,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<ProcessID, ProcessActivityLimitsMap>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let mut map = HashMap::new();
    for record in iter {
        record.validate()?;

        // Get process
        let (id, process) = processes
            .get_key_value(record.process_id.as_str())
            .with_context(|| format!("Process {} not found", record.process_id))?;

        // Get regions
        let process_regions = &process.regions;
        let record_regions =
            parse_region_str(&record.regions, process_regions).with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Get years
        let process_years = &process.milestone_years;
        let record_years = parse_year_str(&record.years, process_years.iter().copied())
            .with_context(|| {
                format!("Invalid year for process {id}. Valid years are {process_years:?}")
            })?;

        // Get timeslices
        let ts_selection = time_slice_info.get_selection(&record.time_slice)?;

        // Insert the activity limit into the map
        let entry = map
            .entry(id.clone())
            .or_insert_with(ProcessActivityLimitsMap::new);
        for (time_slice, ts_length) in ts_selection.iter(time_slice_info) {
            let bounds = record.to_bounds(ts_length);

            for region in &record_regions {
                for year in record_years.iter().copied() {
                    try_insert(
                        entry,
                        &(region.clone(), year, time_slice.clone()),
                        bounds.clone(),
                    )?;
                }
            }
        }
    }

    validate_activity_limits_maps(&map, processes, time_slice_info)?;

    Ok(map)
}

/// Check that the activity limits cover every time slice and all regions/years of the process
fn validate_activity_limits_maps(
    map: &HashMap<ProcessID, ProcessActivityLimitsMap>,
    processes: &ProcessMap,
    time_slice_info: &TimeSliceInfo,
) -> Result<()> {
    for (process_id, process) in processes {
        let map = map
            .get(process_id)
            .with_context(|| format!("Missing availabilities for process {process_id}"))?;

        let reference_years = &process.milestone_years;
        let reference_regions = &process.regions;
        let mut missing_keys = Vec::new();
        for year in reference_years {
            for region in reference_regions {
                for time_slice in time_slice_info.iter_ids() {
                    let key = (region.clone(), *year, time_slice.clone());
                    if !map.contains_key(&key) {
                        missing_keys.push(key);
                    }
                }
            }
        }
        ensure!(
            missing_keys.is_empty(),
            "Process {process_id} is missing availabilities for the following regions, years and timeslice: {}",
            format_items_with_cap(&missing_keys)
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_process_availability_raw(
        limit_type: LimitType,
        value: Dimensionless,
    ) -> ProcessAvailabilityRaw {
        ProcessAvailabilityRaw {
            process_id: "process".into(),
            regions: "region".into(),
            years: "2010".into(),
            time_slice: "day".into(),
            limit_type,
            value,
        }
    }

    #[test]
    fn test_validate() {
        // Valid
        let valid = create_process_availability_raw(LimitType::LowerBound, Dimensionless(0.5));
        assert!(valid.validate().is_ok());
        let valid = create_process_availability_raw(LimitType::LowerBound, Dimensionless(0.0));
        assert!(valid.validate().is_ok());
        let valid = create_process_availability_raw(LimitType::LowerBound, Dimensionless(1.0));
        assert!(valid.validate().is_ok());

        // Invalid: negative value
        let invalid = create_process_availability_raw(LimitType::LowerBound, Dimensionless(-0.5));
        assert!(invalid.validate().is_err());

        // Invalid: value greater than 1
        let invalid = create_process_availability_raw(LimitType::LowerBound, Dimensionless(1.5));
        assert!(invalid.validate().is_err());

        // Invalid: infinity value
        let invalid =
            create_process_availability_raw(LimitType::LowerBound, Dimensionless(f64::INFINITY));
        assert!(invalid.validate().is_err());

        // Invalid: negative infinity value
        let invalid = create_process_availability_raw(
            LimitType::LowerBound,
            Dimensionless(f64::NEG_INFINITY),
        );
        assert!(invalid.validate().is_err());

        // Invalid: NaN value
        let invalid =
            create_process_availability_raw(LimitType::LowerBound, Dimensionless(f64::NAN));
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_to_bounds() {
        let ts_length = Year(0.1);

        // Lower bound
        let raw = create_process_availability_raw(LimitType::LowerBound, Dimensionless(0.5));
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, Dimensionless(0.05)..=Dimensionless(f64::INFINITY));

        // Upper bound
        let raw = create_process_availability_raw(LimitType::UpperBound, Dimensionless(0.5));
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, Dimensionless(0.0)..=Dimensionless(0.05));

        // Equality
        let raw = create_process_availability_raw(LimitType::Equality, Dimensionless(0.5));
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, Dimensionless(0.05)..=Dimensionless(0.05));
    }
}
