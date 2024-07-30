//! Code for simulation models.
use crate::demand::{read_demand_data, Demand};
use crate::input::{input_panic, read_toml};
use crate::process::{read_processes, Process};
use crate::region::{read_regions, Region};
use crate::time_slice::{read_time_slices, TimeSlice};
use log::warn;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const MODEL_FILE_NAME: &str = "model.toml";

/// Model definition
pub struct Model {
    pub milestone_years: Vec<u32>,
    pub processes: HashMap<Rc<str>, Process>,
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
fn check_milestone_years(file_path: &Path, years: &[u32]) {
    if years.is_empty() {
        input_panic(file_path, "milestone_years is empty");
    }

    if !years[..years.len() - 1]
        .iter()
        .zip(years[1..].iter())
        .all(|(y1, y2)| y1 < y2)
    {
        input_panic(
            file_path,
            "milestone_years must be composed of unique values in order",
        );
    }
}

impl ModelFile {
    /// Read a model file from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> ModelFile {
        let file_path = model_dir.as_ref().join(MODEL_FILE_NAME);
        let model_file: ModelFile = read_toml(&file_path);
        check_milestone_years(&file_path, &model_file.milestone_years.years);

        model_file
    }
}

impl Model {
    /// Read a model from the specified directory.
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> Model {
        let model_file = ModelFile::from_path(&model_dir);

        let time_slices = match read_time_slices(model_dir.as_ref()) {
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
        );

        Model {
            milestone_years: model_file.milestone_years.years,
            processes,
            time_slices,
            demand_data: read_demand_data(model_dir.as_ref()),
            regions: read_regions(model_dir.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::panic::catch_unwind;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_check_milestone_years() {
        let p = PathBuf::new();
        check_milestone_years(&p, &[1]);
        check_milestone_years(&p, &[1, 2]);
    }

    #[test]
    fn test_check_milestone_years_err() {
        let p = PathBuf::new();
        macro_rules! check_panic {
            ($years:expr) => {
                assert!(catch_unwind(|| check_milestone_years(&p, $years)).is_err())
            };
        }

        check_panic!(&[]);
        check_panic!(&[1, 1]);
        check_panic!(&[2, 1]);
    }

    #[test]
    fn test_model_file_from_path() {
        let dir = tempdir().unwrap();
        {
            let mut file = File::create(dir.path().join(MODEL_FILE_NAME)).unwrap();
            writeln!(file, "[milestone_years]\nyears = [2020, 2100]").unwrap();
        }

        let model_file = ModelFile::from_path(dir.path());
        assert_eq!(model_file.milestone_years.years, vec![2020, 2100]);
    }
}
