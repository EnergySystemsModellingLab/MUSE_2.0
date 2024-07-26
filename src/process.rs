use crate::input::*;
use serde::{Deserialize, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;
use std::rc::Rc;

const PROCESSES_FILE_NAME: &str = "processes.csv";
const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";
const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";
const PROCESS_PACS_FILE_NAME: &str = "process_pacs.csv";
const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";
const PROCESS_REGIONS_FILE_NAME: &str = "process_regions.csv";

macro_rules! define_process_id_getter {
    ($t:ty) => {
        impl HasID for $t {
            fn get_id(&self) -> &str {
                &self.process_id
            }
        }
    };
}

#[derive(PartialEq, Debug, Deserialize)]
pub struct ProcessAvailability {
    pub process_id: String,
    pub limit_type: LimitType,
    pub time_slice: Option<String>,
    #[serde(deserialize_with = "deserialise_proportion")]
    pub value: f64,
}
define_process_id_getter! {ProcessAvailability}

#[derive(PartialEq, Default, Debug, SerializeLabeledStringEnum, DeserializeLabeledStringEnum)]
pub enum FlowType {
    #[default]
    #[string = "fixed"]
    Fixed,
    #[string = "flexible"]
    Flexible,
}

#[derive(PartialEq, Debug, Deserialize)]
pub struct ProcessFlow {
    pub process_id: String,
    pub commodity_id: String,
    pub flow: f64,
    #[serde(default)]
    pub flow_type: FlowType,
    #[serde(deserialize_with = "deserialise_flow_cost")]
    pub flow_cost: f64,
}
define_process_id_getter! {ProcessFlow}

/// Custom deserialiser for flow cost - treat empty fields as 0.0
fn deserialise_flow_cost<'de, D>(deserialiser: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<f64> = Deserialize::deserialize(deserialiser)?;
    match value {
        None => Ok(0.0),
        Some(value) => Ok(value),
    }
}

/// Primary Activity Commodity
#[derive(PartialEq, Debug, Deserialize)]
struct ProcessPAC {
    process_id: String,
    pac: String,
}
define_process_id_getter! {ProcessPAC}

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
    fn into_parameter(
        self,
        file_path: &Path,
        year_range: &RangeInclusive<u32>,
    ) -> ProcessParameter {
        let start_year = self.start_year.unwrap_or(*year_range.start());
        let end_year = self.end_year.unwrap_or(*year_range.end());

        // Check year range is valid
        if start_year > end_year {
            input_panic(
                file_path,
                &format!(
                    "Error in parameter for process {}: start_year > end_year",
                    self.process_id
                ),
            )
        }

        ProcessParameter {
            process_id: self.process_id,
            years: start_year..=end_year,
            capital_cost: self.capital_cost,
            fixed_operating_cost: self.fixed_operating_cost,
            variable_operating_cost: self.variable_operating_cost,
            lifetime: self.lifetime,
            discount_rate: self.discount_rate.unwrap_or(0.0),
            cap2act: self.cap2act.unwrap_or(1.0),
        }
    }
}

#[derive(PartialEq, Debug, Deserialize)]
pub struct ProcessParameter {
    pub process_id: String,
    pub years: RangeInclusive<u32>,
    pub capital_cost: f64,
    pub fixed_operating_cost: f64,
    pub variable_operating_cost: f64,
    pub lifetime: u32,
    pub discount_rate: f64,
    pub cap2act: f64,
}
define_process_id_getter! {ProcessParameter}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessRegion {
    process_id: String,
    region_id: String,
}
define_process_id_getter! {ProcessRegion}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessDescription {
    id: Rc<str>,
    description: String,
}
define_id_getter! {ProcessDescription}

#[derive(PartialEq, Debug)]
pub struct Process {
    pub id: Rc<str>,
    pub description: String,
    pub availabilities: Vec<ProcessAvailability>,
    pub flows: Vec<ProcessFlow>,
    pub pacs: Vec<String>,
    pub parameter: ProcessParameter,
    pub regions: HashSet<Rc<str>>,
}
define_id_getter! {Process}

fn read_process_parameters_from_iter<I>(
    iter: I,
    file_path: &Path,
    process_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, ProcessParameter>
where
    I: Iterator<Item = ProcessParameterRaw>,
{
    let mut params = HashMap::new();
    for param in iter {
        let param = param.into_parameter(file_path, year_range);
        let id = process_ids.get_id_checked(file_path, &param.process_id);

        if params.insert(Rc::clone(&id), param).is_some() {
            input_panic(
                file_path,
                &format!("More than one parameter provided for process {id}"),
            );
        }
    }

    if params.len() < process_ids.len() {
        input_panic(file_path, "Each process must have an associated parameter");
    }

    params
}

/// Read process parameters from the specified CSV file
fn read_process_parameters(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> HashMap<Rc<str>, ProcessParameter> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path);
    read_process_parameters_from_iter(iter, &file_path, process_ids, year_range)
}

