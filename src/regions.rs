use crate::input::{read_vec_from_csv, InputError};
use serde::Deserialize;
use std::path::Path;

/// Represents a region with a short name and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    pub short_name: String,
    pub description: String,
}

/// Reads regions data from a CSV file.
///
/// # Arguments
///
/// * `file_path` - A reference to the path of the CSV file to read from.
///
/// # Returns
///
/// This function returns a `Result` containing either a `Vec<Region>` with the parsed regions data
/// or a `Box<dyn Error>` if an error occurred.
///
/// # Errors
///
/// This function will return an error if the file cannot be opened or read, or if the CSV data
/// cannot be parsed.
pub fn read_regions_data(file_path: &Path) -> Result<Vec<Region>, InputError> {
    let regions_data = read_vec_from_csv(file_path)?;

    if regions_data.is_empty() {
        Err(InputError::new(
            file_path,
            "Regions data file cannot be empty",
        ))?;
    }

    Ok(regions_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    /// Create an example regions file in dir_path
    fn create_regions_file(dir_path: &Path) -> PathBuf {
        let file_path = dir_path.join("regions.csv");
        let mut file = File::create(&file_path).unwrap();
        writeln!(
            file,
            "short_name,description
NA,North America
EU,Europe
AP,Asia Pacific"
        )
        .unwrap();
        file_path
    }

    #[test]
    fn test_read_regions_from_csv() {
        let dir = tempdir().unwrap();
        let file_path = create_regions_file(dir.path());
        let regions_data = read_regions_data(&file_path).unwrap();
        assert_eq!(
            regions_data,
            vec![
                Region {
                    short_name: "NA".to_string(),
                    description: "North America".to_string(),
                },
                Region {
                    short_name: "EU".to_string(),
                    description: "Europe".to_string(),
                },
                Region {
                    short_name: "AP".to_string(),
                    description: "Asia Pacific".to_string(),
                },
            ]
        )
    }
}
