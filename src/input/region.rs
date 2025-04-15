//! Code for reading region-related information from CSV files.
use super::*;
use crate::id::{HasID, HasRegionID, IDCollection, IDLike};
use crate::region::{RegionID, RegionMap, RegionSelection};
use anyhow::{anyhow, ensure, Context, Result};
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
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
/// A `HashMap<Rc<str>, Region>` with the parsed regions data or an error. The keys are region IDs.
pub fn read_regions(model_dir: &Path) -> Result<RegionMap> {
    read_csv_id_file(&model_dir.join(REGIONS_FILE_NAME))
}

/// Read region IDs associated with a particular entity.
///
/// # Arguments
///
/// `file_path` - Path to CSV file
/// `entity_ids` - All possible valid IDs for the entity type
/// `region_ids` - All possible valid region IDs
pub fn read_regions_for_entity<T, ID: IDLike>(
    file_path: &Path,
    entity_ids: &HashSet<ID>,
    region_ids: &HashSet<RegionID>,
) -> Result<HashMap<ID, RegionSelection>>
where
    T: HasID<ID> + HasRegionID + DeserializeOwned,
{
    read_regions_for_entity_from_iter(read_csv::<T>(file_path)?, entity_ids, region_ids)
        .with_context(|| input_err_msg(file_path))
}

fn read_regions_for_entity_from_iter<I, T, ID: IDLike>(
    entity_iter: I,
    entity_ids: &HashSet<ID>,
    region_ids: &HashSet<RegionID>,
) -> Result<HashMap<ID, RegionSelection>>
where
    I: Iterator<Item = T>,
    T: HasID<ID> + HasRegionID,
{
    let mut entity_regions = HashMap::new();
    for entity in entity_iter {
        let entity_id = entity_ids.check_id(entity.get_id())?;
        let region_id = entity.get_region_id();

        try_insert_region(entity_id, region_id, region_ids, &mut entity_regions).context(
            "Invalid regions specified for entity. Must specify either unique region IDs or \"all\"."
        )?;
    }

    ensure!(
        entity_regions.len() >= entity_ids.len(),
        "At least one region must be specified per entity"
    );

    Ok(entity_regions)
}

/// Try to insert a region ID into the specified map
fn try_insert_region<ID: IDLike>(
    entity_id: ID,
    region_id: &RegionID,
    region_ids: &HashSet<RegionID>,
    entity_regions: &mut HashMap<ID, RegionSelection>,
) -> Result<()> {
    let entity_name = entity_id.clone();

    if region_id.0.eq_ignore_ascii_case("all") {
        // Valid for all regions
        return match entity_regions.insert(entity_id, RegionSelection::All) {
            None => Ok(()),
            Some(region_name) => Err(anyhow!(
                "Cannot specify both \"all\" and \"{}\" regions for \"{}\".",
                region_name,
                entity_name,
            )),
        };
    }

    // Validate region_id
    let region_id = region_ids.check_id(region_id)?;
    let region_name = region_id.clone();

    // Add or create entry in entity_regions
    let selection = entity_regions
        .entry(entity_id)
        .or_insert_with(|| RegionSelection::Some(HashSet::with_capacity(1)));

    match selection {
        RegionSelection::All => Err(anyhow!(
            "Cannot specify both \"{}\" and \"all\" regions for \"{}\".",
            region_name,
            entity_name
        )),
        RegionSelection::Some(ref mut set) => match set.insert(region_id) {
            true => Ok(()),
            false => Err(anyhow!(
                "Region \"{}\" specified multiple times for \"{}\".",
                region_name,
                entity_name
            )),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{define_id_getter, define_region_id_getter};
    use crate::region::Region;
    use serde::Deserialize;
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

    #[test]
    fn test_try_insert_region() {
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();

        // Insert new
        let mut entity_regions = HashMap::new();
        assert!(try_insert_region("key".into(), "GBR", &region_ids, &mut entity_regions).is_ok());
        let selected: HashSet<_> = ["GBR".into()].into_iter().collect();
        assert_eq!(
            *entity_regions.get("key").unwrap(),
            RegionSelection::Some(selected)
        );

        // Insert "all"
        let mut entity_regions = HashMap::new();
        assert!(try_insert_region("key".into(), "all", &region_ids, &mut entity_regions).is_ok());
        assert_eq!(*entity_regions.get("key").unwrap(), RegionSelection::All);

        // Append to existing
        let selected: HashSet<_> = ["FRA".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected.clone()))]
            .into_iter()
            .collect();
        assert!(try_insert_region("key".into(), "GBR", &region_ids, &mut entity_regions).is_ok());
        let selected: HashSet<_> = ["FRA".into(), "GBR".into()].into_iter().collect();
        assert_eq!(
            *entity_regions.get("key").unwrap(),
            RegionSelection::Some(selected)
        );

        // "All" already specified
        let mut entity_regions = [("key".into(), RegionSelection::All)].into_iter().collect();
        assert!(try_insert_region("key".into(), "GBR", &region_ids, &mut entity_regions).is_err());

        // "GBR" specified twice
        let selected: HashSet<_> = ["GBR".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected))]
            .into_iter()
            .collect();
        assert!(try_insert_region("key".into(), "GBR", &region_ids, &mut entity_regions).is_err());

        // Try appending "all" to existing
        let selected: HashSet<_> = ["FRA".into()].into_iter().collect();
        let mut entity_regions = [("key".into(), RegionSelection::Some(selected.clone()))]
            .into_iter()
            .collect();
        assert!(try_insert_region("key".into(), "all", &region_ids, &mut entity_regions).is_err());
    }

    #[derive(Deserialize, PartialEq)]
    struct Record {
        id: String,
        region_id: String,
    }
    define_id_getter! {Record, String}
    define_region_id_getter! {Record}

    #[test]
    fn test_read_regions_for_entity_from_iter() {
        let entity_ids = ["A".into(), "B".into()].into_iter().collect();
        let region_ids = ["GBR".into(), "FRA".into()].into_iter().collect();

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
        let actual = read_regions_for_entity_from_iter(iter, &entity_ids, &region_ids).unwrap();
        assert_eq!(expected, actual);

        // No region(s) specified for "B"
        let iter = [Record {
            id: "A".into(),
            region_id: "GBR".into(),
        }]
        .into_iter();
        assert!(read_regions_for_entity_from_iter(iter, &entity_ids, &region_ids).is_err());

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
        assert!(read_regions_for_entity_from_iter(iter, &entity_ids, &region_ids).is_err());
    }
}
