//! Code for reading process parameters CSV file
use super::super::*;
use super::define_process_id_getter;
use crate::process::{Process, ProcessParameter, ProcessParameterMap};
use crate::year::{deserialize_year, Year};
use ::log::warn;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessParameterRaw {
    process_id: String,
    capital_cost: f64,
    fixed_operating_cost: f64,
    variable_operating_cost: f64,
    lifetime: u32,
    discount_rate: Option<f64>,
    capacity_to_activity: Option<f64>,
    #[serde(deserialize_with = "deserialize_year")]
    year: Year,
}
define_process_id_getter! {ProcessParameterRaw}

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
    process_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Process>,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, ProcessParameterMap>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, process_ids, processes, milestone_years)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_parameters_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    processes: &HashMap<Rc<str>, Process>,
    milestone_years: &[u32],
) -> Result<HashMap<Rc<str>, ProcessParameterMap>>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut params: HashMap<Rc<str>, ProcessParameterMap> = HashMap::new();
    for param_raw in iter {
        let id = process_ids.get_id(&param_raw.process_id)?;
        let year = param_raw.year.clone();
        let param = param_raw.into_parameter()?;

        let entry = params.entry(id.clone()).or_default();
        let process = processes
            .get(&id)
            .ok_or_else(|| anyhow::anyhow!("Process {} not found", id))?;
        let year_range = process.years.clone();

        match year {
            Year::Single(year) => {
                entry.insert(year, param.clone());
            }
            Year::Some(years) => {
                for year in years {
                    entry.insert(year, param.clone());
                }
            }
            Year::All => {
                for year in milestone_years.iter() {
                    if year_range.contains(year) {
                        entry.insert(*year, param.clone());
                    }
                }
            }
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
            year: Year::All,
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

    #[test]
    fn test_read_process_parameters_from_iter_good() {
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        let params_raw = [
            ProcessParameterRaw {
                process_id: "A".into(),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
                year: Year::All,
            },
            ProcessParameterRaw {
                process_id: "B".into(),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
                year: Year::All,
            },
        ];

        let expected: HashMap<Rc<str>, _> = [
            (
                "A".into(),
                AnnualField::Constant(ProcessParameter {
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    capacity_to_activity: 1.0,
                }),
            ),
            (
                "B".into(),
                AnnualField::Constant(ProcessParameter {
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    capacity_to_activity: 1.0,
                }),
            ),
        ]
        .into_iter()
        .collect();
        let actual =
            read_process_parameters_from_iter(params_raw.into_iter(), &process_ids).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_read_process_parameters_from_iter_bad_multiple_params() {
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        let params_raw = [
            ProcessParameterRaw {
                process_id: "A".into(),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
                year: Year::All,
            },
            ProcessParameterRaw {
                process_id: "B".into(),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
                year: Year::All,
            },
            ProcessParameterRaw {
                process_id: "A".into(),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
                year: Year::All,
            },
        ];

        assert!(read_process_parameters_from_iter(params_raw.into_iter(), &process_ids,).is_err());
    }
}
