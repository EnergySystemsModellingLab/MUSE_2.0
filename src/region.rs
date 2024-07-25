use crate::define_id_getter;
use crate::input::{read_csv_id_file, HasID, InputResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

const REGIONS_FILE_NAME: &str = "regions.csv";

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    pub id: Rc<str>,
    pub description: String,
}
define_id_getter! {Region}

/// Reads regions from a CSV file. UPDATE !!!!!!!!!!!!!!!!!!!1
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns a `Result` containing either a `Vec<Region>` with the parsed regions data
/// or an `InputError` if an error occurred.
///
/// # Errors
///
/// This function will return an error if the file cannot be opened or read, or if the CSV data
/// cannot be parsed.
pub fn read_regions(model_dir: &Path) -> InputResult<HashMap<Rc<str>, Region>> {
    read_csv_id_file(&model_dir.join(REGIONS_FILE_NAME))
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
        let regions_data = read_regions(dir.path()).unwrap();
        assert_eq!(
            regions_data,
            HashMap::from([
                (
                    "NA".into(),
                    Region {
                        id: "NA".into(),
                        description: "North America".to_string(),
                    }
                ),
                (
                    "EU".into(),
                    Region {
                        id: "EU".into(),
                        description: "Europe".to_string(),
                    }
                ),
                (
                    "AP".into(),
                    Region {
                        id: "AP".into(),
                        description: "Asia Pacific".to_string(),
                    }
                ),
            ])
        )
    }
}
