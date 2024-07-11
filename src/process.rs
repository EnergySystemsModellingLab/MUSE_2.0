use crate::input::{read_vec_from_csv, InputError};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
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
    pub limit_type: String,
    pub timeslice: Option<String>,
    pub value: f64,
}
define_id_getter! {ProcessAvailability}

#[derive(PartialEq, Debug, Deserialize)]
pub struct ProcessFlow {
    pub process_id: String,
    pub commodity_id: String,
    pub flow: f64,
    pub flow_type: String,
    #[serde(deserialize_with = "deserialise_flow_cost")]
    pub flow_cost: f64,
}
define_id_getter! {ProcessFlow}

/// Custom deserialiser for flow cost - treat empty fields as 0.0
fn deserialise_flow_cost<'de, D>(deserialiser: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(deserialiser)?;
    if s.is_empty() {
        return Ok(0.0);
    }

    let value: f64 = s.parse().map_err(serde::de::Error::custom)?;
    Ok(value)
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

/// Read process information from the specified CSV files
pub fn read_processes(
    processes_file_path: &Path,
    process_availabilities_file_path: &Path,
    process_flows_file_path: &Path,
    process_pacs_file_path: &Path,
    process_parameters_file_path: &Path,
    process_regions_file_path: &Path,
) -> Result<Vec<Process>, InputError> {
    let descriptions: Vec<ProcessDescription> = read_vec_from_csv(processes_file_path)?;
    let process_ids: HashSet<String> =
        HashSet::from_iter(descriptions.iter().map(|desc| desc.id.clone()));
    let mut availabilities =
        read_csv_grouped_by_id(process_availabilities_file_path, &process_ids)?;
    let mut flows = read_csv_grouped_by_id(process_flows_file_path, &process_ids)?;
    let mut pacs = read_csv_grouped_by_id(process_pacs_file_path, &process_ids)?;
    let mut parameters = read_csv_grouped_by_id(process_parameters_file_path, &process_ids)?;
    let mut regions = read_csv_grouped_by_id(process_regions_file_path, &process_ids)?;

    let processes = descriptions
        .into_iter()
        .map(|desc| Process {
            id: desc.id.clone(),
            description: desc.description,
            availabilities: availabilities.remove(desc.id.as_str()).unwrap_or_default(),
            flows: flows.remove(desc.id.as_str()).unwrap_or_default(),
            pacs: pacs
                .remove(desc.id.as_str())
                .unwrap_or_default()
                .into_iter()
                .map(|pacs: ProcessPAC| pacs.pac)
                .collect(),
            parameters: parameters.remove(desc.id.as_str()).unwrap_or_default(),
            regions: regions
                .remove(desc.id.as_str())
                .unwrap_or_default()
                .into_iter()
                .map(|region: ProcessRegion| region.region_id)
                .collect(),
        })
        .collect();

    Ok(processes)
}
