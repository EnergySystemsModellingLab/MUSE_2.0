/// Regions represent different areas in which processes are active.
use serde::Deserialize;
use std::rc::Rc;

/// Represents a region with an ID and a longer description.
#[derive(Debug, Deserialize, PartialEq)]
pub struct Region {
    /// A unique identifier for a region (e.g. "GBR").
    pub id: Rc<str>,
    /// A text description of the region (e.g. "United Kingdom").
    pub description: String,
}