/// Read process regions from the specified CSV file
fn read_process_regions(
    file_path: &Path,
    process_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, HashSet<Rc<str>>> {
    let mut process_regions = HashMap::new();
    for item in read_csv::<ProcessRegion>(file_path) {
        let process_id = process_ids.get_id_checked(file_path, &item.process_id);
        let region_id = region_ids.get_id_checked(file_path, &item.region_id);

        // Add or create entry in process_regions
        process_regions
            .entry(process_id)
            .or_insert_with(|| HashSet::with_capacity(1))
            .insert(region_id);
    }

    process_regions
}

/// Read process information from the specified CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `region_ids` - All possible region IDs
/// * `year_range` - The possible range of milestone years
///
/// # Returns
///
/// This function returns a map of processes, with the IDs as keys.
pub fn read_processes(
    model_dir: &Path,
    region_ids: &HashSet<Rc<str>>,
    year_range: RangeInclusive<u32>,
) -> HashMap<Rc<str>, Process> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let mut descriptions = read_csv_id_file::<ProcessDescription>(&file_path);
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let mut availabilities = read_csv_grouped_by_id(&file_path, &process_ids);
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let mut flows = read_csv_grouped_by_id(&file_path, &process_ids);
    let file_path = model_dir.join(PROCESS_PACS_FILE_NAME);
    let mut pacs = read_csv_grouped_by_id(&file_path, &process_ids);
    let mut parameters = read_process_parameters(model_dir, &process_ids, &year_range);
    let file_path = model_dir.join(PROCESS_REGIONS_FILE_NAME);
    let mut regions = read_process_regions(&file_path, &process_ids, region_ids);

    process_ids
        .into_iter()
        .map(|id| {
            // We know entry is present
            let desc = descriptions.remove(&id).unwrap();

            // We've already checked that every process has an associated parameter
            let parameter = parameters.remove(&id).unwrap();

            let process = Process {
                id: desc.id,
                description: desc.description,
                availabilities: availabilities.remove(&id).unwrap_or_default(),
                flows: flows.remove(&id).unwrap_or_default(),
                pacs: pacs
                    .remove(&id)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p: ProcessPAC| p.pac)
                    .collect(),
                parameter,
                regions: regions.remove(&id).unwrap_or_default(),
            };

            (id, process)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_param_raw(
        start_year: Option<u32>,
        end_year: Option<u32>,
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
            lifetime: 1,
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
        let p = PathBuf::new();
        let year_range = 2000..=2100;

        // No missing values
        let raw = create_param_raw(Some(2010), Some(2020), Some(1.0), Some(0.0));
        assert_eq!(
            raw.into_parameter(&p, &year_range),
            create_param(2010..=2020, 1.0, 0.0)
        );

        // Missing years
        let raw = create_param_raw(None, None, Some(1.0), Some(0.0));
        assert_eq!(
            raw.into_parameter(&p, &year_range),
            create_param(2000..=2100, 1.0, 0.0)
        );

        // Missing discount_rate
        let raw = create_param_raw(Some(2010), Some(2020), None, Some(0.0));
        assert_eq!(
            raw.into_parameter(&p, &year_range),
            create_param(2010..=2020, 0.0, 0.0)
        );

        // Missing cap2act
        let raw = create_param_raw(Some(2010), Some(2020), Some(1.0), None);
        assert_eq!(
            raw.into_parameter(&p, &year_range),
            create_param(2010..=2020, 1.0, 1.0)
        );
    }

    #[test]
    fn test_param_raw_into_param_good_years() {
        let p = PathBuf::new();
        let year_range = 2000..=2100;

        // Normal case
        create_param_raw(Some(2000), Some(2100), Some(1.0), Some(0.0))
            .into_parameter(&p, &year_range);

        // start_year out of range - this is permitted
        create_param_raw(Some(1999), Some(2100), Some(1.0), Some(0.0))
            .into_parameter(&p, &year_range);

        // end_year out of range - this is permitted
        create_param_raw(Some(2000), Some(2101), Some(1.0), Some(0.0))
            .into_parameter(&p, &year_range);
    }

    #[test]
    #[should_panic]
    fn test_param_raw_into_param_bad_years() {
        let p = PathBuf::new();
        let year_range = 2000..=2100;

        // start_year after end_year
        create_param_raw(Some(2001), Some(2000), Some(1.0), Some(0.0))
            .into_parameter(&p, &year_range);
    }

    #[test]
    fn test_read_process_parameters_from_iter_good() {
        let p = PathBuf::new();
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
        let actual = read_process_parameters_from_iter(
            params_raw.into_iter(),
            &p,
            &process_ids,
            &year_range,
        );
        assert_eq!(expected, actual);
    }

    #[test]
    #[should_panic]
    fn test_read_process_parameters_from_iter_bad_multiple_params() {
        let p = PathBuf::new();
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

        read_process_parameters_from_iter(params_raw.into_iter(), &p, &process_ids, &year_range);
    }

    #[test]
    #[should_panic]
    fn test_read_process_parameters_from_iter_bad_process_missing_param() {
        let p = PathBuf::new();
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

        read_process_parameters_from_iter(params_raw.into_iter(), &p, &process_ids, &year_range);
    }
}
