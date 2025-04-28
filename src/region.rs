//! Regions represent different geographical areas in which agents, processes, etc. are active.
use crate::id::{define_id_getter, define_id_type, IDCollection};
use anyhow::{ensure, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashSet;

define_id_type! {RegionID}

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

/// Parse a string of regions separated by semicolons into a vector of RegionID.
///
/// The string can be either "all" (case-insensitive), a single region, or a semicolon-separated
/// list of regions (e.g. "GBR;FRA;USA" or "GBR; FRA; USA")
pub fn parse_region_str(s: &str, region_ids: &HashSet<RegionID>) -> Result<HashSet<RegionID>> {
    let s = s.trim();
    ensure!(!s.is_empty(), "No regions provided");

    if s.eq_ignore_ascii_case("all") {
        return Ok(region_ids.clone());
    }

    s.split(";")
        .map(|y| region_ids.get_id_by_str(y.trim()))
        .collect()
}
