use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use csv::ReaderBuilder;
use serde::Deserialize;

/// Represents a demand entry.
#[derive(Debug, Deserialize)]
pub struct Demand {
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
    let mut csv_reader = ReaderBuilder::new()
        .has_headers(true)
        .from_reader(reader);

    // Parse the CSV data into a vector of `Demand` structs.
    let mut demand_data = Vec::new();
    for result in csv_reader.deserialize() {
        let demand: Demand = result?;
        demand_data.push(demand);
    }

    Ok(demand_data)
}

pub fn initialize_simulation() {
    let file_path = Path::new("demand.csv");

    let demands = read_demand_from_csv(file_path).unwrap_or_else(|err| {
        panic!("Error reading demand from CSV: {:?}", err);
    });

    // Your simulation initialization code here
    println!("Successfully initialized simulation with demands: {:?}", demands);
}
