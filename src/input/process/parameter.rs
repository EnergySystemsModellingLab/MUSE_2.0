//! Code for reading process parameters CSV file
use super::define_process_id_getter;
use crate::input::*;
use crate::process::ProcessParameter;
use ::log::warn;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;
use std::rc::Rc;

const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessParameterRaw {
    pub process_id: String,
    pub start_year: Option<u32>,
    pub end_year: Option<u32>,
    pub capital_cost: f64,
    pub fixed_operating_cost: f64,
    pub variable_operating_cost: f64,
    pub lifetime: u32,
    pub discount_rate: Option<f64>,
    pub cap2act: Option<f64>,
}
define_process_id_getter! {ProcessParameterRaw}

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
            process_id: self.process_id,
            years: start_year..=end_year,
            capital_cost: self.capital_cost,
            fixed_operating_cost: self.fixed_operating_cost,
            variable_operating_cost: self.variable_operating_cost,
            lifetime: self.lifetime,
            discount_rate: self.discount_rate.unwrap_or(0.0),
            cap2act: self.cap2act.unwrap_or(1.0),
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
    /// - `cap2act` is present and less than 0.0.
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

        if let Some(c2a) = self.cap2act {
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
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, ProcessParameter>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, process_ids, year_range)
        .with_context(|| input_err_msg(&file_path))
}

fn read_process_parameters_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, ProcessParameter>>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut params = HashMap::new();
    for param in iter {
        let param = param.into_parameter(year_range)?;
        let id = process_ids.get_id(&param.process_id)?;
        ensure!(
            params.insert(Rc::clone(&id), param).is_none(),
            "More than one parameter provided for process {id}"
        );
    }
    ensure!(
        params.len() == process_ids.len(),
        "Each process must have an associated parameter"
    );
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
        cap2act: Option<f64>,
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
            cap2act,
        }
    }

    fn create_param(
        years: RangeInclusive<u32>,
        discount_rate: f64,
        cap2act: f64,
    ) -> ProcessParameter {
        ProcessParameter {
            process_id: "id".to_string(),
            years,
            capital_cost: 0.0,
            fixed_operating_cost: 0.0,
            variable_operating_cost: 0.0,
            lifetime: 1,
            discount_rate,
            cap2act,
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

        // Missing cap2act
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
                cap2act: Some(1.0),
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
                cap2act: Some(1.0),
            },
        ];

        let expected: HashMap<Rc<str>, _> = [
            (
                "A".into(),
                ProcessParameter {
                    process_id: "A".into(),
                    years: 2010..=2020,
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    cap2act: 1.0,
                },
            ),
            (
                "B".into(),
                ProcessParameter {
                    process_id: "B".into(),
                    years: 2015..=2020,
                    capital_cost: 1.0,
                    fixed_operating_cost: 1.0,
                    variable_operating_cost: 1.0,
                    lifetime: 10,
                    discount_rate: 1.0,
                    cap2act: 1.0,
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
                cap2act: Some(1.0),
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
                cap2act: Some(1.0),
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
                cap2act: Some(1.0),
            },
        ];

        assert!(read_process_parameters_from_iter(
            params_raw.into_iter(),
            &process_ids,
            &year_range
        )
        .is_err());
    }

    #[test]
    fn test_read_process_parameters_from_iter_bad_process_missing_param() {
        let year_range = 2000..=2100;
        let process_ids = ["A".into(), "B".into()].into_iter().collect();

        let params_raw = [ProcessParameterRaw {
            process_id: "A".into(),
            start_year: Some(2010),
            end_year: Some(2020),
            capital_cost: 1.0,
            fixed_operating_cost: 1.0,
            variable_operating_cost: 1.0,
            lifetime: 10,
            discount_rate: Some(1.0),
            cap2act: Some(1.0),
        }];

        assert!(read_process_parameters_from_iter(
            params_raw.into_iter(),
            &process_ids,
            &year_range
        )
        .is_err());
    }
}
