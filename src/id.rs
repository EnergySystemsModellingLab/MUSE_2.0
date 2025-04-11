//! Code for handing IDs
use anyhow::{ensure, Context, Result};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
pub trait IDCollection {
    /// Get the ID after checking that it exists this collection.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID to look up
    ///
    /// # Returns
    ///
    /// A copy of the `Rc<str>` in `self` or an error if not found.
    fn get_id(&self, id: &str) -> Result<Rc<str>>;
}

impl IDCollection for HashSet<Rc<str>> {
    fn get_id(&self, id: &str) -> Result<Rc<str>> {
        let id = self
            .get(id)
            .with_context(|| format!("Unknown ID {id} found"))?;
        Ok(Rc::clone(id))
    }
}

/// Trait for converting an iterator into a [`HashMap`] grouped by IDs.
pub trait IntoIDMap<T> {
    /// Convert into a [`HashMap`] grouped by IDs.
    fn into_id_map(self, ids: &HashSet<Rc<str>>) -> Result<HashMap<Rc<str>, Vec<T>>>;
}

impl<T, I> IntoIDMap<T> for I
where
    T: HasID,
    I: Iterator<Item = T>,
{
    /// Convert the specified iterator into a `HashMap` of the items grouped by ID.
    ///
    /// # Arguments
    ///
    /// `ids` - The set of valid IDs to check against.
    fn into_id_map(self, ids: &HashSet<Rc<str>>) -> Result<HashMap<Rc<str>, Vec<T>>> {
        let map = self
            .map(|item| -> Result<_> {
                let id = ids.get_id(item.get_id())?;
                Ok((id, item))
            })
            .process_results(|iter| iter.into_group_map())?;

        ensure!(!map.is_empty(), "CSV file is empty");

        Ok(map)
    }
}
