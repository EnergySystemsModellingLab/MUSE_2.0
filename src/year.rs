#![allow(missing_docs)]
use anyhow::Result;
use serde::de::Deserializer;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(PartialEq, Debug, Clone)]
pub enum Year {
    All,
    Single(u32),
    Some(HashSet<u32>),
}

pub fn deserialize_year<'de, D>(deserialiser: D) -> Result<Year, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserialiser)?;
    if value == "all" {
        // "all" years specified
        Ok(Year::All)
    } else if let Ok(n) = value.parse::<u32>() {
        // Single year specified
        Ok(Year::Single(n))
    } else {
        // Semicolon-separated list of years
        let years: Result<HashSet<u32>, _> =
            value.split(';').map(|s| s.trim().parse::<u32>()).collect();
        match years {
            Ok(years_set) if !years_set.is_empty() => Ok(Year::Some(years_set)),
            _ => Err(serde::de::Error::custom("Invalid year format")),
        }
    }
}
