//! Regions represent different geographical areas in which agents, processes, etc. are active.
use indexmap::IndexMap;
use itertools::Itertools;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Display;
use std::rc::Rc;

/// A map of [`Region`]s, keyed by region ID
pub type RegionMap = IndexMap<Rc<str>, Region>;

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    /// A unique identifier for a region (e.g. "GBR").
    pub id: Rc<str>,
    /// A text description of the region (e.g. "United Kingdom").
    pub description: String,
}

/// Represents multiple regions
#[derive(PartialEq, Debug, Clone, Default)]
pub enum RegionSelection {
    /// All regions are covered
    #[default]
    All,
    /// Only some regions are covered
    Some(HashSet<Rc<str>>),
}

impl RegionSelection {
    /// Returns true if the [`RegionSelection`] covers a given region
    pub fn contains(&self, region_id: &str) -> bool {
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
