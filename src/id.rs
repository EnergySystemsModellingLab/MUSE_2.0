//! Code for handing IDs
use crate::region::RegionID;
use anyhow::{Context, Result};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt::Display;
use std::hash::Hash;

/// Indicates that the struct has an ID field
pub trait HasID<ID>
where
    ID: Eq + Hash + Borrow<str>,
{
    /// Get the struct's ID
    fn get_id(&self) -> &ID;
}

/// An object which is associated with a single region
pub trait HasRegionID {
    /// Get the associated region ID
    fn get_region_id(&self) -> &RegionID;
}

/// Implement the `HasID` trait for the given type, assuming it has a field called `id`
macro_rules! define_id_getter {
    ($t:ty, $id_ty:ty) => {
        impl crate::id::HasID<$id_ty> for $t {
            fn get_id(&self) -> &$id_ty {
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
            fn get_region_id(&self) -> &RegionID {
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
    /// A copy of the `ID` in `self` or an error if not found.
    fn get_id(&self, id: &str) -> Result<ID>;

    /// Check that the ID exists in this collection.
    fn check_id(&self, id: &ID) -> Result<ID>;
}

impl<ID> IDCollection<ID> for HashSet<ID>
where
    ID: Eq + Hash + Borrow<str> + Clone + Display,
{
    fn get_id(&self, id: &str) -> Result<ID> {
        let found = self
            .get(id)
            .with_context(|| format!("Unknown ID {id} found"))?;
        Ok(found.clone())
    }

    fn check_id(&self, id: &ID) -> Result<ID> {
        let found = self
            .get(id.borrow())
            .with_context(|| format!("Unknown ID {id} found"))?;
        Ok(found.clone())
    }
}
