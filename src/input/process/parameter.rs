//! Code for reading process parameters CSV file
use super::super::*;
use crate::process::{ProcessID, ProcessMap, ProcessParameter, ProcessParameterMap};
use crate::region::parse_region_str;
use crate::units::{
    ActivityPerCapacity, Dimensionless, MoneyPerActivity, MoneyPerCapacity, MoneyPerCapacityPerYear,
};
use crate::year::parse_year_str;
use ::log::warn;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessParameterRaw {
    process_id: String,
    regions: String,
    years: String,
    capital_cost: MoneyPerCapacity,
    fixed_operating_cost: MoneyPerCapacityPerYear,
    variable_operating_cost: MoneyPerActivity,
    lifetime: u32,
    discount_rate: Option<Dimensionless>,
    capacity_to_activity: Option<ActivityPerCapacity>,
}

impl ProcessParameterRaw {
    fn into_parameter(self) -> Result<ProcessParameter> {
        self.validate()?;

        Ok(ProcessParameter {
            capital_cost: self.capital_cost,
            fixed_operating_cost: self.fixed_operating_cost,
            variable_operating_cost: self.variable_operating_cost,
            lifetime: self.lifetime,
            discount_rate: self.discount_rate.unwrap_or(Dimensionless(0.0)),
            capacity_to_activity: self
                .capacity_to_activity
                .unwrap_or(ActivityPerCapacity(1.0)),
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
                dr >= Dimensionless(0.0),
                "Error in parameter for process {}: Discount rate must be positive",
                self.process_id
            );

            if dr > Dimensionless(1.0) {
                warn!(
                    "Warning in parameter for process {}: Discount rate is greater than 1",
                    self.process_id
                );
            }
        }

        if let Some(c2a) = self.capacity_to_activity {
            ensure!(
                c2a >= ActivityPerCapacity(0.0),
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
    processes: &ProcessMap,
) -> Result<HashMap<ProcessID, ProcessParameterMap>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, processes).with_context(|| input_err_msg(&file_path))
}

fn read_process_parameters_from_iter<I>(
    iter: I,
    processes: &ProcessMap,
) -> Result<HashMap<ProcessID, ProcessParameterMap>>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut map: HashMap<ProcessID, ProcessParameterMap> = HashMap::new();
    for param_raw in iter {
        // Get process
        let (id, process) = processes
            .get_key_value(param_raw.process_id.as_str())
            .with_context(|| format!("Process {} not found", param_raw.process_id))?;

        // Get years
        let process_years = &process.years;
        let parameter_years =
            parse_year_str(&param_raw.years, process_years).with_context(|| {
                format!("Invalid year for process {id}. Valid years are {process_years:?}")
            })?;

        // Get regions
        let process_regions = &process.regions;
        let parameter_regions = parse_region_str(&param_raw.regions, process_regions)
            .with_context(|| {
                format!("Invalid region for process {id}. Valid regions are {process_regions:?}")
            })?;

        // Insert parameter into the map
        let param = Rc::new(param_raw.into_parameter()?);
        let entry = map.entry(id.clone()).or_default();
        for year in parameter_years {
            for region in parameter_regions.clone() {
                try_insert(entry, (region, year), param.clone())?;
            }
        }
    }

    // Check parameters cover all years and regions of the process
    for (id, parameters) in map.iter() {
        let process = processes.get(id).unwrap();
        let reference_years = &process.years;
        let reference_regions = &process.regions;

        let mut missing_keys = Vec::new();
        for year in reference_years {
            for region in reference_regions {
                let key = (region.clone(), *year);
                if !parameters.contains_key(&key) {
                    missing_keys.push(key);
                }
            }
        }
        ensure!(
            missing_keys.is_empty(),
            "Process {} is missing parameters for the following regions and years: {:?}",
            id,
            missing_keys
        );
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_param_raw(
        lifetime: u32,
        discount_rate: Option<Dimensionless>,
        capacity_to_activity: Option<ActivityPerCapacity>,
    ) -> ProcessParameterRaw {
        ProcessParameterRaw {
            process_id: "id".to_string(),
            capital_cost: MoneyPerCapacity(0.0),
            fixed_operating_cost: MoneyPerCapacityPerYear(0.0),
            variable_operating_cost: MoneyPerActivity(0.0),
            lifetime,
            discount_rate,
            capacity_to_activity,
            years: "all".to_string(),
            regions: "all".to_string(),
        }
    }

    fn create_param(
        discount_rate: Dimensionless,
        capacity_to_activity: ActivityPerCapacity,
    ) -> ProcessParameter {
        ProcessParameter {
            capital_cost: MoneyPerCapacity(0.0),
            fixed_operating_cost: MoneyPerCapacityPerYear(0.0),
            variable_operating_cost: MoneyPerActivity(0.0),
            lifetime: 1,
            discount_rate,
            capacity_to_activity,
        }
    }

    #[test]
    fn test_param_raw_into_param_ok() {
        // No missing values
        let raw = create_param_raw(1, Some(Dimensionless(1.0)), Some(ActivityPerCapacity(0.0)));
        assert_eq!(
            raw.into_parameter().unwrap(),
            create_param(Dimensionless(1.0), ActivityPerCapacity(0.0))
        );

        // Missing discount_rate
        let raw = create_param_raw(1, None, Some(ActivityPerCapacity(0.0)));
        assert_eq!(
            raw.into_parameter().unwrap(),
            create_param(Dimensionless(0.0), ActivityPerCapacity(0.0))
        );

        // Missing capacity_to_activity
        let raw = create_param_raw(1, Some(Dimensionless(1.0)), None);
        assert_eq!(
            raw.into_parameter().unwrap(),
            create_param(Dimensionless(1.0), ActivityPerCapacity(1.0))
        );
    }

    #[test]
    fn test_param_raw_validate_bad_lifetime() {
        // lifetime = 0
        assert!(
            create_param_raw(0, Some(Dimensionless(1.0)), Some(ActivityPerCapacity(0.0)))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_param_raw_validate_bad_discount_rate() {
        // discount rate = -1
        assert!(
            create_param_raw(0, Some(Dimensionless(-1.0)), Some(ActivityPerCapacity(0.0)))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_param_raw_validate_bad_capt2act() {
        // capt2act = -1
        assert!(
            create_param_raw(0, Some(Dimensionless(1.0)), Some(ActivityPerCapacity(-1.0)))
                .validate()
                .is_err()
        );
    }
}
