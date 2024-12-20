#![allow(missing_docs)]
use crate::commodity::Commodity;
use crate::input::region::{define_region_id_getter, read_regions_for_entity};
use crate::input::*;
use crate::region::RegionSelection;
use crate::time_slice::{TimeSliceInfo, TimeSliceSelection};
use ::log::warn;
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use serde::{Deserialize, Deserializer};
use serde_string_enum::DeserializeLabeledStringEnum;
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

#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum LimitType {
    #[string = "lo"]
    LowerBound,
    #[string = "up"]
    UpperBound,
    #[string = "fx"]
    Equality,
}

/// Represents a row of the process availabilities CSV file
#[derive(PartialEq, Debug, Deserialize)]
struct ProcessAvailabilityRaw {
    process_id: String,
    limit_type: LimitType,
    time_slice: String,
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    value: f64,
}

/// The availabilities for a process over time slices
#[derive(PartialEq, Debug)]
pub struct ProcessAvailability {
    /// Unique identifier for the process (typically uses a structured naming convention).
    process_id: String,
    /// The limit type – lower bound, upper bound or equality.
    pub limit_type: LimitType,
    /// The time slice to which the availability applies.
    pub time_slice: TimeSliceSelection,
    /// The availability value, between 0 and 1 inclusive.
    pub value: f64,
}
define_process_id_getter! {ProcessAvailability}

#[derive(PartialEq, Default, Debug, Clone, DeserializeLabeledStringEnum)]
pub enum FlowType {
    #[default]
    #[string = "fixed"]
    /// The input to output flow ratio is fixed.
    Fixed,
    #[string = "flexible"]
    /// The flow ratio can vary, subject to overall flow of a specified group of commodities whose input/output ratio must be as per user input data.
    Flexible,
}

#[derive(PartialEq, Debug, Deserialize)]
struct ProcessFlowRaw {
    process_id: String,
    commodity_id: String,
    flow: f64,
    #[serde(default)]
    flow_type: FlowType,
    #[serde(deserialize_with = "deserialise_flow_cost")]
    flow_cost: f64,
}

define_process_id_getter! {ProcessFlowRaw}

#[derive(PartialEq, Debug, Deserialize, Clone)]
pub struct ProcessFlow {
    /// A unique identifier for the process (typically uses a structured naming convention).
    pub process_id: String,
    /// Identifies the commodity for the specified flow
    pub commodity: Rc<Commodity>,
    /// Commodity flow quantity relative to other commodity flows. +ve value indicates flow out, -ve value indicates flow in.
    pub flow: f64,
    /// Identifies if a flow is fixed or flexible.
    pub flow_type: FlowType,
    /// Cost per unit flow. For example, cost per unit of natural gas produced. Differs from var_opex because the user can apply it to any specified flow, whereas var_opex applies to pac flow.
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
#[derive(PartialEq, Clone, Eq, Hash, Debug, Deserialize)]
struct ProcessPAC {
    process_id: String,
    commodity_id: String,
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

#[derive(PartialEq, Clone, Debug, Deserialize)]
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
define_region_id_getter! {ProcessRegion}

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
    pub pacs: Vec<Rc<Commodity>>,
    pub parameter: ProcessParameter,
    pub regions: RegionSelection,
}
define_id_getter! {Process}

fn read_process_availabilities_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, Vec<ProcessAvailability>>>
where
    I: Iterator<Item = ProcessAvailabilityRaw>,
{
    let availabilities = iter
        .map(|record| -> Result<_> {
            let time_slice = time_slice_info.get_selection(&record.time_slice)?;

            Ok(ProcessAvailability {
                process_id: record.process_id,
                limit_type: record.limit_type,
                time_slice,
                value: record.value,
            })
        })
        .process_results(|iter| iter.into_id_map(process_ids))??;

    ensure!(
        availabilities.len() >= process_ids.len(),
        "Every process must have at least one availability period"
    );

    Ok(availabilities)
}

/// Read the availability of each process over time slices
fn read_process_availabilities(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
) -> Result<HashMap<Rc<str>, Vec<ProcessAvailability>>> {
    let file_path = model_dir.join(PROCESS_AVAILABILITIES_FILE_NAME);
    let process_availabilities_csv = read_csv(&file_path)?;
    read_process_availabilities_from_iter(process_availabilities_csv, process_ids, time_slice_info)
        .with_context(|| input_err_msg(&file_path))
}

/// Read 'ProcessFlowRaw' records from an iterator and convert them into 'ProcessFlow' records.
fn read_process_flows_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
) -> Result<HashMap<Rc<str>, Vec<ProcessFlow>>>
where
    I: Iterator<Item = ProcessFlowRaw>,
{
    iter.map(|flow_raw| -> Result<ProcessFlow> {
        let commodity = commodities
            .get(flow_raw.commodity_id.as_str())
            .with_context(|| format!("{} is not a valid commodity ID", &flow_raw.commodity_id))?;

        Ok(ProcessFlow {
            process_id: flow_raw.process_id,
            commodity: Rc::clone(commodity),
            flow: flow_raw.flow,
            flow_type: flow_raw.flow_type,
            flow_cost: flow_raw.flow_cost,
        })
    })
    .process_results(|iter| iter.into_id_map(process_ids))?
}

