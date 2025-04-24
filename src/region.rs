//! Regions represent different geographical areas in which agents, processes, etc. are active.
use crate::id::define_id_getter;
use crate::id::define_id_type;
use indexmap::IndexMap;
use itertools::Itertools;
use serde::de::Deserializer;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Display;
use std::str::FromStr;

define_id_type! {RegionID}

impl FromStr for RegionID {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(RegionID::from(s))
    }
}

/// A map of [`Region`]s, keyed by region ID
pub type RegionMap = IndexMap<RegionID, Region>;

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    /// A unique identifier for a region (e.g. "GBR").
    pub id: RegionID,
    /// A text description of the region (e.g. "United Kingdom").
    pub description: String,
}
define_id_getter! {Region, RegionID}

/// Represents multiple regions
#[derive(PartialEq, Debug, Clone, Default)]
pub enum RegionSelection {
    /// All regions are covered
    #[default]
    All,
    /// Only some regions are covered
    Some(HashSet<RegionID>),
}

impl RegionSelection {
    /// Returns true if the [`RegionSelection`] covers a given region
    pub fn contains(&self, region_id: &RegionID) -> bool {
        match self {
            Self::All => true,
            Self::Some(regions) => regions.contains(region_id),
        }
    }
}

impl Display for RegionSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Some(regions) => write!(f, "{}", regions.iter().join(", ")),
        }
    }
}

/// Deserialises a region selection from a string. The string can be either "all", a single region, or a
/// semicolon-separated list of regions (e.g. "GBR;FRA;ESP" or "GBR; FRA; ESP").
pub fn deserialize_region<'de, D>(deserialiser: D) -> Result<RegionSelection, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserialiser)?;
    if value.trim().eq_ignore_ascii_case("all") {
        // "all" regions specified
        Ok(RegionSelection::All)
    } else {
        // Semicolon-separated list of regions
        let regions: Result<HashSet<RegionID>, _> = value
            .split(';')
            .map(|s| s.trim().parse::<RegionID>())
            .collect();
        match regions {
            Ok(regions_set) if !regions_set.is_empty() => Ok(RegionSelection::Some(regions_set)),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid region format: {}",
                value
            ))),
        }
    }
}
