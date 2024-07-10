use crate::log::DEFAULT_LOG_LEVEL;
use crate::time_slices::{read_time_slices, TimeSlice};
use log::warn;
use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Model settings
pub struct Settings {
    pub time_slices: Vec<TimeSlice>,
    pub milestone_years: Vec<u32>,
}

/// Represents the contents of the entire settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct SettingsFile {
    pub global: Global,
    pub input_files: InputFiles,
    pub milestone_years: MilestoneYears,
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

/// Represents the "input_files" section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct InputFiles {
    pub agents_file_path: PathBuf,
    pub agent_objectives_file_path: PathBuf,
    pub agent_regions_file_path: PathBuf,
    pub assets_file_path: PathBuf,
    pub commodities_file_path: PathBuf,
    pub commodity_constraints_file_path: PathBuf,
    pub commodity_costs_file_path: PathBuf,
    pub demand_file_path: PathBuf,
    pub demand_slicing_file_path: PathBuf,
    pub processes_file_path: PathBuf,
    pub process_availabilities_file_path: PathBuf,
    pub process_flow_share_constraints_file_path: PathBuf,
    pub process_flows_file_path: PathBuf,
    pub process_investment_constraints_file_path: PathBuf,
    pub process_pacs_file_path: PathBuf,
    pub process_parameters_file_path: PathBuf,
    pub process_regions_file_path: PathBuf,
    pub regions_file_path: PathBuf,
    pub time_slices_path: Option<PathBuf>,
}

/// Represents the "milestone_years" section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
struct MilestoneYears {
    pub years: Vec<u32>,
}

/// Read the contents of a settings file from the given path.
fn read_settings_file_raw(path: &Path) -> Result<SettingsFile, Box<dyn Error>> {
    let settings_str = fs::read_to_string(path)?;
    let settings_file: SettingsFile = toml::from_str(&settings_str)?;
    Ok(settings_file)
}

/// Read settings from a TOML file and update paths.
///
/// # Arguments
///
/// * `settings_file_path`: The path to the settings TOML file (which includes paths to other
///                         configuration files)
fn read_settings_file(settings_file_path: &Path) -> Result<SettingsFile, Box<dyn Error>> {
    let mut settings_file = read_settings_file_raw(settings_file_path)?;

    // For paths to other files listed in the settings file, if they're relative, we treat them as
    // relative to the folder the settings file is in.
    let settings_dir = settings_file_path.parent().unwrap(); // will never fail

    // Update the file paths in settings to be absolute paths
    macro_rules! update_path {
        ($path:expr) => {
            $path = settings_dir.join(&$path);
        };
    }

    update_path!(settings_file.input_files.agents_file_path);
    update_path!(settings_file.input_files.agent_objectives_file_path);
    update_path!(settings_file.input_files.agent_regions_file_path);
    update_path!(settings_file.input_files.assets_file_path);
    update_path!(settings_file.input_files.commodities_file_path);
    update_path!(settings_file.input_files.commodity_constraints_file_path);
    update_path!(settings_file.input_files.commodity_costs_file_path);
    update_path!(settings_file.input_files.demand_file_path);
    update_path!(settings_file.input_files.demand_slicing_file_path);
    update_path!(settings_file.input_files.processes_file_path);
    update_path!(settings_file.input_files.process_availabilities_file_path);
    update_path!(
        settings_file
            .input_files
            .process_flow_share_constraints_file_path
    );
    update_path!(settings_file.input_files.process_flows_file_path);
    update_path!(
        settings_file
            .input_files
            .process_investment_constraints_file_path
    );
    update_path!(settings_file.input_files.process_pacs_file_path);
    update_path!(settings_file.input_files.process_parameters_file_path);
    update_path!(settings_file.input_files.process_regions_file_path);
    update_path!(settings_file.input_files.regions_file_path);
    if let Some(mut time_slices_path) = settings_file.input_files.time_slices_path {
        update_path!(time_slices_path);
        settings_file.input_files.time_slices_path = Some(time_slices_path);
    }

    Ok(settings_file)
}

/// Read settings from disk.
///
/// # Arguments
///
/// * `settings_file_path`: The path to the settings TOML file (which includes paths to other
///                         configuration files)
pub fn read_settings(settings_file_path: &Path) -> Result<Settings, Box<dyn Error>> {
    let settings_file = read_settings_file(settings_file_path)?;

    // Initialise program logger
    crate::log::init(&settings_file.global.log_level);

    let time_slices = match settings_file.input_files.time_slices_path {
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

        Some(ref path) => read_time_slices(path)?,
    };

    Ok(Settings {
        time_slices,
        milestone_years: settings_file.milestone_years.years,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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

    #[test]
    fn test_read_settings_file_raw() {
        let settings_file = read_settings_file_raw(&get_settings_file_path())
            .expect("Failed to read read example settings file");

        assert_eq!(
            settings_file,
            SettingsFile {
                global: Global {
                    log_level: "info".to_string()
                },
                input_files: InputFiles {
                    agents_file_path: PathBuf::from_str("agents.csv").unwrap(),
                    agent_objectives_file_path: PathBuf::from_str("agent_objectives.csv").unwrap(),
                    agent_regions_file_path: PathBuf::from_str("agent_regions.csv").unwrap(),
                    assets_file_path: PathBuf::from_str("assets.csv").unwrap(),
                    commodities_file_path: PathBuf::from_str("commodities.csv").unwrap(),
                    commodity_constraints_file_path: PathBuf::from_str("commodity_constraints.csv")
                        .unwrap(),
                    commodity_costs_file_path: PathBuf::from_str("commodity_costs.csv").unwrap(),
                    demand_file_path: PathBuf::from_str("demand.csv").unwrap(),
                    demand_slicing_file_path: PathBuf::from_str("demand_slicing.csv").unwrap(),
                    processes_file_path: PathBuf::from_str("processes.csv").unwrap(),
                    process_availabilities_file_path: PathBuf::from_str(
                        "process_availabilities.csv"
                    )
                    .unwrap(),
                    process_flow_share_constraints_file_path: PathBuf::from_str(
                        "process_flow_share_constraints.csv"
                    )
                    .unwrap(),
                    process_flows_file_path: PathBuf::from_str("process_flows.csv").unwrap(),
                    process_investment_constraints_file_path: PathBuf::from_str(
                        "process_investment_constraints.csv"
                    )
                    .unwrap(),
                    process_pacs_file_path: PathBuf::from_str("process_pacs.csv").unwrap(),
                    process_parameters_file_path: PathBuf::from_str("process_parameters.csv")
                        .unwrap(),
                    process_regions_file_path: PathBuf::from_str("process_regions.csv").unwrap(),
                    regions_file_path: PathBuf::from_str("regions.csv").unwrap(),
                    time_slices_path: Some(PathBuf::from_str("time_slices.csv").unwrap()),
                },
                milestone_years: MilestoneYears { years: vec![2020] }
            }
        )
    }

    #[test]
    fn test_read_settings_file() {
        let settings_file = read_settings_file(&get_settings_file_path())
            .expect("Failed to read example settings file");

        assert_eq!(settings_file.milestone_years.years, vec![2020]);
    }

    #[test]
    fn test_read_settings() {
        read_settings(&get_settings_file_path())
            .unwrap_or_else(|err| panic!("Failed to read example settings file: {:?}", err));
    }
}
