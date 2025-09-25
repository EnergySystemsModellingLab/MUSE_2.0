//! Defines the `ModelParameters` struct, which represents the contents of `model.toml`.
use crate::asset::check_capacity_valid_for_asset;
use crate::input::{
    deserialise_proportion_nonzero, input_err_msg, is_sorted_and_unique, read_toml,
};
use crate::units::{Capacity, Dimensionless, MoneyPerFlow};
use anyhow::{Context, Result, ensure};
use log::warn;
use serde::Deserialize;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::path::Path;

const MODEL_PARAMETERS_FILE_NAME: &str = "model.toml";

macro_rules! define_unit_param_default {
    ($name:ident, $type: ty, $value: expr) => {
        fn $name() -> $type {
            <$type>::new($value)
        }
    };
}

macro_rules! define_param_default {
    ($name:ident, $type: ty, $value: expr) => {
        fn $name() -> $type {
            $value
        }
    };
}

define_unit_param_default!(default_candidate_asset_capacity, Capacity, 0.0001);
define_unit_param_default!(default_capacity_limit_factor, Dimensionless, 0.1);
define_unit_param_default!(default_value_of_lost_load, MoneyPerFlow, 1e9);
define_unit_param_default!(default_price_tolerance, Dimensionless, 1e-6);
define_param_default!(default_max_ironing_out_iterations, u32, 10);

/// Represents the contents of the entire model file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ModelParameters {
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
    /// Affects the maximum capacity that can be given to a newly created asset.
    ///
    /// It is the proportion of maximum capacity that could be required across time slices.
    #[serde(default = "default_capacity_limit_factor")]
    #[serde(deserialize_with = "deserialise_proportion_nonzero")]
    pub capacity_limit_factor: Dimensionless,
    /// The cost applied to unmet demand.
    ///
    /// Currently this only applies to the LCOX appraisal.
    #[serde(default = "default_value_of_lost_load")]
    pub value_of_lost_load: MoneyPerFlow,
    /// The maximum number of iterations to run the "ironing out" step of agent investment for
    #[serde(default = "default_max_ironing_out_iterations")]
    pub max_ironing_out_iterations: u32,
    /// The relative tolerance for price convergence in the ironing out loop
    #[serde(default = "default_price_tolerance")]
    pub price_tolerance: Dimensionless,
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

impl ModelParameters {
    /// Read a model file from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    ///
    /// # Returns
    ///
    /// The model file contents as a [`ModelParameters`] struct or an error if the file is invalid
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> Result<ModelParameters> {
        let file_path = model_dir.as_ref().join(MODEL_PARAMETERS_FILE_NAME);
        let model_params: ModelParameters = read_toml(&file_path)?;

        model_params
            .validate()
            .with_context(|| input_err_msg(file_path))?;

        Ok(model_params)
    }

    /// Validate parameters after reading in file
    fn validate(&self) -> Result<()> {
        // milestone_years
        check_milestone_years(&self.milestone_years)?;

        // pricing_strategy
        if self.pricing_strategy == PricingStrategy::ScarcityAdjusted {
            warn!(
                "The pricing strategy is set to 'scarcity_adjusted'. Commodity prices may be \
                incorrect if assets have more than one output commodity. See: {}/issues/677",
                env!("CARGO_PKG_REPOSITORY")
            );
        }

        // capacity_limit_factor already validated with deserialise_proportion_nonzero

        // candidate_asset_capacity
        check_capacity_valid_for_asset(self.candidate_asset_capacity)
            .context("Invalid value for candidate_asset_capacity")?;

        Ok(())
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
    fn test_model_params_from_path() {
        let dir = tempdir().unwrap();
        {
            let mut file = File::create(dir.path().join(MODEL_PARAMETERS_FILE_NAME)).unwrap();
            writeln!(file, "milestone_years = [2020, 2100]").unwrap();
        }

        let model_params = ModelParameters::from_path(dir.path()).unwrap();
        assert_eq!(model_params.milestone_years, [2020, 2100]);
    }
}
