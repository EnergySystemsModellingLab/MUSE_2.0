//! Code for reading region-related information from CSV files.
use super::*;
use crate::id::IDCollection;
use crate::region::{RegionID, RegionMap};
use anyhow::ensure;
use std::collections::HashSet;
use std::path::Path;

const REGIONS_FILE_NAME: &str = "regions.csv";

/// Reads regions from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// A `HashMap<RegionID, Region>` with the parsed regions data or an error
pub fn read_regions(model_dir: &Path) -> Result<RegionMap> {
    read_csv_id_file(&model_dir.join(REGIONS_FILE_NAME))
}

/// Parse a string of regions separated by semicolons into a vector of RegionID.
///
/// The string can be either "all" (case-insensitive), a single region, or a semicolon-separated
/// list of regions (e.g. "GBR;FRA;USA" or "GBR; FRA; USA")
pub fn parse_region_str(s: &str, region_ids: HashSet<RegionID>) -> Result<HashSet<RegionID>> {
    let s = s.trim();
    ensure!(!s.is_empty(), "No regions provided");

    if s.eq_ignore_ascii_case("all") {
        return Ok(region_ids.clone());
    }

    s.split(";")
        .map(|y| region_ids.get_id_by_str(y.trim()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Region;
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
    fn test_read_regions() {
        let dir = tempdir().unwrap();
        create_regions_file(dir.path());
        let regions = read_regions(dir.path()).unwrap();
        assert_eq!(
            regions,
            RegionMap::from([
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