fn read_process_flows(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
) -> Result<HashMap<Rc<str>, Vec<ProcessFlow>>> {
    let file_path = model_dir.join(PROCESS_FLOWS_FILE_NAME);
    let process_flow_csv = read_csv(&file_path)?;
    read_process_flows_from_iter(process_flow_csv, process_ids, commodities)
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

/// Read process parameters from the specified CSV file
fn read_process_parameters(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, ProcessParameter>> {
    let file_path = model_dir.join(PROCESS_PARAMETERS_FILE_NAME);
    let iter = read_csv::<ProcessParameterRaw>(&file_path)?;
    read_process_parameters_from_iter(iter, process_ids, year_range)
        .with_context(|| input_err_msg(&file_path))
}

/// Read process Primary Activity Commodities (PACs) from an iterator.
///
/// # Arguments
///
/// * `iter` - An iterator of `ProcessPAC`s
/// * `process_ids` - All possible process IDs
/// * `commodities` - Commodities for the model
///
/// # Returns
///
/// A `HashMap` with process IDs as keys and `Vec`s of commodities as values or an error.
fn read_process_pacs_from_iter<I>(
    iter: I,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<HashMap<Rc<str>, Vec<Rc<Commodity>>>>
where
    I: Iterator<Item = ProcessPAC>,
{
    // Keep track of previous PACs so we can check for duplicates
    let mut existing_pacs = HashSet::new();

    // Build hashmap of process ID to PAC commodities
    let pacs = iter
        .map(|pac| {
            let process_id = process_ids.get_id(&pac.process_id)?;
            let commodity = commodities
                .get(pac.commodity_id.as_str())
                .with_context(|| format!("{} is not a valid commodity ID", &pac.commodity_id))?;

            // Check that commodity is valid and PAC is not a duplicate
            ensure!(existing_pacs.insert(pac), "Duplicate PACs found");
            Ok((process_id, Rc::clone(commodity)))
        })
        .process_results(|iter| iter.into_group_map())?;

    // Check that PACs for each process are either all inputs or all outputs
    validate_pac_flows(&pacs, flows)?;

    // Return result
    Ok(pacs)
}

/// Validate that the PACs for each process are either all inputs or all outputs.
///
/// # Arguments
///
/// * `pacs` - A map of process IDs to PAC commodities
/// * `flows` - A map of process IDs to process flows
///
/// # Returns
/// An `Ok(())` if the check is successful, or an error.
fn validate_pac_flows(
    pacs: &HashMap<Rc<str>, Vec<Rc<Commodity>>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<()> {
    for (process_id, pacs) in pacs.iter() {
        // Get the flows for the process (unwrap is safe as every process has associated flows)
        let flows = flows.get(process_id).unwrap();

        let mut flow_sign: Option<bool> = None; // False for inputs, true for outputs
        for pac in pacs.iter() {
            // Find the flow associated with the PAC
            let flow = flows
                .iter()
                .find(|item| *item.commodity.id == *pac.id)
                .with_context(|| {
                    format!(
                        "PAC {} for process {} must have an associated flow",
                        pac.id, process_id
                    )
                })?;

            // Check that flow sign is consistent
            let current_flow_sign = flow.flow > 0.0;
            if let Some(flow_sign) = flow_sign {
                ensure!(
                    current_flow_sign == flow_sign,
                    "PACs for process {} are a mix of inputs and outputs",
                    process_id
                );
            }
            flow_sign = Some(current_flow_sign);
        }
    }
    Ok(())
}

/// Read process Primary Activity Commodities (PACs) from the specified model directory.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `process_ids` - All possible process IDs
/// * `commodities` - Commodities for the model
fn read_process_pacs(
    model_dir: &Path,
    process_ids: &HashSet<Rc<str>>,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    flows: &HashMap<Rc<str>, Vec<ProcessFlow>>,
) -> Result<HashMap<Rc<str>, Vec<Rc<Commodity>>>> {
    let file_path = model_dir.join(PROCESS_PACS_FILE_NAME);
    let process_pacs_csv = read_csv(&file_path)?;
    read_process_pacs_from_iter(process_pacs_csv, process_ids, commodities, flows)
        .with_context(|| input_err_msg(&file_path))
}

/// Read process information from the specified CSV files.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
/// * `commodities` - Commodities for the model
/// * `region_ids` - All possible region IDs
/// * `time_slice_info` - Information about seasons and times of day
/// * `year_range` - The possible range of milestone years
///
/// # Returns
///
/// This function returns a map of processes, with the IDs as keys.
pub fn read_processes(
    model_dir: &Path,
    commodities: &HashMap<Rc<str>, Rc<Commodity>>,
    region_ids: &HashSet<Rc<str>>,
    time_slice_info: &TimeSliceInfo,
    year_range: &RangeInclusive<u32>,
) -> Result<HashMap<Rc<str>, Rc<Process>>> {
    let file_path = model_dir.join(PROCESSES_FILE_NAME);
    let mut descriptions = read_csv_id_file::<ProcessDescription>(&file_path)?;
    let process_ids = HashSet::from_iter(descriptions.keys().cloned());

    let mut availabilities = read_process_availabilities(model_dir, &process_ids, time_slice_info)?;
    let mut flows = read_process_flows(model_dir, &process_ids, commodities)?;
    let mut pacs = read_process_pacs(model_dir, &process_ids, commodities, &flows)?;
    let mut parameters = read_process_parameters(model_dir, &process_ids, year_range)?;
    let file_path = model_dir.join(PROCESS_REGIONS_FILE_NAME);
    let mut regions =
        read_regions_for_entity::<ProcessRegion>(&file_path, &process_ids, region_ids)?;

    Ok(process_ids
        .into_iter()
        .map(|id| {
            // We know entry is present
            let desc = descriptions.remove(&id).unwrap();

            // We've already checked that these exist for each process
            let parameter = parameters.remove(&id).unwrap();
            let regions = regions.remove(&id).unwrap();

            let process = Process {
                id: desc.id,
                description: desc.description,
                availabilities: availabilities.remove(&id).unwrap_or_default(),
                flows: flows.remove(&id).unwrap_or_default(),
                pacs: pacs.remove(&id).unwrap_or_default(),
                parameter,
                regions,
            };

            (id, process.into())
        })
        .collect())
}

#[cfg(test)]
mod tests {

    use crate::commodity::{CommodityCostMap, CommodityType};
    use crate::time_slice::TimeSliceLevel;

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
    fn test_read_process_flows_from_iter_good() {
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities: HashMap<Rc<str>, Rc<Commodity>> = ["commodity1", "commodity2"]
            .into_iter()
            .map(|id| {
                let commodity = Commodity {
                    id: id.into(),
                    description: "Some description".into(),
                    kind: CommodityType::InputCommodity,
                    time_slice_level: TimeSliceLevel::Annual,
                    costs: CommodityCostMap::new(),
                    demand_by_region: HashMap::new(),
                };

                (Rc::clone(&commodity.id), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
            },
            ProcessFlowRaw {
                process_id: "id2".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
            },
        ];

        let expected = HashMap::from([
            (
                "id1".into(),
                vec![
                    ProcessFlow {
                        process_id: "id1".into(),
                        commodity: commodities.get("commodity1").unwrap().clone(),
                        flow: 1.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                    },
                    ProcessFlow {
                        process_id: "id1".into(),
                        commodity: commodities.get("commodity2").unwrap().clone(),
                        flow: 1.0,
                        flow_type: FlowType::Fixed,
                        flow_cost: 1.0,
                    },
                ],
            ),
            (
                "id2".into(),
                vec![ProcessFlow {
                    process_id: "id2".into(),
                    commodity: commodities.get("commodity1").unwrap().clone(),
                    flow: 1.0,
                    flow_type: FlowType::Fixed,
                    flow_cost: 1.0,
                }],
            ),
        ]);

        let actual =
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
                .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_read_process_flows_from_iter_bad_commodity_id() {
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities = ["commodity1", "commodity2"]
            .into_iter()
            .map(|id| {
                let commodity = Commodity {
                    id: id.into(),
                    description: "Some description".into(),
                    kind: CommodityType::InputCommodity,
                    time_slice_level: TimeSliceLevel::Annual,
                    costs: CommodityCostMap::new(),
                    demand_by_region: HashMap::new(),
                };

                (Rc::clone(&commodity.id), commodity.into())
            })
            .collect();

        let flows_raw = [
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
            },
            ProcessFlowRaw {
                process_id: "id1".into(),
                commodity_id: "commodity3".into(),
                flow: 1.0,
                flow_type: FlowType::Fixed,
                flow_cost: 1.0,
            },
        ];

        assert!(
            read_process_flows_from_iter(flows_raw.into_iter(), &process_ids, &commodities)
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

    #[test]
    fn test_read_process_pacs_from_iter() {
        // Prepare test data
        let process_ids = ["id1".into(), "id2".into()].into_iter().collect();
        let commodities: HashMap<Rc<str>, Rc<Commodity>> = ["commodity1", "commodity2"]
            .into_iter()
            .map(|id| {
                let commodity = Commodity {
                    id: id.into(),
                    description: "Some description".into(),
                    kind: CommodityType::InputCommodity,
                    time_slice_level: TimeSliceLevel::Annual,
                    costs: CommodityCostMap::new(),
                    demand_by_region: HashMap::new(),
                };
                (Rc::clone(&commodity.id), commodity.into())
            })
            .collect();
        let flows: HashMap<Rc<str>, Vec<ProcessFlow>> = ["id1", "id2"]
            .into_iter()
            .map(|process_id| {
                (
                    process_id.into(),
                    ["commodity1", "commodity2"]
                        .into_iter()
                        .map(|commodity_id| ProcessFlow {
                            process_id: process_id.into(),
                            commodity: commodities.get(commodity_id).unwrap().clone(),
                            flow: 1.0,
                            flow_type: FlowType::Fixed,
                            flow_cost: 1.0,
                        })
                        .collect(),
                )
            })
            .collect();

        // duplicate PAC
        let pac = ProcessPAC {
            process_id: "id1".into(),
            commodity_id: "commodity1".into(),
        };
        let pacs = [pac.clone(), pac];
        assert!(
            read_process_pacs_from_iter(pacs.into_iter(), &process_ids, &commodities, &flows)
                .is_err()
        );

        // invalid commodity ID
        let bad_pac = ProcessPAC {
            process_id: "id1".into(),
            commodity_id: "other_commodity".into(),
        };
        assert!(read_process_pacs_from_iter(
            [bad_pac].into_iter(),
            &process_ids,
            &commodities,
            &flows
        )
        .is_err());

        // Valid
        let pacs = [
            ProcessPAC {
                process_id: "id1".into(),
                commodity_id: "commodity1".into(),
            },
            ProcessPAC {
                process_id: "id1".into(),
                commodity_id: "commodity2".into(),
            },
            ProcessPAC {
                process_id: "id2".into(),
                commodity_id: "commodity1".into(),
            },
        ];
        let expected = [
            (
                "id1".into(),
                [
                    commodities.get("commodity1").unwrap(),
                    commodities.get("commodity2").unwrap(),
                ]
                .into_iter()
                .cloned()
                .collect(),
            ),
            (
                "id2".into(),
                [commodities.get("commodity1").unwrap()]
                    .into_iter()
                    .cloned()
                    .collect(),
            ),
        ]
        .into_iter()
        .collect();
        assert!(
            read_process_pacs_from_iter(
                pacs.clone().into_iter(),
                &process_ids,
                &commodities,
                &flows
            )
            .unwrap()
                == expected
        );

        // Invalid flows
        // Making commodity1 an input so the PACs for process id1 are a mix of inputs and outputs
        let mut flows = flows.clone();
        flows
            .get_mut(&Rc::from("id1"))
            .unwrap()
            .iter_mut()
            .find(|flow| flow.commodity.id == "commodity1".into())
            .unwrap()
            .flow = -1.0;
        assert!(
            read_process_pacs_from_iter(pacs.into_iter(), &process_ids, &commodities, &flows)
                .is_err()
        );
    }
}
