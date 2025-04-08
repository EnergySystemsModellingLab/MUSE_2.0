#![allow(missing_docs)]
use anyhow::{bail, Result};
use serde::de::Deserializer;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum AnnualField<T> {
    Empty,
    Constant(T),
    Variable(HashMap<u32, T>),
}

impl<T: Clone> AnnualField<T> {
    pub fn get(&self, year: u32) -> &T {
        match self {
            AnnualField::Empty => panic!("AnnualField is empty."),
            AnnualField::Constant(value) => value,
            AnnualField::Variable(values) => values.get(&year).unwrap(),
        }
    }

    pub fn insert(&mut self, year: u32, value: T) -> Result<()> {
        match self {
            AnnualField::Constant(_) => {
                bail!("Cannot insert into a constant field.");
            }
            AnnualField::Variable(values) => {
                if values.contains_key(&year) {
                    bail!("Year {} already exists in variable field.", year);
                }
                values.insert(year, value);
                Ok(())
            }
            AnnualField::Empty => {
                *self = AnnualField::Variable(HashMap::new());
                self.insert(year, value)
            }
        }
    }

    pub fn merge(&mut self, other: &Self) -> Result<()> {
        match (self, other) {
            (AnnualField::Variable(values), AnnualField::Variable(other_values)) => {
                for (year, value) in other_values.iter() {
                    if values.contains_key(year) {
                        bail!("Year {} already exists in variable field.", year);
                    }
                    values.insert(*year, value.clone());
                }
                Ok(())
            }
            (AnnualField::Constant(_), AnnualField::Constant(_)) => {
                bail!("Cannot merge two constant fields.")
            }
            _ => bail!("Cannot merge constant and variable fields."),
        }
    }

    pub fn contains(&self, year: &u32) -> bool {
        match self {
            AnnualField::Empty => false,
            AnnualField::Constant(_) => true,
            AnnualField::Variable(values) => values.contains_key(year),
        }
    }

    pub fn check_reference(&self, reference_years: &HashSet<u32>) -> Result<()> {
        match self {
            AnnualField::Empty => {
                bail!("AnnualField is empty. Cannot check reference years.");
            }
            AnnualField::Constant(_) => Ok(()),
            AnnualField::Variable(_) => {
                for year in reference_years.iter() {
                    if !self.contains(year) {
                        bail!("Missing data for year {}.", year);
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum Year {
    All,
    Single(u32),
}

pub fn deserialize_year<'de, D>(deserialiser: D) -> Result<Year, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserialiser)?;
    if value == "all" {
        Ok(Year::All)
    } else if let Ok(n) = value.parse::<u32>() {
        Ok(Year::Single(n))
    } else {
        Err(serde::de::Error::custom("Invalid year format"))
    }
}
