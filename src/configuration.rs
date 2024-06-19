use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use toml;

/// Represents the contents of the entire settings file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Settings {
    pub input_files: InputFiles,
    pub milestone_years: MilestoneYears,
}

/// Represents the "input_files" section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct InputFiles {
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
    pub time_slices_path: PathBuf,
}

/// Represents the "milestone_years" section of the settings file.
#[derive(Debug, Deserialize, PartialEq)]
pub struct MilestoneYears {
    pub years: Vec<u32>,
}

/// Read a settings file from the given path.
fn read_settings_file(path: &Path) -> Settings {
    let config_str = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("Failed to read file {:?}: {:?}", path, err));
    toml::from_str(&config_str)
        .unwrap_or_else(|err| panic!("Could not parse settings file: {:?}", err))
}

/// Read settings from disk and update paths.
///
/// # Arguments
///
/// * `settings_file_path`: The path to the settings TOML file (which includes paths to other
///                         configuration files)
pub fn read_settings(settings_file_path: &Path) -> Settings {
    let mut config = read_settings_file(settings_file_path);

    // For paths to other files listed in the settings file, if they're relative, we treat them as
    // relative to the folder the settings file is in.
    let settings_dir = settings_file_path.parent().unwrap(); // will never fail

    // Update the file paths in the config to be absolute paths
    macro_rules! update_path {
        ($path:expr) => {
            $path = settings_dir.join(&$path);
        };
    }

    update_path!(config.input_files.agents_file_path);
    update_path!(config.input_files.agent_objectives_file_path);
    update_path!(config.input_files.agent_regions_file_path);
    update_path!(config.input_files.assets_file_path);
    update_path!(config.input_files.commodities_file_path);
    update_path!(config.input_files.commodity_constraints_file_path);
    update_path!(config.input_files.commodity_costs_file_path);
    update_path!(config.input_files.demand_file_path);
    update_path!(config.input_files.demand_slicing_file_path);
    update_path!(config.input_files.processes_file_path);
    update_path!(config.input_files.process_availabilities_file_path);
    update_path!(config.input_files.process_flow_share_constraints_file_path);
    update_path!(config.input_files.process_flows_file_path);
    update_path!(config.input_files.process_investment_constraints_file_path);
    update_path!(config.input_files.process_pacs_file_path);
    update_path!(config.input_files.process_parameters_file_path);
    update_path!(config.input_files.process_regions_file_path);
    update_path!(config.input_files.regions_file_path);
    update_path!(config.input_files.time_slices_path);

    config
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
    fn test_read_settings_file() {
        let settings = read_settings_file(&get_settings_file_path());

        assert_eq!(
            settings,
            Settings {
                input_files: InputFiles {
                    agents_file_path: PathBuf::from_str("agents.csv").unwrap(),
                    agent_objectives_file_path: PathBuf::from_str("agent_objectives.csv").unwrap(),
                    agent_regions_file_path: PathBuf::from_str("agent_regions.csv").unwrap(),
                    assets_file_path: PathBuf::from_str("assets.csv").unwrap(),
                    commodities_file_path: PathBuf::from_str("commodities.csv").unwrap(),
                    commodity_constraints_file_path: PathBuf::from_str("commodity_constraints.csv").unwrap(),
                    commodity_costs_file_path: PathBuf::from_str("commodity_costs.csv").unwrap(),
                    demand_file_path: PathBuf::from_str("demand.csv").unwrap(),
                    demand_slicing_file_path: PathBuf::from_str("demand_slicing.csv").unwrap(),
                    processes_file_path: PathBuf::from_str("processes.csv").unwrap(),
                    process_availabilities_file_path: PathBuf::from_str("process_availabilities.csv").unwrap(),
                    process_flow_share_constraints_file_path: PathBuf::from_str("process_flow_share_constraints.csv").unwrap(),
                    process_flows_file_path: PathBuf::from_str("process_flows.csv").unwrap(),
                    process_investment_constraints_file_path: PathBuf::from_str("process_investment_constraints.csv").unwrap(),
                    process_pacs_file_path: PathBuf::from_str("process_pacs.csv").unwrap(),
                    process_parameters_file_path: PathBuf::from_str("process_parameters.csv").unwrap(),
                    process_regions_file_path: PathBuf::from_str("process_regions.csv").unwrap(),
                    regions_file_path: PathBuf::from_str("regions.csv").unwrap(),
                    time_slices_path: PathBuf::from_str("time_slices.csv").unwrap(),
                },
                milestone_years: MilestoneYears {
                    years: vec![2020]
                }
            }
        )
    }

    #[test]
    fn test_read_settings() {
        let settings = read_settings(&get_settings_file_path());

        assert_eq!(settings.milestone_years.years, vec![2020]);

        // Additional assertions can be added to test the absolute paths
        dbg!(settings);
    }
}
