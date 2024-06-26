use crate::regions::{read_regions_from_csv, Region};
use std::path::Path;

/// Initializes the simulation by reading the regions from the CSV file and performing necessary setup.
pub fn initialize_simulation() {
    let file_path = Path::new("regions.csv");

    match read_regions_from_csv(file_path) {
        Ok(regions) => {
            println!("Successfully read regions data:");
            for region in &regions {
                println!("Short Name: {}, Description: {}", region.short_name, region.description);
            }

            // Call additional simulation functions with regions data
            run_simulation(regions);
        }
        Err(err) => {
            eprintln!("Error reading regions from CSV: {}", err);
        }
    }
}


fn run_simulation(regions: Vec<Region>) {
    // Example simulation logic: Count and print the number of regions
    let num_regions = regions.len();
    println!("Number of regions: {}", num_regions);

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
short_name,description
NA,North America
EU,Europe
AP,Asia Pacific";

        let file = create_temp_csv(csv_content);

        // Override the file path to point to our temporary file
        let file_path = file.path();
        
        match read_regions_from_csv(file_path) {
            Ok(regions) => {
                assert_eq!(regions.len(), 3);
                assert_eq!(regions[0].short_name, "NA");
                assert_eq!(regions[0].description, "North America");
                assert_eq!(regions[1].short_name, "EU");
                assert_eq!(regions[1].description, "Europe");
                assert_eq!(regions[2].short_name, "AP");
                assert_eq!(regions[2].description, "Asia Pacific");

                // Call the run_simulation function to ensure it works correctly
                run_simulation(regions);
            }
            Err(err) => {
                panic!("Error reading regions from CSV: {}", err);
            }
        }
    }
}
