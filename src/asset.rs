//! Code for working with [Asset]s.
//!
//! For a description of what assets are, please see the glossary.
use crate::process::Process;
use std::rc::Rc;

mod input;
pub use input::read_assets;

/// An asset controlled by an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    /// The [Process] that this asset corresponds to
    pub process: Rc<Process>,
    /// The region in which the asset is located
    pub region_id: Rc<str>,
    /// Capacity of asset
    pub capacity: f64,
    /// The year the asset comes online
    pub commission_year: u32,
}
