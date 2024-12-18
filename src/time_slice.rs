//! Code for working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
#![allow(missing_docs)]
use crate::input::*;
use anyhow::{Context, Result};
use itertools::Itertools;
use serde_string_enum::DeserializeLabeledStringEnum;
use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::iter;
use std::rc::Rc;

/// An ID describing season and time of day
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct TimeSliceID {
    /// The name of each season.
    pub season: Rc<str>,
    /// The name of each time slice within a day.
    pub time_of_day: Rc<str>,
}

impl Display for TimeSliceID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.season, self.time_of_day)
    }
}

/// Represents a time slice read from an input file, which can be all
#[derive(PartialEq, Debug)]
pub enum TimeSliceSelection {
    /// All year and all day
    Annual,
    /// Only applies to one season
    Season(Rc<str>),
    /// Only applies to a single time slice
    Single(TimeSliceID),
}

/// Information about the time slices in the simulation, including names and fractions
#[derive(PartialEq, Debug)]
pub struct TimeSliceInfo {
    /// Names of seasons
    pub seasons: HashSet<Rc<str>>,
    /// Names of times of day (e.g. "evening")
    pub times_of_day: HashSet<Rc<str>>,
    /// The fraction of the year that this combination of season and time of day occupies
    pub fractions: HashMap<TimeSliceID, f64>,
}

impl Default for TimeSliceInfo {
    /// The default `TimeSliceInfo` is a single time slice covering the whole year
    fn default() -> Self {
        let id = TimeSliceID {
            season: "all-year".into(),
            time_of_day: "all-day".into(),
        };
        let fractions = [(id.clone(), 1.0)].into_iter().collect();

        Self {
            seasons: [id.season].into_iter().collect(),
            times_of_day: [id.time_of_day].into_iter().collect(),
            fractions,
        }
    }
}

impl TimeSliceInfo {
    /// Get the `TimeSliceID` corresponding to the `time_slice`.
    ///
    /// `time_slice` must be in the form "season.time_of_day".
    pub fn get_time_slice_id_from_str(&self, time_slice: &str) -> Result<TimeSliceID> {
        let (season, time_of_day) = time_slice
            .split('.')
            .collect_tuple()
            .context("Time slice must be in the form season.time_of_day")?;
        let season = self
            .seasons
            .iter()
            .find(|item| item.eq_ignore_ascii_case(season))
            .with_context(|| format!("{} is not a known season", season))?;
        let time_of_day = self
            .times_of_day
            .iter()
            .find(|item| item.eq_ignore_ascii_case(time_of_day))
            .with_context(|| format!("{} is not a known time of day", time_of_day))?;

        Ok(TimeSliceID {
            season: Rc::clone(season),
            time_of_day: Rc::clone(time_of_day),
        })
    }

    /// Get a `TimeSliceSelection` from the specified string.
    ///
    /// If the string is empty, the default value is `TimeSliceSelection::Annual`.
    pub fn get_selection(&self, time_slice: &str) -> Result<TimeSliceSelection> {
        if time_slice.is_empty() || time_slice.eq_ignore_ascii_case("annual") {
            Ok(TimeSliceSelection::Annual)
        } else if time_slice.contains('.') {
            let time_slice = self.get_time_slice_id_from_str(time_slice)?;
            Ok(TimeSliceSelection::Single(time_slice))
        } else {
            let season = self.seasons.get_id(time_slice)?;
            Ok(TimeSliceSelection::Season(season))
        }
    }

    /// Iterate over all [`TimeSliceID`]s.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter(&self) -> impl Iterator<Item = &TimeSliceID> {
        self.fractions.keys()
    }

    /// Iterate over the subset of [`TimeSliceID`] indicated by `selection`.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter_selection<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
    ) -> Box<dyn Iterator<Item = &'a TimeSliceID> + 'a> {
        match selection {
            TimeSliceSelection::Annual => Box::new(self.iter()),
            TimeSliceSelection::Season(season) => {
                Box::new(self.iter().filter(move |ts| ts.season == *season))
            }
            TimeSliceSelection::Single(ts) => Box::new(iter::once(ts)),
        }
    }
}

/// Refers to a particular aspect of a time slice
#[derive(PartialEq, Debug, DeserializeLabeledStringEnum)]
pub enum TimeSliceLevel {
    #[string = "annual"]
    Annual,
    #[string = "season"]
    Season,
    #[string = "daynight"]
    DayNight,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_selection() {
        let slices = [
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            TimeSliceID {
                season: "summer".into(),
                time_of_day: "night".into(),
            },
        ];
        let ts_info = TimeSliceInfo {
            seasons: ["winter".into(), "summer".into()].into_iter().collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            fractions: [(slices[0].clone(), 0.5), (slices[1].clone(), 0.5)]
                .into_iter()
                .collect(),
        };

        assert_eq!(
            HashSet::<&TimeSliceID>::from_iter(ts_info.iter_selection(&TimeSliceSelection::Annual)),
            HashSet::from_iter(slices.iter())
        );
        itertools::assert_equal(
            ts_info.iter_selection(&TimeSliceSelection::Season("winter".into())),
            iter::once(&slices[0]),
        );
        let ts = ts_info.get_time_slice_id_from_str("summer.night").unwrap();
        itertools::assert_equal(
            ts_info.iter_selection(&TimeSliceSelection::Single(ts)),
            iter::once(&slices[1]),
        );
    }
}
