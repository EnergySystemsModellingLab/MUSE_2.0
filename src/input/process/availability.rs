//! Code for reading process availabilities CSV file
use super::super::*;
use crate::id::IDCollection;
use crate::process::{Process, ProcessEnergyLimitsMap, ProcessID};
use crate::region::parse_region_str;
use crate::time_slice::TimeSliceInfo;
use crate::year::parse_year_str;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;

const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";

/// Represents a row of the process availabilities CSV file
#[derive(Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    regions: String,
    year: String,
    time_slice: String,
    limit_type: LimitType,
    value: f64,
}

impl ProcessAvailabilityRaw {
    fn validate(&self) -> Result<()> {
        // Check availability value
        ensure!(
            self.value >= 0.0 && self.value <= 1.0,
            "Value for availability must be between 0 and 1 inclusive"
        );

        Ok(())
    }

    /// Calculate fraction of annual energy as availability multiplied by time slice length.
    ///
    /// The resulting limits are max/min PAC energy produced/consumed in each timeslice per
    /// cap2act units of capacity
    fn to_bounds(&self, ts_length: f64) -> RangeInclusive<f64> {
        let value = self.value * ts_length;
        match self.limit_type {
            LimitType::LowerBound => value..=f64::INFINITY,
            LimitType::UpperBound => 0.0..=value,
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
/// * `process_ids` - The possible valid process IDs
/// * `time_slice_info` - Information about seasons and times of day
///
/// # Returns
///
/// A [`HashMap`] with process IDs as the keys and [`ProcessEnergyLimitsMap`]s as the values or an
/// error.
pub fn read_process_availabilities(
    model_dir: &Path,
    process_ids: &HashSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<ProcessID, ProcessEnergyLimitsMap>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(
        process_availabilities_csv,
        process_ids,
        processes,
        time_slice_info,
        milestone_years,
    )
    .with_context(|| input_err_msg(&file_path))
}

/// Process raw process availabilities input data into [`ProcessEnergyLimitsMap`]s
fn read_process_availabilities_from_iter<I>(
    iter: I,
    process_ids: &HashSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<HashMap<ProcessID, ProcessEnergyLimitsMap>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let mut map = HashMap::new();
    for record in iter {
        record.validate()?;

        // Get process
        let id = process_ids.get_id_by_str(&record.process_id)?;
        let process = processes
            .get(&id)
            .with_context(|| format!("Process {id} not found"))?;

        // Get regions
        let process_regions = process.regions.clone();
        let record_regions =
            parse_region_str(&record.regions, &process_regions).with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Get years
        let process_year_range = &process.years;
        let process_years: Vec<u32> = milestone_years
            .iter()
            .copied()
            .filter(|year| process_year_range.contains(year))
            .collect();
        let record_years = parse_year_str(&record.year, &process_years).with_context(|| {
            format!("Invalid year for process {id}. Valid years are {process_years:?}")
        })?;

        // Get timeslices
        let ts_selection = time_slice_info.get_selection(&record.time_slice)?;

        // Insert the energy limit into the map
        let entry = map.entry(id).or_insert_with(ProcessEnergyLimitsMap::new);
        for (time_slice, ts_length) in time_slice_info.iter_selection(&ts_selection) {
            let bounds = record.to_bounds(ts_length);

            for region in &record_regions {
                for year in record_years.iter().copied() {
                    try_insert(
                        entry,
                        (region.clone(), year, time_slice.clone()),
                        bounds.clone(),
                    )?;
                }
            }
        }
    }

    validate_energy_limits_maps(&map, processes, time_slice_info, milestone_years)?;

    Ok(map)
}

/// Check that every energy limits covers every time slice, and all regions/years of the process
fn validate_energy_limits_maps(
    map: &HashMap<ProcessID, ProcessEnergyLimitsMap>,
    processes: &HashMap<ProcessID, Process>,
    time_slice_info: &TimeSliceInfo,
    milestone_years: &[u32],
) -> Result<()> {
    for (process_id, map) in map.iter() {
        let process = processes.get(process_id).unwrap();
        let year_range = &process.years;
        let reference_years: HashSet<u32> = milestone_years
            .iter()
            .copied()
            .filter(|year| year_range.contains(year))
            .collect();
        let reference_regions = &process.regions;
        let mut missing_keys = Vec::new();
        for year in &reference_years {
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
            "Process {} is missing availabilities for the following regions, years and timeslice: {:?}",
            process_id,
            missing_keys
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_process_availability_raw(
        limit_type: LimitType,
        value: f64,
    ) -> ProcessAvailabilityRaw {
        ProcessAvailabilityRaw {
            process_id: "process".into(),
            regions: "region".into(),
            year: "2010".into(),
            time_slice: "day".into(),
            limit_type,
            value,
        }
    }

    #[test]
    fn test_validate() {
        // Valid
        let valid = create_process_availability_raw(LimitType::LowerBound, 0.5);
        assert!(valid.validate().is_ok());
        let valid = create_process_availability_raw(LimitType::LowerBound, 0.0);
        assert!(valid.validate().is_ok());
        let valid = create_process_availability_raw(LimitType::LowerBound, 1.0);
        assert!(valid.validate().is_ok());

        // Invalid: negative value
        let invalid = create_process_availability_raw(LimitType::LowerBound, -0.5);
        assert!(invalid.validate().is_err());

        // Invalid: value greater than 1
        let invalid = create_process_availability_raw(LimitType::LowerBound, 1.5);
        assert!(invalid.validate().is_err());

        // Invalid: infinity value
        let invalid = create_process_availability_raw(LimitType::LowerBound, f64::INFINITY);
        assert!(invalid.validate().is_err());

        // Invalid: negative infinity value
        let invalid = create_process_availability_raw(LimitType::LowerBound, f64::NEG_INFINITY);
        assert!(invalid.validate().is_err());

        // Invalid: NaN value
        let invalid = create_process_availability_raw(LimitType::LowerBound, f64::NAN);
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_to_bounds() {
        let ts_length = 0.1;

        // Lower bound
        let raw = create_process_availability_raw(LimitType::LowerBound, 0.5);
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, 0.05..=f64::INFINITY);

        // Upper bound
        let raw = create_process_availability_raw(LimitType::UpperBound, 0.5);
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, 0.0..=0.05);

        // Equality
        let raw = create_process_availability_raw(LimitType::Equality, 0.5);
        let bounds = raw.to_bounds(ts_length);
        assert_eq!(bounds, 0.05..=0.05);
    }
}
