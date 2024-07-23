//! Code for simulation models.
use crate::demand::{read_demand_data, Demand};
use crate::input::{read_toml, InputError, InputResult};
use crate::process::{read_processes, Process};
use crate::region::{read_regions_data, Region};
use crate::time_slice::{read_time_slices, TimeSlice};
use log::warn;
use serde::Deserialize;
use std::path::Path;

const MODEL_FILE_NAME: &str = "model.toml";

/// Model definition
pub struct Model {
    pub milestone_years: Vec<u32>,
    pub processes: Vec<Process>,
    pub time_slices: Vec<TimeSlice>,
    pub demand_data: Vec<Demand>,
    pub regions: Vec<Region>,
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
fn check_milestone_years(file_path: &Path, years: &[u32]) -> InputResult<()> {
    if years.is_empty() {
        Err(InputError::new(file_path, "milestone_years is empty"))?;
    }

    if !years[..years.len() - 1]
        .iter()
        .zip(years[1..].iter())
        .all(|(y1, y2)| y1 < y2)
    {
        Err(InputError::new(
            file_path,
            "milestone_years must be composed of unique values in order",
        ))?
    }

    Ok(())
}

impl ModelFile {
    /// Read a model file from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> InputResult<ModelFile> {
        let file_path = model_dir.as_ref().join(MODEL_FILE_NAME);
        let model_file: ModelFile = read_toml(&file_path)?;
        check_milestone_years(&file_path, &model_file.milestone_years.years)?;

        Ok(model_file)
    }
}

impl Model {
    /// Read a model from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> InputResult<Model> {
        let model_file = ModelFile::from_path(&model_dir)?;

        let time_slices = match read_time_slices(model_dir.as_ref())? {
            None => {
                // If there is no time slice file provided, use a default time slice which covers the
                // whole year and the whole day
                warn!("No time slices CSV file provided; using a single time slice");

                vec![TimeSlice {
                    season: "all-year".to_string(),
                    time_of_day: "all-day".to_string(),
                    fraction: 1.0,
                }]
            }

            Some(time_slices) => time_slices,
        };

        let years = &model_file.milestone_years.years;
        let processes = read_processes(
            model_dir.as_ref(),
            *years.first().unwrap()..=*years.last().unwrap(),
        )?;

        Ok(Model {
            milestone_years: model_file.milestone_years.years,
            processes,
            time_slices,
            demand_data: read_demand_data(model_dir.as_ref())?,
            regions: read_regions_data(model_dir.as_ref())?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_check_milestone_years() {
        let p = PathBuf::new();
        assert!(check_milestone_years(&p, &[]).is_err());
        assert!(check_milestone_years(&p, &[1]).is_ok());
        assert!(check_milestone_years(&p, &[1, 2]).is_ok());
        assert!(check_milestone_years(&p, &[1, 1]).is_err());
        assert!(check_milestone_years(&p, &[2, 1]).is_err());
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
