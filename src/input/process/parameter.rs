//! Code for reading process parameters CSV file
use super::super::*;
use crate::id::IDCollection;
use crate::process::{Process, ProcessID, ProcessParameter, ProcessParameterMap};
use crate::region::parse_region_str;
use crate::year::parse_year_str;
use ::log::warn;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessParameterRaw {
    process_id: String,
    regions: String,
    year: String,
    capital_cost: f64,
    fixed_operating_cost: f64,
    variable_operating_cost: f64,
    lifetime: u32,
    discount_rate: Option<f64>,
    capacity_to_activity: Option<f64>,
}

impl ProcessParameterRaw {
    fn into_parameter(self) -> Result<ProcessParameter> {
        self.validate()?;

        Ok(ProcessParameter {
            capital_cost: self.capital_cost,
            fixed_operating_cost: self.fixed_operating_cost,
            variable_operating_cost: self.variable_operating_cost,
            lifetime: self.lifetime,
            discount_rate: self.discount_rate.unwrap_or(0.0),
            capacity_to_activity: self.capacity_to_activity.unwrap_or(1.0),
        })
    }
}

impl ProcessParameterRaw {
    /// Validates the `ProcessParameterRaw` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `lifetime` is 0.
    /// - `discount_rate` is present and less than 0.0.
    /// - `capacity_to_activity` is present and less than 0.0.
    ///
    /// # Warnings
    ///
    /// Logs a warning if:
    /// - `discount_rate` is present and greater than 1.0.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all validations pass.
    fn validate(&self) -> Result<()> {
        ensure!(
            self.lifetime > 0,
            "Error in parameter for process {}: Lifetime must be greater than 0",
            self.process_id
        );

        if let Some(dr) = self.discount_rate {
            ensure!(
                dr >= 0.0,
                "Error in parameter for process {}: Discount rate must be positive",
                self.process_id
            );

            if dr > 1.0 {
                warn!(
                    "Warning in parameter for process {}: Discount rate is greater than 1",
                    self.process_id
                );
            }
        }

        if let Some(c2a) = self.capacity_to_activity {
            ensure!(
                c2a >= 0.0,
                "Error in parameter for process {}: Cap2act must be positive",
                self.process_id
            );
        }

        Ok(())
    }
}

/// Read process parameters from the specified model directory
pub fn read_process_parameters(
    model_dir: &Path,
    process_ids: &HashSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    milestone_years: &[u32],
) -> Result<HashMap<ProcessID, ProcessParameterMap>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, process_ids, processes, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_parameters_from_iter<I>(
    iter: I,
    process_ids: &HashSet<ProcessID>,
    processes: &HashMap<ProcessID, Process>,
    milestone_years: &[u32],
) -> Result<HashMap<ProcessID, ProcessParameterMap>>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut params: HashMap<ProcessID, ProcessParameterMap> = HashMap::new();
    for param_raw in iter {
        // Get process
        let id = process_ids.get_id_by_str(&param_raw.process_id)?;
        let process = processes
            .get(&id)
            .with_context(|| format!("Process {id} not found"))?;

        // Get years
        let process_year_range = &process.years;
        let process_years: Vec<u32> = milestone_years
            .iter()
            .copied()
            .filter(|year| process_year_range.contains(year))
            .collect();
        let parameter_years =
            parse_year_str(&param_raw.year, &process_years).with_context(|| {
                format!("Invalid year for process {id}. Valid years are {process_years:?}")
            })?;

        // Get regions
        let process_regions = process.regions.clone();
        let parameter_regions = parse_region_str(&param_raw.regions, &process_regions)
            .with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Insert parameter into the map
        let param = param_raw.into_parameter()?;
        let entry = params.entry(id.clone()).or_default();
        for year in parameter_years {
            for region in parameter_regions.clone() {
                try_insert(entry, (region, year), param.clone())?;
            }
        }
    }

    // Check if all processes are present in the parameters
    for id in process_ids {
        if !params.contains_key(id) {
            return Err(anyhow::anyhow!("Process {} is missing parameters", id));
        }
    }

    // Check parameters cover all years and regions of the process
    for (id, parameter) in params.iter() {
        let process = processes.get(id).unwrap();
        let year_range = process.years.clone();
        let reference_years: HashSet<u32> = milestone_years
            .iter()
            .copied()
            .filter(|year| year_range.contains(year))
            .collect();
        let reference_regions = process.regions.clone();

        let mut missing_keys = Vec::new();
        for year in &reference_years {
            for region in &reference_regions {
                if !parameter.contains_key(&(region.clone(), *year)) {
                    missing_keys.push((region, *year));
                }
            }
        }
        if !missing_keys.is_empty() {
            return Err(anyhow::anyhow!(
                "Process {} is missing parameters for the following regions and years: {:?}",
                id,
                missing_keys
            ));
        }
    }
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_param_raw(
        lifetime: u32,
        discount_rate: Option<f64>,
        capacity_to_activity: Option<f64>,
    ) -> ProcessParameterRaw {
        ProcessParameterRaw {
            process_id: "id".to_string(),
            capital_cost: 0.0,
            fixed_operating_cost: 0.0,
            variable_operating_cost: 0.0,
            lifetime,
            discount_rate,
            capacity_to_activity,
            year: "all".to_string(),
            regions: "all".to_string(),
        }
    }

    fn create_param(discount_rate: f64, capacity_to_activity: f64) -> ProcessParameter {
        ProcessParameter {
            capital_cost: 0.0,
            fixed_operating_cost: 0.0,
            variable_operating_cost: 0.0,
            lifetime: 1,
            discount_rate,
            capacity_to_activity,
        }
    }

    #[test]
    fn test_param_raw_into_param_ok() {
        // No missing values
        let raw = create_param_raw(1, Some(1.0), Some(0.0));
        assert_eq!(raw.into_parameter().unwrap(), create_param(1.0, 0.0));

        // Missing discount_rate
        let raw = create_param_raw(1, None, Some(0.0));
        assert_eq!(raw.into_parameter().unwrap(), create_param(0.0, 0.0));

        // Missing capacity_to_activity
        let raw = create_param_raw(1, Some(1.0), None);
        assert_eq!(raw.into_parameter().unwrap(), create_param(1.0, 1.0));
    }

    #[test]
    fn test_param_raw_validate_bad_lifetime() {
        // lifetime = 0
        assert!(create_param_raw(0, Some(1.0), Some(0.0))
            .validate()
            .is_err());
    }

    #[test]
    fn test_param_raw_validate_bad_discount_rate() {
        // discount rate = -1
        assert!(create_param_raw(0, Some(-1.0), Some(0.0))
            .validate()
            .is_err());
    }

    #[test]
    fn test_param_raw_validate_bad_capt2act() {
        // capt2act = -1
        assert!(create_param_raw(0, Some(1.0), Some(-1.0))
            .validate()
            .is_err());
    }
}
