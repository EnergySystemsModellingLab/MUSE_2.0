use crate::input::read_csv_as_vec;
use serde::Deserialize;
use std::path::Path;

const DEMAND_FILE_NAME: &str = "demand.csv";

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
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns a `Vec<Demand>` with the parsed demand data.
pub fn read_demand_data(model_dir: &Path) -> Vec<Demand> {
    // **TODO**: add validation checks here? e.g. check not negative, apply interpolation and
    // extrapolation rules?
    read_csv_as_vec(&model_dir.join(DEMAND_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    /// Create an example demand file in dir_path
    fn create_demand_file(dir_path: &Path) {
        let file_path = dir_path.join(DEMAND_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "commodity_id,region_id,year,demand
COM1,North,2023,10
COM1,South,2023,11
COM1,East,2023,12
COM1,West,2023,13"
        )
        .unwrap();
    }

    #[test]
    fn test_read_demand_from_csv() {
        let dir = tempdir().unwrap();
        create_demand_file(dir.path());
        let demand_data = read_demand_data(dir.path());
        assert_eq!(
            demand_data,
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
