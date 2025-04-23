//! Code for working with years.
use anyhow::Result;
use serde::de::Deserializer;
use serde::Deserialize;
use std::collections::HashSet;

/// Represents a set of years.
#[derive(PartialEq, Debug, Clone)]
pub enum YearSelection {
    /// Covers all years. It's up to the user to interpret this (e.g. could be all milestone years,
    /// or all active years for a process etc.)
    All,
    /// Covers some years.
    Some(HashSet<u32>),
}

/// Deserialises a year selection from a string. The string can be either "all", a single year, or a
/// semicolon-separated list of years (e.g. "2020;2021;2022" or "2020; 2021; 2022").
pub fn deserialize_year<'de, D>(deserialiser: D) -> Result<YearSelection, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserialiser)?;
    if value.trim().eq_ignore_ascii_case("all") {
        // "all" years specified
        Ok(YearSelection::All)
    } else {
        // Semicolon-separated list of years
        let years: Result<HashSet<u32>, _> =
            value.split(';').map(|s| s.trim().parse::<u32>()).collect();
        match years {
            Ok(years_set) if !years_set.is_empty() => Ok(YearSelection::Some(years_set)),
            _ => Err(serde::de::Error::custom(format!(
                "Invalid year format: {}",
                value
            ))),
        }
    }
}
