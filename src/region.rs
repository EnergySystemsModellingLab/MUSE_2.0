use crate::input::{input_panic, read_csv_id_file, HasID};
use serde::Deserialize;
use std::cmp::Eq;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

const REGIONS_FILE_NAME: &str = "regions.csv";

macro_rules! define_id_type {
    ($t:ident) => {
        #[derive(Debug, Deserialize, Eq, PartialEq, Hash)]
        pub struct $t(Rc<str>);

        impl $t {
            pub fn create_valid(id: &str, ids: &HashSet<Rc<str>>, file_path: &Path) -> Self {
                match ids.get(id) {
                    None => input_panic(file_path, &format!("Unknown ID {id} found")),
                    Some(id) => Self(Rc::clone(&id)),
                }
            }
        }

        impl IDType for $t {
            fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

pub trait IDType {
    fn as_str(&self) -> &str;
}

// #[derive(Debug, Deserialize, PartialEq)]
// pub struct RegionID(Rc<str>);
define_id_type! {RegionID}

// impl RegionID {
//     pub fn create_valid(id: &str, ids: &HashSet<Rc<str>>, file_path: &Path) -> Self {
//         match ids.get(id) {
//             None => input_panic(file_path, &format!("Unknown ID {id} found")),
//             Some(id) => Self(Rc::clone(id)),
//         }
//     }
// }

// impl IDType for RegionID {
//     fn as_str(&self) -> &str {
//         &self.0
//     }
// }

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    pub id: RegionID,
    pub description: String,
}

impl HasID for Region {
    fn get_id(&self) -> &str {
        RegionID("hello".into());
        &self.id.0
    }
}

/// Reads regions from a CSV file.
///
/// # Arguments
///
/// * `model_dir` - Folder containing model configuration files
///
/// # Returns
///
/// This function returns a `HashMap<RegionID, Region>` with the parsed regions data. The keys are
/// region IDs.
pub fn read_regions(model_dir: &Path) -> HashMap<RegionID, Region> {
    read_csv_id_file(&model_dir.join(REGIONS_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
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
        let p = PathBuf::new();
        let dir = tempdir().unwrap();
        create_regions_file(dir.path());
        let regions = read_regions(dir.path());
        let ids = HashSet::from_iter(regions.keys().cloned());
        assert_eq!(
            regions,
            HashMap::from([
                (
                    "NA".into(),
                    Region {
                        id: RegionID::create_valid("NA", &ids, &p),
                        description: "North America".to_string(),
                    }
                ),
                (
                    "EU".into(),
                    Region {
                        id: RegionID::create_valid("EU", &ids, &p),
                        description: "Europe".to_string(),
                    }
                ),
                (
                    "AP".into(),
                    Region {
                        id: RegionID::create_valid("AP", &ids, &p),
                        description: "Asia Pacific".to_string(),
                    }
                ),
            ])
        )
    }
}
