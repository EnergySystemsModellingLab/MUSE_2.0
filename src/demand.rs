use crate::input::read_vec_from_csv;
use serde::Deserialize;
use std::error::Error;
use std::path::Path;

/// Represents a single demand entry in the dataset.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Demand {
    /// The year of the demand entry
    pub commodity_id: String,
    /// The region of the demand entry
    pub region_id: String,
    /// The year of the demand entry
    pub year: u32,
    /// The year of the demand entry
    pub demand: f64,
}

/// Reads demand data from a CSV file.
///
/// # Arguments
///
/// * `file_path` - A reference to the path of the CSV file to read from.
///
/// # Returns
///
/// This function returns a `Result` containing either a `Vec<Demand>` with the
/// parsed demand data or a `Box<dyn Error>` if an error occurred.
///
/// # Errors
///
/// This function will return an error if the file cannot be opened or read, or if
/// the CSV data cannot be parsed.
pub fn read_demand_from_csv(file_path: &Path) -> Result<Vec<Demand>, Box<dyn Error>> {
    let demand_data = read_vec_from_csv(file_path)?;

    if demand_data.is_empty() {
        Err("Demand data file cannot be empty")?;
    }

    // TBD add validation checks here? e.g. check not negative, apply interpolation and extrapolation rules?
    Ok(demand_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    /// Create an example demand file in dir_path
    fn create_demand_file(dir_path: &Path) -> PathBuf {
        let file_path = dir_path.join("demand.csv");
        let mut file = File::create(&file_path).unwrap();
        writeln!(
            file,
            "commodity_id,region_id,year,demand
COM1,North,2023,10
COM1,South,2023,11
COM1,East,2023,12
COM1,West,2023,13"
        )
        .unwrap();
        file_path
    }

    #[test]
    fn test_read_demand_from_csv() {
        let dir = tempdir().unwrap();
        let file_path = create_demand_file(dir.path());
        let demands = read_demand_from_csv(&file_path).unwrap();
        assert_eq!(
            demands,
            vec![
                Demand {
                    year: 2023,
                    region_id: "North".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: 10.0,
                },
                Demand {
                    year: 2023,
                    region_id: "South".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: 11.0,
                },
                Demand {
                    year: 2023,
                    region_id: "East".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: 12.0,
                },
                Demand {
                    year: 2023,
                    region_id: "West".to_string(),
                    commodity_id: "COM1".to_string(),
                    demand: 13.0,
                },
            ]
        )
    }
}
