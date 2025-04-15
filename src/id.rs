//! Code for handing IDs
use anyhow::{Context, Result};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::hash::Hash;

/// Indicates that the struct has an ID field
pub trait HasID {
    /// Get a string representation of the struct's ID
    fn get_id(&self) -> &str;
}

/// An object which is associated with a single region
pub trait HasRegionID {
    /// Get the associated region ID
    fn get_region_id(&self) -> &str;
}

/// Implement the `HasID` trait for the given type, assuming it has a field called `id`
macro_rules! define_id_getter {
    ($t:ty) => {
        impl crate::id::HasID for $t {
            fn get_id(&self) -> &str {
                &self.id
            }
        }
    };
}
pub(crate) use define_id_getter;

/// Implement the `HasRegionID` trait for the given type, assuming it has a field called `region_id`
macro_rules! define_region_id_getter {
    ($t:ty) => {
        impl crate::id::HasRegionID for $t {
            fn get_region_id(&self) -> &str {
                &self.region_id
            }
        }
    };
}
pub(crate) use define_region_id_getter;

/// A data structure containing a set of IDs
pub trait IDCollection<ID>
where
    ID: Eq + Hash + Borrow<str>,
{
    /// Get the ID after checking that it exists this collection.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID to look up
    ///
    /// # Returns
    ///
    /// A copy of the `Rc<str>` in `self` or an error if not found.
    fn get_id(&self, id: &str) -> Result<ID>;
}

impl<ID> IDCollection<ID> for HashSet<ID>
where
    ID: Eq + Hash + Borrow<str> + Clone,
{
    fn get_id(&self, id: &str) -> Result<ID> {
        let found = self
            .get(id)
            .with_context(|| format!("Unknown ID {id} found"))?;
        Ok(found.clone())
    }
}
