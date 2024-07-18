use crate::input::{deserialise_proportion, read_vec_from_csv, InputError, LimitType};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;

trait HasProcessID {
    fn get_process_id(&self) -> &str;
}

macro_rules! define_id_getter {
    ($t:ty) => {
        impl HasProcessID for $t {
            fn get_process_id(&self) -> &str {
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
define_id_getter! {ProcessAvailability}

#[derive(PartialEq, Debug, SerializeLabeledStringEnum, DeserializeLabeledStringEnum)]
pub enum FlowType {
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
    #[serde(default = "default_flow_type")]
    pub flow_type: FlowType,
    #[serde(deserialize_with = "deserialise_flow_cost")]
    pub flow_cost: f64,
}
define_id_getter! {ProcessFlow}

fn default_flow_type() -> FlowType {
    FlowType::Fixed
}

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
define_id_getter! {ProcessPAC}

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
define_id_getter! {ProcessParameterRaw}

impl ProcessParameterRaw {
    fn into_parameter(
        self,
        file_path: &Path,
        year_range: &RangeInclusive<u32>,
    ) -> Result<ProcessParameter, InputError> {
        let start_year = match self.start_year {
            None => *year_range.start(),
            Some(year) => {
                if !year_range.contains(&year) {
                    Err(InputError::new(file_path, "start_year is out of range"))?
                }

                year
            }
        };
        let end_year = match self.end_year {
            None => *year_range.end(),
            Some(year) => {
                if !year_range.contains(&year) {
                    Err(InputError::new(file_path, "end_year is out of range"))?
                }

                year
            }
        };

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
define_id_getter! {ProcessParameter}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessRegion {
    process_id: String,
    region_id: String,
}
define_id_getter! {ProcessRegion}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessDescription {
    id: String,
    description: String,
}

#[derive(PartialEq, Debug)]
pub struct Process {
    pub id: String,
    pub description: String,
    pub availabilities: Vec<ProcessAvailability>,
    pub flows: Vec<ProcessFlow>,
    pub pacs: Vec<String>,
    pub parameters: Vec<ProcessParameter>,
    pub regions: Vec<String>,
}

/// Read a CSV file, grouping the entries by process ID, applying a filter to each element
fn read_csv_grouped_by_id_with_filter<'a, T, U, F>(
    file_path: &Path,
    process_ids: &'a HashSet<String>,
    filter: F,
) -> Result<HashMap<&'a str, Vec<T>>, InputError>
where
    U: HasProcessID + DeserializeOwned,
    F: Fn(&Path, U) -> Result<T, InputError>,
{
    let vec: Vec<U> = read_vec_from_csv(file_path)?;
    let mut map = HashMap::new();
    for elem in vec.into_iter() {
        let elem_id = elem.get_process_id();
        let id = match process_ids.get(elem_id) {
            None => Err(InputError::new(
                file_path,
                &format!("Process ID {} not present in processes CSV file", elem_id),
            ))?,
            Some(id) => id.as_str(),
        };

        let elem: T = filter(file_path, elem)?;
        match map.get_mut(&id) {
            None => {
                map.insert(id, vec![elem]);
            }
            Some(vec) => vec.push(elem),
        }
    }

    Ok(map)
}

/// Read a CSV file, grouping the entries by process ID
fn read_csv_grouped_by_id<'a, T>(
    file_path: &Path,
    process_ids: &'a HashSet<String>,
) -> Result<HashMap<&'a str, Vec<T>>, InputError>
where
    T: HasProcessID + DeserializeOwned,
{
    read_csv_grouped_by_id_with_filter(file_path, process_ids, |_, x| Ok(x))
}

/// Read processes CSV file, which contains IDs and descriptions.
///
/// Returns a map of IDs to descriptions.
fn read_processes_file(file_path: &Path) -> Result<HashMap<String, String>, InputError> {
    let mut reader = csv::Reader::from_path(file_path)
        .map_err(|err| InputError::new(file_path, &err.to_string()))?;

    let mut descriptions = HashMap::new();
    for result in reader.deserialize() {
        let desc: ProcessDescription =
            result.map_err(|err| InputError::new(file_path, &err.to_string()))?;
        if descriptions.contains_key(&desc.id) {
            Err(InputError::new(
                file_path,
                &format!("Duplicate process ID: {}", &desc.id),
            ))?;
        }

        descriptions.insert(desc.id, desc.description);
    }

    Ok(descriptions)
}

/// Read process information from the specified CSV files
pub fn read_processes(
    processes_file_path: &Path,
    process_availabilities_file_path: &Path,
    process_flows_file_path: &Path,
    process_pacs_file_path: &Path,
    process_parameters_file_path: &Path,
    process_regions_file_path: &Path,
    year_range: RangeInclusive<u32>,
) -> Result<Vec<Process>, InputError> {
    let mut descriptions = read_processes_file(processes_file_path)?;

    // Clone the IDs into a separate set. We need to copy them as the other maps will contain
    // references to the IDs and we want to consume descriptions.
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let mut availabilities =
        read_csv_grouped_by_id(process_availabilities_file_path, &process_ids)?;
    let mut flows = read_csv_grouped_by_id(process_flows_file_path, &process_ids)?;
    let mut pacs = read_csv_grouped_by_id(process_pacs_file_path, &process_ids)?;
    let mut parameters = read_csv_grouped_by_id_with_filter(
        process_parameters_file_path,
        &process_ids,
        |file_path, param: ProcessParameterRaw| param.into_parameter(file_path, &year_range),
    )?;
    let mut regions = read_csv_grouped_by_id(process_regions_file_path, &process_ids)?;

    let processes = process_ids
        .iter()
        .map(|id| {
            let desc = descriptions.remove_entry(id).unwrap(); // we know entry is present
            Process {
                id: desc.0,
                description: desc.1,
                availabilities: availabilities.remove(id.as_str()).unwrap_or_default(),
                flows: flows.remove(id.as_str()).unwrap_or_default(),
                pacs: pacs
                    .remove(id.as_str())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p: ProcessPAC| p.pac)
                    .collect(),
                parameters: parameters.remove(id.as_str()).unwrap_or_default(),
                regions: regions
                    .remove(id.as_str())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|region: ProcessRegion| region.region_id)
                    .collect(),
            }
        })
        .collect();

    Ok(processes)
}
