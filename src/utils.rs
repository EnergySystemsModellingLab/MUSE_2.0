//! Utility functions.
use anyhow::{anyhow, Result};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::hash::Hash;

/// Inserts a key-value pair into a HashMap if the key does not already exist.
///
/// If the key already exists, it returns an error with a message indicating the key's existence.
pub fn try_insert<K, V>(map: &mut HashMap<K, V>, key: K, value: V) -> Result<()>
where
    K: Eq + Hash + std::fmt::Display,
{
    match map.entry(key) {
        Vacant(entry) => {
            entry.insert(value);
            Ok(())
        }
        Occupied(entry) => Err(anyhow!("Key {} already exists in the map", entry.key())),
    }
}
