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
    entity_id: Rc<str>,
    region_id: &str,
    region_ids: &HashSet<Rc<str>>,
    entity_regions: &mut HashMap<Rc<str>, RegionSelection>,
) -> bool {
    if region_id.eq_ignore_ascii_case("all") {
        // Valid for all regions
        return entity_regions
            .insert(entity_id, RegionSelection::All)
            .is_none();
    }

    // Validate region_id
    let region_id = region_ids.get_id_checked(file_path, region_id);

    // Add or create entry in entity_regions
    let selection = entity_regions
        .entry(entity_id)
        .or_insert_with(|| RegionSelection::Some(HashSet::with_capacity(1)));

    match selection {
        RegionSelection::All => false,
        RegionSelection::Some(ref mut set) => set.insert(region_id),
    }
}

fn read_regions_for_entity_from_iter<I, T>(
    entity_iter: I,
    file_path: &Path,
    entity_ids: &HashSet<Rc<str>>,
    region_ids: &HashSet<Rc<str>>,
) -> HashMap<Rc<str>, RegionSelection>
where
    I: Iterator<Item = T>,
    T: HasID + HasRegionID,
{
    let mut entity_regions = HashMap::new();
    for entity in entity_iter {
        let entity_id = entity_ids.get_id_checked(file_path, entity.get_id());
        let region_id = entity.get_region_id();

        let succeeded = try_insert_region(
            file_path,
            entity_id,
            region_id,
            region_ids,
            &mut entity_regions,
        );

        if !succeeded {
            input_panic(
                file_path,
                "Invalid regions specified for entity. \
                 Must specify either unique region IDs or \"all\".",
            )
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
    read_regions_for_entity_from_iter(read_csv::<T>(file_path), file_path, entity_ids, region_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::panic::catch_unwind;
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
    fn test_read_regions() {
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

    #[test]
    fn test_try_insert_region() {
        let p = PathBuf::new();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();

        // Insert new
        let mut entity_regions = HashMap::new();
        assert!(try_insert_region(
            &p,
            "key".into(),
            "GBR",
            &region_ids,
            &mut entity_regions
        ));
        let selected: HashSet<_> = ["GBR".into()].into_iter().collect();
        assert_eq!(
            *entity_regions.get("key").unwrap(),
            RegionSelection::Some(selected)
        );

        // Insert "all"
        let mut entity_regions = HashMap::new();
        assert!(try_insert_region(
            &p,
            "key".into(),
            "all",
            &region_ids,
            &mut entity_regions
        ));
        assert_eq!(*entity_regions.get("key").unwrap(), RegionSelection::All);

        // Append to existing
        let selected: HashSet<_> = ["FRA".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected.clone()))]
            .into_iter()
            .collect();
        assert!(try_insert_region(
            &p,
            "key".into(),
            "GBR",
            &region_ids,
            &mut entity_regions
        ));
        let selected: HashSet<_> = ["FRA".into(), "GBR".into()].into_iter().collect();
        assert_eq!(
            *entity_regions.get("key").unwrap(),
            RegionSelection::Some(selected)
        );

        // "All" already specified
        let mut entity_regions = [("key".into(), RegionSelection::All)].into_iter().collect();
        assert!(!try_insert_region(
            &p,
            "key".into(),
            "GBR",
            &region_ids,
            &mut entity_regions
        ));

        // "GBR" specified twice
        let selected: HashSet<_> = ["GBR".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected))]
            .into_iter()
            .collect();
        assert!(!try_insert_region(
            &p,
            "key".into(),
            "GBR",
            &region_ids,
            &mut entity_regions
        ));

        // Try appending "all" to existing
        let selected: HashSet<_> = ["FRA".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected.clone()))]
            .into_iter()
            .collect();
        assert!(!try_insert_region(
            &p,
            "key".into(),
            "all",
            &region_ids,
            &mut entity_regions
        ));
    }

    #[derive(Deserialize, PartialEq)]
    struct Record {
        id: String,
        region_id: String,
    }
    define_id_getter! {Record}
    define_region_id_getter! {Record}

    #[test]
    fn test_read_regions_for_entity_from_iter() {
        let p = PathBuf::new();
        let entity_ids = ["A".into(), "B".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();

        macro_rules! assert_panics {
            ($e:expr) => {
                assert!(catch_unwind(|| $e).is_err())
            };
        }

        // Valid case
        let iter = [
            Record {
                id: "A".into(),
                region_id: "GBR".into(),
            },
            Record {
                id: "B".into(),
                region_id: "FRA".into(),
            },
        ]
        .into_iter();
        let expected = HashMap::from_iter([
            (
                "A".into(),
                RegionSelection::Some(HashSet::from_iter(["GBR".into()])),
            ),
            (
                "B".into(),
                RegionSelection::Some(HashSet::from_iter(["FRA".into()])),
            ),
        ]);
        let actual = read_regions_for_entity_from_iter(iter, &p, &entity_ids, &region_ids);
        assert_eq!(expected, actual);

        // No region(s) specified for "B"
        let iter = [Record {
            id: "A".into(),
            region_id: "GBR".into(),
        }]
        .into_iter();
        assert_panics!(read_regions_for_entity_from_iter(
            iter,
            &p,
            &entity_ids,
            &region_ids
        ));

        // Make try_insert_region fail
        let iter = [
            Record {
                id: "A".into(),
                region_id: "GBR".into(),
            },
            Record {
                id: "B".into(),
                region_id: "FRA".into(),
            },
            Record {
                id: "A".into(),
                region_id: "all".into(),
            },
        ]
        .into_iter();
        assert_panics!(read_regions_for_entity_from_iter(
            iter,
            &p,
            &entity_ids,
            &region_ids
        ));
    }
}
