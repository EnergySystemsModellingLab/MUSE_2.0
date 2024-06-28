use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Represents a single demand entry in the dataset.
#[derive(Debug, Deserialize, PartialEq)] // Derive PartialEq here
pub struct Demand {
    /// The year of the demand entry
    pub year: u32,
    /// The region of the demand entry
    pub region: String,
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
///
/// # Example
///
/// ```
/// use std::path::Path;
/// let file_path = Path::new("path/to/demand.csv");
/// match read_demand_from_csv(&file_path) {
///     Ok(demand_data) => println!("Successfully read demand data: {:?}", demand_data),
///     Err(e) => println!("Failed to read demand data: {}", e),
/// }
/// ```
pub fn read_demand_from_csv(file_path: &Path) -> Result<Vec<Demand>, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    // Create a CSV reader with the appropriate configuration.
    let mut csv_reader = csv::Reader::from_reader(reader);

    // Parse the CSV data into a vector of `Demand` structs.
    let mut demand_data = Vec::new();
    for result in csv_reader.deserialize() {
        let demand: Demand = result?;
        demand_data.push(demand);
    }

    Ok(demand_data)
}

/// Initializes the simulation with demand data from a CSV file.
///
/// # Arguments
///
/// * `file_path` - A reference to the path of the CSV file to read from.
pub fn initialize_simulation(file_path: &Path) {
    let demands = read_demand_from_csv(file_path).unwrap_or_else(|err| {
        panic!("Error reading demand from CSV: {:?}", err);
    });

    // Your simulation initialization code here
    println!(
        "Successfully initialized simulation with demands: {:?}",
        demands
    );
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
            "year,region
2023,North
2024,South
2025,East
2026,West"
        )
        .unwrap();
        file_path
    }

    #[test]
    fn test_read_demand_from_csv() {
        let dir = tempdir().unwrap();
        let file_path = create_demand_file(dir.path());
        let demands = read_demand_from_csv(&file_path).unwrap();

        let expected_demands = [
            Demand {
                year: 2023,
                region: "North".to_string(),
            },
            Demand {
                year: 2024,
                region: "South".to_string(),
            },
            Demand {
                year: 2025,
                region: "East".to_string(),
            },
            Demand {
                year: 2026,
                region: "West".to_string(),
            },
        ];

        assert_eq!(&demands[..], &expected_demands[..]); // Convert Vec to slice
    }
}
