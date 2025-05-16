//! The model represents the static input data provided by the user.
use crate::agent::AgentMap;
use crate::commodity::CommodityMap;
use crate::input::{input_err_msg, read_toml};
use crate::process::ProcessMap;
use crate::region::{RegionID, RegionMap};
use crate::time_slice::TimeSliceInfo;
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

const MODEL_FILE_NAME: &str = "model.toml";

/// Model definition
pub struct Model {
    /// Milestone years for the simulation. Sorted.
    pub milestone_years: Vec<u32>,
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
}

/// Represents the contents of the entire model file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct ModelFile {
    /// Milestone years section of model file
    pub milestone_years: MilestoneYears,
}

/// Represents the "milestone_years" section of the model file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct MilestoneYears {
    /// Milestone years
    pub years: Vec<u32>,
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
        years[..years.len() - 1]
            .iter()
            .zip(years[1..].iter())
            .all(|(y1, y2)| y1 < y2),
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
        check_milestone_years(&model_file.milestone_years.years)
            .with_context(|| input_err_msg(file_path))?;

        Ok(model_file)
    }
}

impl Model {
    /// Iterate over the model's milestone years.
    pub fn iter_years(&self) -> impl Iterator<Item = u32> + '_ {
        self.milestone_years.iter().copied()
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
            writeln!(file, "[milestone_years]\nyears = [2020, 2100]").unwrap();
        }

        let model_file = ModelFile::from_path(dir.path()).unwrap();
        assert_eq!(model_file.milestone_years.years, [2020, 2100]);
    }
}
