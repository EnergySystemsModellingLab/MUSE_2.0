//! Code for reading process parameters CSV file
use super::super::*;
use crate::id::IDCollection;
use crate::process::{ProcessID, ProcessParameter};
use ::log::warn;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;

const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessParameterRaw {
    process_id: String,
    start_year: Option<u32>,
    end_year: Option<u32>,
    capital_cost: f64,
    fixed_operating_cost: f64,
    variable_operating_cost: f64,
    lifetime: u32,
    discount_rate: Option<f64>,
    capacity_to_activity: Option<f64>,
}

impl ProcessParameterRaw {
    fn into_parameter(self, year_range: &RangeInclusive<u32>) -> Result<ProcessParameter> {
        let start_year = self.start_year.unwrap_or(*year_range.start());
        let end_year = self.end_year.unwrap_or(*year_range.end());

        // Check year range is valid
        ensure!(
            start_year <= end_year,
            "Error in parameter for process {}: start_year > end_year",
            self.process_id
        );

        self.validate()?;

        Ok(ProcessParameter {
            years: start_year..=end_year,
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
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<ProcessID, ProcessParameter>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, process_ids, year_range)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_parameters_from_iter<I>(
    iter: I,
    process_ids: &HashSet<ProcessID>,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<ProcessID, ProcessParameter>>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut params = HashMap::new();
    for param_raw in iter {
        let id = process_ids.get_id_by_str(&param_raw.process_id)?;
        let param = param_raw.into_parameter(year_range)?;
        ensure!(
            params.insert(id.clone(), param).is_none(),
            "More than one parameter provided for process {id}"
        );
    }
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_param_raw(
        start_year: Option<u32>,
        end_year: Option<u32>,
        lifetime: u32,
        discount_rate: Option<f64>,
        capacity_to_activity: Option<f64>,
    ) -> ProcessParameterRaw {
        ProcessParameterRaw {
            process_id: "id".to_string(),
            start_year,
            end_year,
            capital_cost: 0.0,
            fixed_operating_cost: 0.0,
            variable_operating_cost: 0.0,
            lifetime,
            discount_rate,
            capacity_to_activity,
        }
    }

    fn create_param(
        years: RangeInclusive<u32>,
        discount_rate: f64,
        capacity_to_activity: f64,
    ) -> ProcessParameter {
        ProcessParameter {
            years,
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
        let year_range = 2000..=2100;

        // No missing values
        let raw = create_param_raw(Some(2010), Some(2020), 1, Some(1.0), Some(0.0));
        assert_eq!(
            raw.into_parameter(&year_range).unwrap(),
            create_param(2010..=2020, 1.0, 0.0)
        );

        // Missing years
        let raw = create_param_raw(None, None, 1, Some(1.0), Some(0.0));
        assert_eq!(
            raw.into_parameter(&year_range).unwrap(),
            create_param(2000..=2100, 1.0, 0.0)
        );

        // Missing discount_rate
        let raw = create_param_raw(Some(2010), Some(2020), 1, None, Some(0.0));
        assert_eq!(
            raw.into_parameter(&year_range).unwrap(),
            create_param(2010..=2020, 0.0, 0.0)
        );

        // Missing capacity_to_activity
        let raw = create_param_raw(Some(2010), Some(2020), 1, Some(1.0), None);
        assert_eq!(
            raw.into_parameter(&year_range).unwrap(),
            create_param(2010..=2020, 1.0, 1.0)
        );
    }

    #[test]
    fn test_param_raw_into_param_good_years() {
        let year_range = 2000..=2100;

        // Normal case
        assert!(
            create_param_raw(Some(2000), Some(2100), 1, Some(1.0), Some(0.0))
                .into_parameter(&year_range)
                .is_ok()
        );

        // start_year out of range - this is permitted
        assert!(
            create_param_raw(Some(1999), Some(2100), 1, Some(1.0), Some(0.0))
                .into_parameter(&year_range)
                .is_ok()
        );

        // end_year out of range - this is permitted
        assert!(
            create_param_raw(Some(2000), Some(2101), 1, Some(1.0), Some(0.0))
                .into_parameter(&year_range)
                .is_ok()
        );
    }

    #[test]
    #[should_panic]
    fn test_param_raw_into_param_bad_years() {
        let year_range = 2000..=2100;

        // start_year after end_year
        assert!(
            create_param_raw(Some(2001), Some(2000), 1, Some(1.0), Some(0.0))
                .into_parameter(&year_range)
                .is_ok()
        );
    }

    #[test]
    fn test_param_raw_validate_bad_lifetime() {
        // lifetime = 0
        assert!(
            create_param_raw(Some(2000), Some(2100), 0, Some(1.0), Some(0.0))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_param_raw_validate_bad_discount_rate() {
        // discount rate = -1
        assert!(
            create_param_raw(Some(2000), Some(2100), 0, Some(-1.0), Some(0.0))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_param_raw_validate_bad_capt2act() {
        // capt2act = -1
        assert!(
            create_param_raw(Some(2000), Some(2100), 0, Some(1.0), Some(-1.0))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn test_read_process_parameters_from_iter_good() {
        let year_range = 2000..=2100;
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        let params_raw = [
            ProcessParameterRaw {
                process_id: "A".into(),
                start_year: Some(2010),
                end_year: Some(2020),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
            },
            ProcessParameterRaw {
                process_id: "B".into(),
                start_year: Some(2015),
                end_year: Some(2020),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
            },
        ];

        let expected: HashMap<ProcessID, _> = [
            (
                "A".into(),
                ProcessParameter {
                    years: 2010..=2020,
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    capacity_to_activity: 1.0,
                },
            ),
            (
                "B".into(),
                ProcessParameter {
                    years: 2015..=2020,
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    capacity_to_activity: 1.0,
                },
            ),
        ]
        .into_iter()
        .collect();
        let actual =
            read_process_parameters_from_iter(params_raw.into_iter(), &process_ids, &year_range)
                .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_read_process_parameters_from_iter_bad_multiple_params() {
        let year_range = 2000..=2100;
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        let params_raw = [
            ProcessParameterRaw {
                process_id: "A".into(),
                start_year: Some(2010),
                end_year: Some(2020),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
            },
            ProcessParameterRaw {
                process_id: "B".into(),
                start_year: Some(2015),
                end_year: Some(2020),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
            },
            ProcessParameterRaw {
                process_id: "A".into(),
                start_year: Some(2015),
                end_year: Some(2020),
                capital_cost: 1.0,
                fixed_operating_cost: 1.0,
                variable_operating_cost: 1.0,
                lifetime: 10,
                discount_rate: Some(1.0),
                capacity_to_activity: Some(1.0),
            },
        ];

        assert!(read_process_parameters_from_iter(
            params_raw.into_iter(),
            &process_ids,
            &year_range
        )
        .is_err());
    }
}
