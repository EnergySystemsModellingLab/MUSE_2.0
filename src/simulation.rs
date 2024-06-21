use crate::settings::read_settings;
use std::path::Path;

pub fn initialize_simulation() {
    // Specify the path to the settings.toml file
    let settings_file_path = Path::new("examples/simple/settings.toml");

    // Read and process the settings file
    let settings = read_settings(settings_file_path);

    // Example usage: Accessing the milestone years
    println!("Milestone Years: {:?}", settings.milestone_years.years);

    // Use settings as needed for your simulation or other functionality
    dbg!(settings);
}
