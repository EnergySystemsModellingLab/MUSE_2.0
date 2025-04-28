//! Regions represent different geographical areas in which agents, processes, etc. are active.
use crate::id::define_id_getter;
use crate::id::define_id_type;
use indexmap::IndexMap;
use serde::Deserialize;

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
