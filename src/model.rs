//! Code for simulation models.
use crate::asset::{read_assets_by_region, Asset};
use crate::commodity::{read_commodities, Commodity};
use crate::demand::{read_demand_data, Demand};
use crate::input::{input_panic, read_toml};
use crate::process::{read_processes, Process};
use crate::region::{read_regions, Region};
use crate::time_slice::{read_time_slices, TimeSlice, TimeSliceDefinitions, TimeSliceID};
use itertools::Itertools;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

pub const MODEL_FILE_NAME: &str = "model.toml";

/// Model definition
pub struct Model {
    pub milestone_years: Vec<u32>,
    pub time_slice_definitions: TimeSliceDefinitions,
    pub assets_by_region: HashMap<Rc<str>, Vec<Asset>>,
    pub commodities: HashMap<Rc<str>, Rc<Commodity>>,
    pub processes: HashMap<Rc<str>, Rc<Process>>,
    pub time_slices: Vec<TimeSlice>,
    pub demand_data: Vec<Demand>,
    pub regions: HashMap<Rc<str>, Region>,
}

/// Represents the contents of the entire model file.
#[derive(Debug, Deserialize, PartialEq)]
struct ModelFile {
    time_slices: TimeSliceDefinitions,
    milestone_years: MilestoneYears,
}

/// Represents the "milestone_years" section of the model file.
#[derive(Debug, Deserialize, PartialEq)]
struct MilestoneYears {
    pub years: Vec<u32>,
}

fn check_time_slice_part(file_path: &Path, field: &'static str, names: &[Rc<str>]) {
    if names.is_empty() {
        input_panic(file_path, &format!("Must provide {field}"));
    }

    if !names.iter().all_unique() {
        input_panic(file_path, &format!("Duplicate values found in {field}"));
    }

    // We use "." for separating season and time of day
    if !names.iter().all(|name| !name.contains('.')) {
        input_panic(file_path, "Time slice names cannot contain dots");
    }
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
        check_time_slice_part(&file_path, "seasons", &model_file.time_slices.seasons);
        check_time_slice_part(
            &file_path,
            "times_of_day",
            &model_file.time_slices.times_of_day,
        );
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

        let regions = read_regions(model_dir.as_ref());
        let region_ids = HashSet::from_iter(regions.keys().cloned());
        let time_slices = read_time_slices(model_dir.as_ref(), &model_file.time_slices);
        let years = &model_file.milestone_years.years;
        let year_range = *years.first().unwrap()..=*years.last().unwrap();

        let commodities = read_commodities(model_dir.as_ref(), &region_ids, &year_range);
        let processes = read_processes(
            model_dir.as_ref(),
            &commodities,
            &region_ids,
            &model_file.time_slices,
            &year_range,
        );
        let assets_by_region = read_assets_by_region(model_dir.as_ref(), &processes, &region_ids);

        Model {
            milestone_years: model_file.milestone_years.years,
            time_slice_definitions: model_file.time_slices,
            assets_by_region,
            commodities,
            processes,
            time_slices,
            demand_data: read_demand_data(model_dir.as_ref()),
            regions,
        }
    }

    pub fn iter_years(&self) -> impl Iterator<Item = u32> {
        let years = &self.milestone_years;
        *years.first().unwrap()..=*years.last().unwrap()
    }

    /// Iterate over all possible time slices
    pub fn iter_time_slices(&self) -> impl Iterator<Item = TimeSliceID> + '_ {
        self.time_slice_definitions
            .seasons
            .iter()
            .cartesian_product(self.time_slice_definitions.times_of_day.iter())
            .map(|(season, time_of_day)| TimeSliceID {
                season: Rc::clone(season),
                time_of_day: Rc::clone(time_of_day),
            })
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

    macro_rules! assert_panics {
        ($e:expr) => {
            assert!(catch_unwind(|| { $e }).is_err())
        };
    }

    #[test]
    fn test_check_time_slice_part() {
        let p = PathBuf::new();

        // Valid
        check_time_slice_part(&p, "field", &["a".into(), "b".into()]);

        // Valid: we currently don't check case
        check_time_slice_part(&p, "field", &["a".into(), "A".into()]);

        // Invalid: empty
        assert_panics!(check_time_slice_part(&p, "field", &[]));

        // Invalid: duplicate values
        assert_panics!(check_time_slice_part(
            &p,
            "field",
            &["a".into(), "b".into(), "a".into()]
        ));

        // Invalid name
        assert_panics!(check_time_slice_part(
            &p,
            "field",
            &["a".into(), "a.b".into()]
        ));
    }

    #[test]
    fn test_check_milestone_years() {
        let p = PathBuf::new();
        check_milestone_years(&p, &[1]);
        check_milestone_years(&p, &[1, 2]);
    }

    #[test]
    fn test_check_milestone_years_err() {
        let p = PathBuf::new();

        assert_panics!(check_milestone_years(&p, &[]));
        assert_panics!(check_milestone_years(&p, &[1, 1]));
        assert_panics!(check_milestone_years(&p, &[2, 1]));
    }

    #[test]
    fn test_model_file_from_path() {
        let dir = tempdir().unwrap();
        {
            let mut file = File::create(dir.path().join(MODEL_FILE_NAME)).unwrap();
            writeln!(
                file,
                r#"
                [time_slices]
                seasons = ["summer", "winter"]
                times_of_day = ["day", "night"]

                [milestone_years]
                years = [2020, 2100]
                "#
            )
            .unwrap();
        }

        let model_file = ModelFile::from_path(dir.path());
        assert_eq!(
            model_file.time_slices.seasons,
            vec!["summer".into(), "winter".into()]
        );
        assert_eq!(
            model_file.time_slices.times_of_day,
            vec!["day".into(), "night".into()]
        );
        assert_eq!(model_file.milestone_years.years, vec![2020, 2100]);
    }
}
