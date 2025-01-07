//! Code for reading process availabilities CSV file
use super::define_process_id_getter;
use crate::input::*;
use crate::process::{LimitType, ProcessAvailability};
use crate::time_slice::TimeSliceInfo;
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";

define_process_id_getter! {ProcessAvailability}

/// Represents a row of the process availabilities CSV file
#[derive(PartialEq, Debug, Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    limit_type: LimitType,
    time_slice: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    value: f64,
}

/// Read the availability of each process over time slices
pub fn read_process_availabilities(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, Vec<ProcessAvailability>>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(process_availabilities_csv, process_ids, time_slice_info)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_availabilities_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, Vec<ProcessAvailability>>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let availabilities = iter
        .map(|record| -> Result<_> {
            let time_slice = time_slice_info.get_selection(&record.time_slice)?;

            Ok(ProcessAvailability {
                process_id: record.process_id,
                limit_type: record.limit_type,
                time_slice,
                value: record.value,
            })
        })
        .process_results(|iter| iter.into_id_map(process_ids))??;

    ensure!(
        availabilities.len() >= process_ids.len(),
        "Every process must have at least one availability period"
    );

    Ok(availabilities)
}
