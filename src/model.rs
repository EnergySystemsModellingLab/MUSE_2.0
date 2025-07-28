//! The model represents the static input data provided by the user.
use crate::agent::AgentMap;
use crate::asset::check_capacity_valid_for_asset;
use crate::commodity::{CommodityID, CommodityMap};
use crate::input::{input_err_msg, is_sorted_and_unique, read_toml};
use crate::process::ProcessMap;
use crate::region::{RegionID, RegionMap};
use crate::time_slice::TimeSliceInfo;
use crate::units::{Capacity, MoneyPerFlow};
use anyhow::{ensure, Context, Result};
use log::warn;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MODEL_FILE_NAME: &str = "model.toml";
const DEFAULT_CANDIDATE_ASSET_CAPACITY: Capacity = Capacity(0.0001);
const DEFAULT_VALUE_OF_LOST_LOAD: MoneyPerFlow = MoneyPerFlow(1e9);

/// Model definition
pub struct Model {
    /// Path to model folder
    pub model_path: PathBuf,
    /// Parameters from the model TOML file
    pub parameters: ModelFile,
    /// Agents for the simulation
    pub agents: AgentMap,
    /// Commodities for the simulation
    pub commodities: CommodityMap,
    /// Processes for the simulation
    pub processes: ProcessMap,
    /// Information about seasons and time slices
    pub time_slice_info: TimeSliceInfo,
    /// Regions for the simulation
    pub regions: RegionMap,
    /// Commodity ordering for each region and year
    pub commodity_order: HashMap<(RegionID, u32), Vec<CommodityID>>,
}

/// Represents the contents of the entire model file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ModelFile {
    /// Milestone years
    pub milestone_years: Vec<u32>,
    /// The (small) value of capacity given to candidate assets.
    ///
    /// Don't change unless you know what you're doing.
    #[serde(default = "default_candidate_asset_capacity")]
    pub candidate_asset_capacity: Capacity,
    /// Defines the strategy used for calculating commodity prices
    #[serde(default)]
    pub pricing_strategy: PricingStrategy,
    /// The cost applied to unmet demand.
    ///
    /// Currently this only applies to the LCOX appraisal.
    #[serde(default = "default_value_of_lost_load")]
    pub value_of_lost_load: MoneyPerFlow,
}

/// The strategy used for calculating commodity prices
#[derive(DeserializeLabeledStringEnum, Debug, PartialEq, Default)]
pub enum PricingStrategy {
    /// Take commodity prices directly from the shadow prices
    #[default]
    #[string = "shadow_prices"]
    ShadowPrices,
    /// Adjust shadow prices for scarcity
    #[string = "scarcity_adjusted"]
    ScarcityAdjusted,
}

const fn default_candidate_asset_capacity() -> Capacity {
    DEFAULT_CANDIDATE_ASSET_CAPACITY
}

const fn default_value_of_lost_load() -> MoneyPerFlow {
    DEFAULT_VALUE_OF_LOST_LOAD
}

/// Check that the milestone years parameter is valid
///
/// # Arguments
///
/// * `years` - Integer list of milestone years
///
/// # Returns
///
/// An error if the milestone years are invalid
fn check_milestone_years(years: &[u32]) -> Result<()> {
    ensure!(!years.is_empty(), "`milestone_years` is empty");

    ensure!(
        is_sorted_and_unique(years),
        "`milestone_years` must be composed of unique values in order"
    );

    Ok(())
}

impl ModelFile {
    /// Read a model file from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    ///
    /// # Returns
    ///
    /// The model file contents as a `ModelFile` struct or an error if the file is invalid
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> Result<ModelFile> {
        let file_path = model_dir.as_ref().join(MODEL_FILE_NAME);
        let model_file: ModelFile = read_toml(&file_path)?;

        if model_file.pricing_strategy == PricingStrategy::ScarcityAdjusted {
            warn!(
                "The pricing strategy is set to 'scarcity_adjusted'. Commodity prices may be \
                incorrect if assets have more than one output commodity. See: {}/issues/677",
                env!("CARGO_PKG_REPOSITORY")
            );
        }

        let validate = || -> Result<()> {
            check_milestone_years(&model_file.milestone_years)?;
            check_capacity_valid_for_asset(model_file.candidate_asset_capacity)
                .context("Invalid value for candidate_asset_capacity")?;

            Ok(())
        };
        validate().with_context(|| input_err_msg(file_path))?;

        Ok(model_file)
    }
}

impl Model {
    /// Iterate over the model's milestone years.
    pub fn iter_years(&self) -> impl Iterator<Item = u32> + '_ {
        self.parameters.milestone_years.iter().copied()
    }

    /// Iterate over the model's regions (region IDs).
    pub fn iter_regions(&self) -> impl Iterator<Item = &RegionID> + '_ {
        self.regions.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_check_milestone_years() {
        // Valid
        assert!(check_milestone_years(&[1]).is_ok());
        assert!(check_milestone_years(&[1, 2]).is_ok());

        // Invalid
        assert!(check_milestone_years(&[]).is_err());
        assert!(check_milestone_years(&[1, 1]).is_err());
        assert!(check_milestone_years(&[2, 1]).is_err());
    }

    #[test]
    fn test_model_file_from_path() {
        let dir = tempdir().unwrap();
        {
            let mut file = File::create(dir.path().join(MODEL_FILE_NAME)).unwrap();
            writeln!(file, "milestone_years = [2020, 2100]").unwrap();
        }

        let model_file = ModelFile::from_path(dir.path()).unwrap();
        assert_eq!(model_file.milestone_years, [2020, 2100]);
    }
}
