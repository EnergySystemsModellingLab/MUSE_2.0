use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::path::Path;

/// Represents a region with a short name and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    pub short_name: String,
    pub description: String,
}

pub fn read_regions_from_csv(file_path: &Path) -> Result<Vec<Region>, Box<dyn Error>> {
    let file = File::open(file_path)?;
    let mut rdr = csv::Reader::from_reader(file);
    let mut regions = Vec::new();

    for result in rdr.deserialize() {
        let region: Region = result?;
        regions.push(region);
    }

    Ok(regions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A temporary CSV file for testing.
    fn create_temp_csv(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_read_regions_from_csv() {
        let csv_content = "\
short_name,description
NA,North America
EU,Europe
AP,Asia Pacific";

        let file = create_temp_csv(csv_content);
        let regions = read_regions_from_csv(file.path()).expect("Failed to read regions from CSV");

        let expected_regions = vec![
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
        ];

        assert_eq!(regions, expected_regions);
    }

    #[test]
    fn test_read_empty_csv() {
        let csv_content = "short_name,description\n";

        let file = create_temp_csv(csv_content);
        let regions = read_regions_from_csv(file.path()).expect("Failed to read regions from CSV");

        assert!(regions.is_empty());
    }

    #[test]
    fn test_read_invalid_csv() {
        let csv_content = "short_name,description\nNA,North America\ninvalid,line";

        let file = create_temp_csv(csv_content);
        let result = read_regions_from_csv(file.path());

        assert!(result.is_err());
    }
}
