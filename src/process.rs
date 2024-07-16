use crate::input::{deserialise_proportion, read_vec_from_csv, InputError, LimitType};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::{HashMap, HashSet};
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
pub struct ProcessParameter {
    pub process_id: String,
    pub start_year: u32,
    pub end_year: u32,
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

/// Read a CSV file, grouping the entries by process ID
fn read_csv_grouped_by_id<'a, T>(
    file_path: &Path,
    process_ids: &'a HashSet<String>,
) -> Result<HashMap<&'a str, Vec<T>>, InputError>
where
    T: HasProcessID + DeserializeOwned,
{
    let vec: Vec<T> = read_vec_from_csv(file_path)?;
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

        match map.get_mut(&id) {
            None => {
                map.insert(id, vec![elem]);
            }
            Some(vec) => vec.push(elem),
        }
    }

    Ok(map)
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
) -> Result<Vec<Process>, InputError> {
    let mut descriptions = read_processes_file(processes_file_path)?;

    // Clone the IDs into a separate set. We need to copy them as the other maps will contain
    // references to the IDs and we want to consume descriptions.
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let mut availabilities =
        read_csv_grouped_by_id(process_availabilities_file_path, &process_ids)?;
    let mut flows = read_csv_grouped_by_id(process_flows_file_path, &process_ids)?;
    let mut pacs = read_csv_grouped_by_id(process_pacs_file_path, &process_ids)?;
    let mut parameters = read_csv_grouped_by_id(process_parameters_file_path, &process_ids)?;
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
                    .map(|pacs: ProcessPAC| pacs.pac)
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
