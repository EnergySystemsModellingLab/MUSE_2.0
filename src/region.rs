use crate::input::*;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
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

#[derive(PartialEq, Debug)]
pub enum RegionSelection {
    All,
    Some(HashSet<Rc<str>>),
}

impl RegionSelection {
    pub fn contains(&self, region_id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Some(regions) => regions.contains(region_id),
        }
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
/// This function returns a `HashMap<Rc<str>, Region>` with the parsed regions data. The keys are
/// region IDs.
pub fn read_regions(model_dir: &Path) -> HashMap<Rc<str>, Region> {
    read_csv_id_file(&model_dir.join(REGIONS_FILE_NAME))
}

pub trait HasRegionID {
    fn get_region_id(&self) -> &str;
}

macro_rules! define_region_id_getter {
    ($t:ty) => {
        impl HasRegionID for $t {
            fn get_region_id(&self) -> &str {
                &self.region_id
            }
        }
    };
}

pub(crate) use define_region_id_getter;

/// Try to insert a region ID into the specified map
#[must_use]
fn try_insert_region(
    file_path: &Path,
    key: Rc<str>,
    region_id: &str,
    region_ids: &HashSet<Rc<str>>,
    entity_regions: &mut HashMap<Rc<str>, RegionSelection>,
) -> bool {
    if region_id.eq_ignore_ascii_case("all") {
        // Valid for all regions
        return entity_regions.insert(key, RegionSelection::All).is_none();
    }

    // Validate region_id
    let region_id = region_ids.get_id_checked(file_path, region_id);

    // Add or create entry in entity_regions
    let selection = entity_regions
        .entry(key)
        .or_insert_with(|| RegionSelection::Some(HashSet::with_capacity(1)));

    match selection {
        RegionSelection::All => false,
        RegionSelection::Some(ref mut set) => set.insert(region_id),
    }
}

/// Read region IDs associated with a particular entity.
///
/// # Arguments
///
/// `file_path` - Path to CSV file
/// `entity_ids` - All possible valid IDs for the entity type
/// `region_ids` - All possible valid region IDs
pub fn read_regions_for_entity<T>(
    file_path: &Path,
    entity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, RegionSelection>
where
    T: HasID + HasRegionID + DeserializeOwned,
{
    let mut entity_regions = HashMap::new();
    for record in read_csv::<T>(file_path) {
        let key = entity_ids.get_id_checked(file_path, record.get_id());
        let region_id = record.get_region_id();

        let succeeded =
            try_insert_region(file_path, key, region_id, region_ids, &mut entity_regions);

        if !succeeded {
            input_panic(file_path, "Invalid regions specified for entity. Must specify either unique region IDs or \"all\".")
        }
    }

    if entity_regions.len() < entity_ids.len() {
        input_panic(
            file_path,
            "At least one region must be specified per entity",
        );
    }

    entity_regions
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
