//! Code for handling IDs
use anyhow::{Context, Result};
use indexmap::IndexSet;
use std::collections::HashSet;

/// A trait alias for ID types
pub trait IDLike:
    Eq + std::hash::Hash + std::borrow::Borrow<str> + Clone + std::fmt::Display + From<String>
{
}
impl<T> IDLike for T where
    T: Eq + std::hash::Hash + std::borrow::Borrow<str> + Clone + std::fmt::Display + From<String>
{
}

macro_rules! define_id_type {
    ($name:ident) => {
        #[derive(
            Clone, std::hash::Hash, PartialEq, Eq, serde::Deserialize, Debug, serde::Serialize,
        )]
        /// An ID type (e.g. `AgentID`, `CommodityID`, etc.)
        pub struct $name(pub std::rc::Rc<str>);

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                $name(std::rc::Rc::from(s))
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                $name(std::rc::Rc::from(s))
            }
        }

        impl $name {
            /// Create a new ID from a string slice
            pub fn new(id: &str) -> Self {
                $name(std::rc::Rc::from(id))
            }
        }
    };
}
pub(crate) use define_id_type;

#[cfg(test)]
define_id_type!(GenericID);

/// Indicates that the struct has an ID field
pub trait HasID<ID: IDLike> {
    /// Get the struct's ID
    fn get_id(&self) -> &ID;
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

/// A data structure containing a set of IDs
pub trait IDCollection<ID: IDLike> {
    /// Get the ID from the collection by its string representation.
    ///
    /// # Arguments
    ///
    /// * `id` - The string representation of the ID
    ///
    /// # Returns
    ///
    /// A copy of the ID in `self`, or an error if not found.
    fn get_id_by_str(&self, id: &str) -> Result<ID>;

    /// Check if the ID is in the collection, returning a copy of it if found.
    ///
    /// # Arguments
    ///
    /// * `id` - The ID to check
    ///
    /// # Returns
    ///
    /// A copy of the ID in `self`, or an error if not found.
    fn get_id(&self, id: &ID) -> Result<ID>;
}

macro_rules! define_id_methods {
    () => {
        fn get_id_by_str(&self, id: &str) -> Result<ID> {
            let found = self
                .get(id)
                .with_context(|| format!("Unknown ID {id} found"))?;
            Ok(found.clone())
        }

        fn get_id(&self, id: &ID) -> Result<ID> {
            let found = self
                .get(id.borrow())
                .with_context(|| format!("Unknown ID {id} found"))?;
            Ok(found.clone())
        }
    };
}

impl<ID: IDLike> IDCollection<ID> for HashSet<ID> {
    define_id_methods!();
}

impl<ID: IDLike> IDCollection<ID> for IndexSet<ID> {
    define_id_methods!();
}
