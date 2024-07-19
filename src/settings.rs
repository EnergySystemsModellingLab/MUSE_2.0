use crate::demand::{read_demand_data, Demand};
use crate::input::InputError;
use crate::log::DEFAULT_LOG_LEVEL;
use crate::process::{read_processes, Process};
use crate::region::{read_regions_data, Region};
use crate::time_slice::{read_time_slices, TimeSlice};
use log::warn;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Model settings
pub struct Settings {
    pub processes: Vec<Process>,
    pub time_slices: Vec<TimeSlice>,
    pub milestone_years: Vec<u32>,
    pub demand_data: Vec<Demand>,
    pub regions: Vec<Region>,
}

/// Represents the contents of the entire settings file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct SettingsReader {
    #[serde(skip)]
    model_dir: PathBuf,
    global: Global,
    milestone_years: MilestoneYears,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Global {
    #[serde(default = "default_log_level")]
    log_level: String,
}

/// Helper function to get default log level
fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
}

/// Represents the "milestone_years" section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct MilestoneYears {
    pub years: Vec<u32>,
}

impl SettingsReader {
    /// The user's preferred log level
    pub fn log_level(&self) -> &str {
        &self.global.log_level
    }

    /// Read the contents of a settings file from the given path.
    fn from_path_raw(path: &Path) -> Result<SettingsReader, Box<dyn Error>> {
        let settings_str = fs::read_to_string(path)?;
        let mut reader: SettingsReader = toml::from_str(&settings_str)?;
        reader.model_dir = path.parent().unwrap().to_path_buf(); // won't fail
        Ok(reader)
    }

    /// Read the contents of a settings file from the given path.
    /// # Arguments
    ///
    /// * `path` - The path to the settings TOML file (which includes paths to other configuration
    ///            files)
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<SettingsReader, InputError> {
        let reader = Self::from_path_raw(path.as_ref())
            .map_err(|err| InputError::new(path.as_ref(), &err.to_string()))?;

        if reader.milestone_years.years.is_empty() {
            Err(InputError::new(path.as_ref(), "milestone_years is empty"))?;
        }

        Ok(reader)
    }

    pub fn into_settings(self) -> Result<Settings, InputError> {
        let time_slices = match read_time_slices(&self.model_dir)? {
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

        let years = &self.milestone_years.years;
        let processes = read_processes(
            &self.model_dir,
            *years.first().unwrap()..=*years.last().unwrap(),
        )?;

        Ok(Settings {
            processes,
            time_slices,
            milestone_years: self.milestone_years.years,
            demand_data: read_demand_data(&self.model_dir)?,
            regions: read_regions_data(&self.model_dir)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Get the path to the example settings file in the examples/simple folder.
    fn get_settings_file_path() -> PathBuf {
        Path::new(file!())
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("examples")
            .join("simple")
            .join("settings.toml")
    }

    fn get_settings_reader() -> SettingsReader {
        SettingsReader::from_path(get_settings_file_path())
            .expect("Failed to read example settings file")
    }

    #[test]
    fn test_settings_reader_from_path_raw() {
        let reader = SettingsReader::from_path_raw(&get_settings_file_path())
            .expect("Failed to read example settings file");

        assert_eq!(
            reader,
            SettingsReader {
                model_dir: get_settings_file_path().parent().unwrap().to_owned(),
                global: Global {
                    log_level: "info".to_string()
                },
                milestone_years: MilestoneYears {
                    years: vec![2020, 2100]
                }
            }
        )
    }

    #[test]
    fn test_settings_reader_from_path() {
        let reader = get_settings_reader();
        assert_eq!(reader.milestone_years.years, vec![2020, 2100]);
    }

    #[test]
    fn test_read_settings() {
        get_settings_reader()
            .into_settings()
            .unwrap_or_else(|err| panic!("Failed to read example settings file: {}", err));
    }
}
