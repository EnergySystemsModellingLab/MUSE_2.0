use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::Path;

/// Represents the demand data with year and region.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Demand {
    pub year: u32,
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
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);
    let mut demands = Vec::new();

    for result in rdr.deserialize() {
        let demand: Demand = result?;
        demands.push(demand);
    }

    Ok(demands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile;

    /// Create a temporary CSV file for testing.
    fn create_temp_csv(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_read_demand_from_csv() {
        let csv_content = "\
year,region
2020,NA
2020,EU
2021,NA";

        let file = create_temp_csv(csv_content);

        let demands = read_demand_from_csv(file.path()).expect("Failed to read demand from CSV");

        let expected_demands = vec![
            Demand {
                year: 2020,
                region: "NA".to_string(),
            },
            Demand {
                year: 2020,
                region: "EU".to_string(),
            },
            Demand {
                year: 2021,
                region: "NA".to_string(),
            },
        ];

        assert_eq!(demands, expected_demands);
    }

    #[test]
    fn test_read_empty_csv() {
        let csv_content = "year,region\n";

        let file = create_temp_csv(csv_content);
        let demands = read_demand_from_csv(file.path()).expect("Failed to read demand from CSV");

        assert!(demands.is_empty());
    }

    #[test]
    fn test_read_invalid_csv() {
        let csv_content = "year,region\n2020,NA\ninvalid,line";

        let file = create_temp_csv(csv_content);
        let result = read_demand_from_csv(file.path());

        assert!(result.is_err());
    }
}
