//! Code for working with time slices.
//!
//! Time slices provide a mechanism for users to indicate production etc. varies with the time of
//! day and time of year.
use crate::id::{define_id_type, IDCollection};
use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use serde::de::Error;
use serde::{Deserialize, Serialize};
use serde_string_enum::DeserializeLabeledStringEnum;
use std::fmt::Display;
use std::iter;

define_id_type! {Season}
define_id_type! {TimeOfDay}

/// An ID describing season and time of day
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct TimeSliceID {
    /// The name of each season.
    pub season: Season,
    /// The name of each time slice within a day.
    pub time_of_day: TimeOfDay,
}

impl Display for TimeSliceID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.season, self.time_of_day)
    }
}

impl<'de> Deserialize<'de> for TimeSliceID {
    fn deserialize<D>(deserialiser: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserialiser)?;
        let (season, time_of_day) = s
            .split(".")
            .collect_tuple()
            .ok_or_else(|| D::Error::custom(format!("Invalid input '{}': Should be in form season.time_of_day", s)))?;
        Ok(Self {
            season: season.into(),
            time_of_day: time_of_day.into(),
        })
    }
}

impl Serialize for TimeSliceID {
    fn serialize<S>(&self, serialiser: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serialiser.collect_str(self)
    }
}

/// Represents a time slice read from an input file, which can be all
#[derive(PartialEq, Clone, Debug)]
pub enum TimeSliceSelection {
    /// All year and all day
    Annual,
    /// Only applies to one season
    Season(Season),
    /// Only applies to a single time slice
    Single(TimeSliceID),
}

/// The time granularity for a particular operation
#[derive(PartialEq, Copy, Clone, Debug, DeserializeLabeledStringEnum)]
pub enum TimeSliceLevel {
    /// The whole year
    #[string = "annual"]
    Annual,
    /// Whole seasons
    #[string = "season"]
    Season,
    /// Treat individual time slices separately
    #[string = "daynight"]
    DayNight,
}

/// Information about the time slices in the simulation, including names and fractions
#[derive(PartialEq, Debug)]
pub struct TimeSliceInfo {
    /// Names of seasons
    pub seasons: IndexSet<Season>,
    /// Names of times of day (e.g. "evening")
    pub times_of_day: IndexSet<TimeOfDay>,
    /// The fraction of the year that this combination of season and time of day occupies
    pub fractions: IndexMap<TimeSliceID, f64>,
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
            seasons: iter::once(id.season).collect(),
            times_of_day: iter::once(id.time_of_day).collect(),
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
            .get_id_by_str(season)
            .with_context(|| format!("{} is not a known season", season))?;
        let time_of_day = self
            .times_of_day
            .get_id_by_str(time_of_day)
            .with_context(|| format!("{} is not a known time of day", time_of_day))?;

        Ok(TimeSliceID {
            season: season.clone(),
            time_of_day: time_of_day.clone(),
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
            let season = self
                .seasons
                .get(time_slice)
                .with_context(|| format!("'{time_slice}' is not a valid season"))?
                .clone();
            Ok(TimeSliceSelection::Season(season))
        }
    }

