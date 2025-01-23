//! Code for reading process availabilities CSV file
use crate::input::*;
use crate::process::{LimitType, ProcessAvailability, ProcessAvailabilityMap};
use crate::time_slice::TimeSliceInfo;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";

/// Represents a row of the process availabilities CSV file
#[derive(PartialEq, Debug, Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    limit_type: LimitType,
    time_slice: String,
    value: f64,
}

/// Read the availability of each process over time slices
pub fn read_process_availabilities(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, ProcessAvailabilityMap>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(process_availabilities_csv, process_ids, time_slice_info)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_availabilities_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, ProcessAvailabilityMap>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let mut map = HashMap::new();

    for record in iter {
        ensure!(
            record.value >= 0.0 && record.value <= 1.0,
            "value for availability must be between 0 and 1 inclusive"
        );

        let process_id = process_ids.get_id(&record.process_id)?;
        let map = map
            .entry(process_id)
            .or_insert_with(ProcessAvailabilityMap::new);

        let ts_selection = time_slice_info.get_selection(&record.time_slice)?;
        for (time_slice, _) in time_slice_info.iter_selection(&ts_selection) {
            let existing = map
                .insert(
                    time_slice.clone(),
                    ProcessAvailability {
                        limit_type: record.limit_type,
                        value: record.value,
                    },
                )
                .is_some();

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
