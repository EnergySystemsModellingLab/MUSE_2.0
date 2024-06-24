use crate::demand::{read_demand_from_csv, Demand};
use std::path::Path;

/// Initializes the simulation by reading the demand data from the CSV file and performing necessary setup.
pub fn initialize_simulation() {
    let file_path = Path::new("demand.csv");

    match read_demand_from_csv(file_path) {
        Ok(demands) => {
            println!("Successfully read demand data:");
            for demand in &demands {
                println!("Year: {}, Region: {}", demand.year, demand.region);
            }

            // Call additional simulation functions with demand data
            run_simulation(demands);
        }
        Err(err) => {
            eprintln!("Error reading demand from CSV: {}", err);
        }
    }
}

/// Runs the main simulation logic with the provided demand data.
///
/// # Arguments
///
/// * `demands` - A vector of `Demand` structs containing the demand data.
fn run_simulation(demands: Vec<Demand>) {
    // Example simulation logic: Count and print the number of demand entries
    let num_demands = demands.len();
    println!("Number of demand entries: {}", num_demands);

    // Additional simulation logic can be added here
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
    fn test_initialize_simulation() {
        let csv_content = "\
year,region
2020,NA
2020,EU
2021,NA";

        let file = create_temp_csv(csv_content);

        // Override the file path to point to our temporary file
        let file_path = file.path();
        
        match read_demand_from_csv(file_path) {
            Ok(demands) => {
                assert_eq!(demands.len(), 3);
                assert_eq!(demands[0].year, 2020);
                assert_eq!(demands[0].region, "NA");
                assert_eq!(demands[1].year, 2020);
                assert_eq!(demands[1].region, "EU");
                assert_eq!(demands[2].year, 2021);
                assert_eq!(demands[2].region, "NA");

                // Call the run_simulation function to ensure it works correctly
                run_simulation(demands);
            }
            Err(err) => {
                panic!("Error reading demand from CSV: {}", err);
            }
        }
    }
}