    /// Iterate over all [`TimeSliceID`]s.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter_ids(&self) -> impl Iterator<Item = &TimeSliceID> {
        self.fractions.keys()
    }

    /// Iterate over all time slices.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter(&self) -> impl Iterator<Item = (&TimeSliceID, f64)> {
        self.fractions.iter().map(|(ts, fraction)| (ts, *fraction))
    }

    /// Iterate over the subset of time slices indicated by `selection`.
    ///
    /// The order will be consistent each time this is called, but not every time the program is
    /// run.
    pub fn iter_selection<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
    ) -> Box<dyn Iterator<Item = (&'a TimeSliceID, f64)> + 'a> {
        match selection {
            TimeSliceSelection::Annual => Box::new(self.iter()),
            TimeSliceSelection::Season(season) => {
                Box::new(self.iter().filter(move |(ts, _)| ts.season == *season))
            }
            TimeSliceSelection::Single(ts) => {
                Box::new(iter::once((ts, *self.fractions.get(ts).unwrap())))
            }
        }
    }

    /// Iterate over the different time slice selections for a given time slice level.
    ///
    /// For example, if [`TimeSliceLevel::Season`] is specified, this function will return an
    /// iterator of [`TimeSliceSelection`]s covering each season.
    pub fn iter_selections_for_level(
        &self,
        level: TimeSliceLevel,
    ) -> Box<dyn Iterator<Item = TimeSliceSelection> + '_> {
        match level {
            TimeSliceLevel::Annual => Box::new(iter::once(TimeSliceSelection::Annual)),
            TimeSliceLevel::Season => {
                Box::new(self.seasons.iter().cloned().map(TimeSliceSelection::Season))
            }
            TimeSliceLevel::DayNight => {
                Box::new(self.iter_ids().cloned().map(TimeSliceSelection::Single))
            }
        }
    }

    /// Iterate over a subset of time slices calculating the relative duration of each.
    ///
    /// The relative duration is specified as a fraction of the total time (proportion of year)
    /// covered by `selection`.
    ///
    /// # Arguments
    ///
    /// * `selection` - A subset of time slices
    ///
    /// # Returns
    ///
    /// An iterator of time slices along with the fraction of the total selection.
    pub fn iterate_selection_share<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
    ) -> impl Iterator<Item = (&'a TimeSliceID, f64)> {
        // Store time slices as we have to iterate over selection twice
        let time_slices = self.iter_selection(selection).collect_vec();

        // Total fraction of year covered by selection
        let time_total: f64 = time_slices.iter().map(|(_, fraction)| *fraction).sum();

        // Calculate share
        time_slices
            .into_iter()
            .map(move |(ts, time_fraction)| (ts, time_fraction / time_total))
    }

    /// Share a value between a subset of time slices in proportion to their lengths.
    ///
    /// For instance, you could use this function to compute how demand is distributed between the
    /// different time slices of winter.
    ///
    /// # Arguments
    ///
    /// * `selection` - A subset of time slices
    /// * `value` - The value to be shared between the time slices
    ///
    /// # Returns
    ///
    /// An iterator of time slices along with a fraction of `value`.
    pub fn calculate_share<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
        value: f64,
    ) -> impl Iterator<Item = (&'a TimeSliceID, f64)> {
        self.iterate_selection_share(selection)
            .map(move |(ts, share)| (ts, value * share))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::assert_approx_eq;
    use itertools::assert_equal;

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

        assert_equal(
            ts_info
                .iter_selection(&TimeSliceSelection::Annual)
                .map(|(ts, _)| ts),
            slices.iter(),
        );
        assert_equal(
            ts_info
                .iter_selection(&TimeSliceSelection::Season("winter".into()))
                .map(|(ts, _)| ts),
            iter::once(&slices[0]),
        );
        let ts = ts_info.get_time_slice_id_from_str("summer.night").unwrap();
        assert_equal(
            ts_info
                .iter_selection(&TimeSliceSelection::Single(ts))
                .map(|(ts, _)| ts),
            iter::once(&slices[1]),
        );
    }

    #[test]
    fn test_calculate_share() {
        let slices = [
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "night".into(),
            },
            TimeSliceID {
                season: "summer".into(),
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
            fractions: slices.iter().map(|ts| (ts.clone(), 0.25)).collect(),
        };

        macro_rules! check_share {
            ($selection:expr, $expected:expr) => {
                let expected = $expected;
                let actual: IndexMap<_, _> = IndexMap::from_iter(
                    ts_info
                        .calculate_share(&$selection, 8.0)
                        .map(|(ts, share)| (ts.clone(), share)),
                );
                assert!(actual.len() == expected.len());
                for (k, v) in actual {
                    assert_approx_eq!(f64, v, *expected.get(&k).unwrap());
                }
            };
        }

        // Whole year
        let expected: IndexMap<_, _> =
            IndexMap::from_iter(slices.iter().map(|ts| (ts.clone(), 2.0)));
        check_share!(TimeSliceSelection::Annual, expected);

        // One season
        let selection = TimeSliceSelection::Season("winter".into());
        let expected: IndexMap<_, _> = IndexMap::from_iter(
            ts_info
                .iter_selection(&selection)
                .map(|(ts, _)| (ts.clone(), 4.0)),
        );
        check_share!(selection, expected);

        // Single time slice
        let time_slice = ts_info.get_time_slice_id_from_str("winter.day").unwrap();
        let selection = TimeSliceSelection::Single(time_slice.clone());
        let expected: IndexMap<_, _> = IndexMap::from_iter(iter::once((time_slice, 8.0)));
        check_share!(selection, expected);
    }
}
