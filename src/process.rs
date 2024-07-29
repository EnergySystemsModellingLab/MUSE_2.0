use crate::input::{
    deserialise_proportion, input_panic, read_csv_as_vec, LimitType, UnwrapInputError,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_string_enum::{DeserializeLabeledStringEnum, SerializeLabeledStringEnum};
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::path::Path;

const PROCESSES_FILE_NAME: &str = "processes.csv";
const PROCESS_AVAILABILITIES_FILE_NAME: &str = "process_availabilities.csv";
const PROCESS_FLOWS_FILE_NAME: &str = "process_flows.csv";
const PROCESS_PACS_FILE_NAME: &str = "process_pacs.csv";
const PROCESS_PARAMETERS_FILE_NAME: &str = "process_parameters.csv";
const PROCESS_REGIONS_FILE_NAME: &str = "process_regions.csv";

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
define_id_getter! {ProcessFlow}

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
///
/// # Arguments
///
/// * `file_path` - Path to CSV file
/// * `process_ids` - All possible process IDs
/// * `filter` - Function to convert the deserialised CSV row into another data structure
///
/// `filter` must be a function which takes a file path and a deserialised CSV row as arguments,
/// returning either another data structure or an error.
///
/// # Returns
///
/// A HashMap with process ID as a key and a vector of filtered CSV data as a value.
fn read_csv_grouped_by_id_with_filter<'a, T, U, F>(
    file_path: &Path,
    process_ids: &'a HashSet<String>,
    filter: F,
) -> HashMap<&'a str, Vec<T>>
where
    U: HasProcessID + DeserializeOwned,
    F: Fn(U) -> T,
{
    let vec: Vec<U> = read_csv_as_vec(file_path);
    let mut map = HashMap::new();
    for elem in vec.into_iter() {
        let elem_id = elem.get_process_id();
        let id = match process_ids.get(elem_id) {
            None => input_panic(
                file_path,
                &format!("Process ID {} not present in processes CSV file", elem_id),
            ),
            Some(id) => id.as_str(),
        };

        let elem: T = filter(elem);
        match map.get_mut(&id) {
            None => {
                map.insert(id, vec![elem]);
            }
            Some(vec) => vec.push(elem),
        }
    }

    map
}

/// Read a CSV file, grouping the entries by process ID
///
/// # Arguments
///
/// * `file_path` - Path to CSV file
/// * `process_ids` - All possible process IDs
///
/// # Returns
///
/// A HashMap with process ID as a key and a vector of CSV data as a value.
fn read_csv_grouped_by_id<'a, T>(
    file_path: &Path,
    process_ids: &'a HashSet<String>,
) -> HashMap<&'a str, Vec<T>>
where
    T: HasProcessID + DeserializeOwned,
{
    read_csv_grouped_by_id_with_filter(file_path, process_ids, |x| x)
}

/// Read processes CSV file, which contains IDs and descriptions.
///
/// Returns a map of IDs to descriptions.
fn read_processes_file(model_dir: &Path) -> HashMap<String, String> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let mut reader = csv::Reader::from_path(&file_path).unwrap_input_err(&file_path);

    let mut descriptions = HashMap::new();
    for result in reader.deserialize() {
        let desc: ProcessDescription = result.unwrap_input_err(&file_path);
        if descriptions.contains_key(&desc.id) {
            input_panic(&file_path, &format!("Duplicate process ID: {}", &desc.id));
        }

        descriptions.insert(desc.id, desc.description);
    }

    descriptions
}

