//! Code for simulation models.
#![allow(missing_docs)]
use crate::agent::{read_agents, Agent};
use crate::commodity::{read_commodities, Commodity};
use crate::input::*;
use crate::process::Process;
use crate::process_readers::read_processes;
use crate::region::{read_regions, Region};
use crate::time_slice::{read_time_slice_info, TimeSliceInfo};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const MODEL_FILE_NAME: &str = "model.toml";

/// Model definition
pub struct Model {
    pub milestone_years: Vec<u32>,
    pub agents: HashMap<Rc<str>, Agent>,
    pub commodities: HashMap<Rc<str>, Rc<Commodity>>,
    pub processes: HashMap<Rc<str>, Rc<Process>>,
    pub time_slice_info: TimeSliceInfo,
    pub regions: HashMap<Rc<str>, Region>,
}

/// Represents the contents of the entire model file.
#[derive(Debug, Deserialize, PartialEq)]
struct ModelFile {
    milestone_years: MilestoneYears,
}

/// Represents the "milestone_years" section of the model file.
#[derive(Debug, Deserialize, PartialEq)]
struct MilestoneYears {
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
    /// Read a model from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    ///
    /// # Returns
    ///
    /// The model contents as a `Model` struct or an error if the model is invalid
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> Result<Model> {
        let model_file = ModelFile::from_path(&model_dir)?;

        let time_slice_info = read_time_slice_info(model_dir.as_ref())?;
        let regions = read_regions(model_dir.as_ref())?;
        let region_ids = regions.keys().cloned().collect();
        let years = &model_file.milestone_years.years;
        let year_range = *years.first().unwrap()..=*years.last().unwrap();

        let commodities =
            read_commodities(model_dir.as_ref(), &region_ids, &time_slice_info, years)?;
        let processes = read_processes(
            model_dir.as_ref(),
            &commodities,
            &region_ids,
            &time_slice_info,
            &year_range,
        )?;
        let agents = read_agents(model_dir.as_ref(), &processes, &region_ids)?;

        Ok(Model {
            milestone_years: model_file.milestone_years.years,
            agents,
            commodities,
            processes,
            time_slice_info,
            regions,
        })
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
        assert_eq!(model_file.milestone_years.years, vec![2020, 2100]);
    }
}
