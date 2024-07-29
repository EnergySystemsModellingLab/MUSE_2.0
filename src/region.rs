use crate::input::read_csv_as_vec;
use serde::Deserialize;
use std::path::Path;

const REGIONS_FILE_NAME: &str = "regions.csv";

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    pub id: String,
    pub description: String,
}

/// Reads regions data from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns a `Vec<Region>` with the parsed regions data
pub fn read_regions(model_dir: &Path) -> Vec<Region> {
    read_csv_as_vec(&model_dir.join(REGIONS_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use tempfile::tempdir;

    /// Create an example regions file in dir_path
    fn create_regions_file(dir_path: &Path) {
        let file_path = dir_path.join(REGIONS_FILE_NAME);
        let mut file = File::create(file_path).unwrap();
        writeln!(
            file,
            "id,description
NA,North America
EU,Europe
AP,Asia Pacific"
        )
        .unwrap();
    }

    #[test]
    fn test_read_regions_from_csv() {
        let dir = tempdir().unwrap();
        create_regions_file(dir.path());
        let regions = read_regions(dir.path());
        assert_eq!(
            regions,
            vec![
                Region {
                    id: "NA".to_string(),
                    description: "North America".to_string(),
                },
                Region {
                    id: "EU".to_string(),
                    description: "Europe".to_string(),
                },
                Region {
                    id: "AP".to_string(),
                    description: "Asia Pacific".to_string(),
                },
            ]
        )
    }
}