/// Read process information from the specified CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `year_range` - The possible range of milestone years
///
/// # Returns
///
/// This function returns a `Vec<Process>` with the parsed process data.
pub fn read_processes(model_dir: &Path, year_range: RangeInclusive<u32>) -> Vec<Process> {
    let mut descriptions = read_processes_file(model_dir);

    // Clone the IDs into a separate set. We need to copy them as the other maps will contain
    // references to the IDs and we want to consume descriptions.
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let mut availabilities = read_csv_grouped_by_id(
        &model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME),
        &process_ids,
    );
    let mut flows = read_csv_grouped_by_id(&model_dir.join(PROCESS_FLOWS_FILE_NAME), &process_ids);
    let mut pacs = read_csv_grouped_by_id(&model_dir.join(PROCESS_PACS_FILE_NAME), &process_ids);
    let param_file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let mut parameters = read_csv_grouped_by_id_with_filter(
        &param_file_path,
        &process_ids,
        |param: ProcessParameterRaw| param.into_parameter(&param_file_path, &year_range),
    );
    let mut regions =
        read_csv_grouped_by_id(&model_dir.join(PROCESS_REGIONS_FILE_NAME), &process_ids);

    process_ids
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
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    use std::path::PathBuf;
    use tempfile::tempdir;

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
    fn test_read_processes_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join(PROCESSES_FILE_NAME);
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "id,description\nA,Process A\nB,Process B\n").unwrap();
        }

        let expected = HashMap::from([
            ("A".to_string(), "Process A".to_string()),
            ("B".to_string(), "Process B".to_string()),
        ]);
        assert_eq!(read_processes_file(dir.path()), expected);
    }

    #[test]
    #[should_panic]
    fn test_read_processes_file_duplicate_process() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("processes.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();

            // NB: Reuse process ID "A" on purpose
            writeln!(
                file,
                "id,description\nA,Process A\nB,Process B\nA,Process C"
            )
            .unwrap();
        }

        // Duplicate process IDs are not permitted
        read_processes_file(dir.path());
    }

    fn create_process_ids() -> HashSet<String> {
        HashSet::from(["A".to_string(), "B".to_string()])
    }

    #[derive(PartialEq, Debug, Deserialize)]
    struct ProcessData {
        process_id: String,
        value: i32,
    }
    define_id_getter! {ProcessData}

    #[test]
    fn test_read_csv_grouped_by_id() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "process_id,value\nA,1\nB,2\nA,3").unwrap();
        }

        let expected = HashMap::from([
            (
                "A",
                vec![
                    ProcessData {
                        process_id: "A".to_string(),
                        value: 1,
                    },
                    ProcessData {
                        process_id: "A".to_string(),
                        value: 3,
                    },
                ],
            ),
            (
                "B",
                vec![ProcessData {
                    process_id: "B".to_string(),
                    value: 2,
                }],
            ),
        ]);
        let process_ids = create_process_ids();
        let map: HashMap<&str, Vec<ProcessData>> =
            read_csv_grouped_by_id(&dir.path().join("data.csv"), &process_ids);
        assert_eq!(expected, map);
    }

    #[test]
    #[should_panic]
    fn test_read_csv_grouped_by_id_duplicate() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();

            // NB: Process ID "C" isn't valid
            writeln!(file, "process_id,value\nA,1\nB,2\nC,3").unwrap();
        }

        // Check that it fails if a non-existent process ID is provided
        let process_ids = create_process_ids();
        read_csv_grouped_by_id::<ProcessData>(&dir.path().join("data.csv"), &process_ids);
    }

    #[test]
    fn test_read_csv_grouped_by_id_with_filter() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("data.csv");
        {
            let file_path: &Path = &file_path; // cast
            let mut file = File::create(file_path).unwrap();
            writeln!(file, "process_id,value\nA,1\nB,2\nA,3").unwrap();
        }

        // Test using filter which multiplies the value in ProcessData by 2
        let expected = HashMap::from([("A", vec![2, 6]), ("B", vec![4])]);
        let process_ids = create_process_ids();
        let map: HashMap<&str, Vec<i32>> = read_csv_grouped_by_id_with_filter(
            &dir.path().join("data.csv"),
            &process_ids,
            |data: ProcessData| data.value * 2,
        );
        assert_eq!(expected, map);
    }
}
