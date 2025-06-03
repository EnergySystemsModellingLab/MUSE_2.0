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
        let (season, time_of_day) = s.split(".").collect_tuple().ok_or_else(|| {
            D::Error::custom(format!(
                "Invalid input '{}': Should be in form season.time_of_day",
                s
            ))
        })?;
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

impl TimeSliceSelection {
    /// The [`TimeSliceLevel`] to which this [`TimeSliceSelection`] corresponds
    pub fn level(&self) -> TimeSliceLevel {
        match self {
            TimeSliceSelection::Annual => TimeSliceLevel::Annual,
            TimeSliceSelection::Season(_) => TimeSliceLevel::Season,
            TimeSliceSelection::Single(_) => TimeSliceLevel::DayNight,
        }
    }
}

impl From<TimeSliceID> for TimeSliceSelection {
    fn from(value: TimeSliceID) -> Self {
        Self::Single(value)
    }
}

impl From<Season> for TimeSliceSelection {
    fn from(value: Season) -> Self {
        Self::Season(value)
    }
}

/// The time granularity for a particular operation
#[derive(PartialEq, PartialOrd, Copy, Clone, Debug, DeserializeLabeledStringEnum)]
pub enum TimeSliceLevel {
    /// Treat individual time slices separately
    #[string = "daynight"]
    DayNight,
    /// Whole seasons
    #[string = "season"]
    Season,
    /// The whole year
    #[string = "annual"]
    Annual,
}

/// Information about the time slices in the simulation, including names and fractions
#[derive(PartialEq, Debug)]
pub struct TimeSliceInfo {
    /// Names of times of day (e.g. "evening")
    pub times_of_day: IndexSet<TimeOfDay>,
    /// Names and fraction of year occupied by each season
    pub seasons: IndexMap<Season, f64>,
    /// The fraction of the year that this combination of season and time of day occupies
    pub time_slices: IndexMap<TimeSliceID, f64>,
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
            seasons: iter::once((id.season, 1.0)).collect(),
            times_of_day: iter::once(id.time_of_day).collect(),
            time_slices: fractions,
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
            .get_id(season)
            .with_context(|| format!("{} is not a known season", season))?;
        let time_of_day = self
            .times_of_day
            .get_id(time_of_day)
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
                .get_id(time_slice)
                .with_context(|| format!("'{time_slice}' is not a valid season"))?
                .clone();
            Ok(TimeSliceSelection::Season(season))
        }
    }

    /// Iterate over all [`TimeSliceID`]s
    pub fn iter_ids(&self) -> impl Iterator<Item = &TimeSliceID> {
        self.time_slices.keys()
    }

    /// Iterate over all time slices
    pub fn iter(&self) -> impl Iterator<Item = (&TimeSliceID, f64)> {
        self.time_slices
            .iter()
            .map(|(ts, fraction)| (ts, *fraction))
    }

    /// Iterate over the subset of time slices indicated by `selection`
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
                Box::new(iter::once((ts, *self.time_slices.get(ts).unwrap())))
            }
        }
    }

    /// Iterate over the given [`TimeSliceSelection`] at the specified level.
    ///
    /// For example, this allows you to iterate over a [`TimeSliceSelection::Season`] at the level
    /// of either seasons (in which case, the iterator will just contain the season) or time slices
    /// (in which case it will contain all time slices for that season).
    ///
    /// Note that you cannot iterate over a [`TimeSliceSelection`] with coarser temporal granularity
    /// than the [`TimeSliceSelection`] itself (for example, you cannot iterate over a
    /// [`TimeSliceSelection::Season`] at the [`TimeSliceLevel::Annual`] level). In this case, the
    /// function will return `None`.
    pub fn iter_selection_at_level<'a>(
        &'a self,
        selection: &'a TimeSliceSelection,
        level: TimeSliceLevel,
    ) -> Option<Box<dyn Iterator<Item = (TimeSliceSelection, f64)> + 'a>> {
        if level > selection.level() {
            return None;
        }

        let iter: Box<dyn Iterator<Item = _>> = match selection {
            TimeSliceSelection::Annual => match level {
                TimeSliceLevel::Annual => Box::new(iter::once((TimeSliceSelection::Annual, 1.0))),
                TimeSliceLevel::Season => Box::new(
                    self.seasons
                        .iter()
                        .map(|(season, fraction)| (season.clone().into(), *fraction)),
                ),
                TimeSliceLevel::DayNight => Box::new(
                    self.time_slices
                        .iter()
                        .map(|(ts, fraction)| (ts.clone().into(), *fraction)),
                ),
            },
            TimeSliceSelection::Season(season) => match level {
                TimeSliceLevel::Season => Box::new(iter::once((
                    selection.clone(),
                    *self.seasons.get(season).unwrap(),
                ))),
                TimeSliceLevel::DayNight => Box::new(
                    self.time_slices
                        .iter()
                        .filter(move |(ts, _)| &ts.season == season)
                        .map(|(ts, fraction)| (ts.clone().into(), *fraction)),
                ),
                _ => unreachable!(),
            },
            TimeSliceSelection::Single(time_slice) => Box::new(iter::once((
                time_slice.clone().into(),
                *self.time_slices.get(time_slice).unwrap(),
            ))),
        };

        Some(iter)
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
                Box::new(self.seasons.keys().cloned().map(TimeSliceSelection::Season))
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
    pub fn iter_selection_share<'a>(
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
        self.iter_selection_share(selection)
            .map(move |(ts, share)| (ts, value * share))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::assert_approx_eq;
    use itertools::assert_equal;
    use rstest::{fixture, rstest};

    #[fixture]
    fn time_slices1() -> [TimeSliceID; 2] {
        [
            TimeSliceID {
                season: "winter".into(),
                time_of_day: "day".into(),
            },
            TimeSliceID {
                season: "summer".into(),
                time_of_day: "night".into(),
            },
        ]
    }

    #[fixture]
    fn time_slice_info1(time_slices1: [TimeSliceID; 2]) -> TimeSliceInfo {
        TimeSliceInfo {
            seasons: [("winter".into(), 0.5), ("summer".into(), 0.5)]
                .into_iter()
                .collect(),
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            time_slices: time_slices1.map(|ts| (ts, 0.5)).into_iter().collect(),
        }
    }

    #[rstest]
    fn test_iter_selection_annual(time_slice_info1: TimeSliceInfo, time_slices1: [TimeSliceID; 2]) {
        assert_equal(
            time_slice_info1
                .iter_selection(&TimeSliceSelection::Annual)
                .map(|(ts, _)| ts),
            time_slices1.iter(),
        );
    }

    #[rstest]
    fn test_iter_selection_season(time_slice_info1: TimeSliceInfo, time_slices1: [TimeSliceID; 2]) {
        assert_equal(
            time_slice_info1
                .iter_selection(&TimeSliceSelection::Season("winter".into()))
                .map(|(ts, _)| ts),
            iter::once(&time_slices1[0]),
        );
    }

    #[rstest]
    fn test_iter_selection_single(time_slice_info1: TimeSliceInfo, time_slices1: [TimeSliceID; 2]) {
        let ts = time_slice_info1
            .get_time_slice_id_from_str("summer.night")
            .unwrap();
        assert_equal(
            time_slice_info1
                .iter_selection(&TimeSliceSelection::Single(ts))
                .map(|(ts, _)| ts),
            iter::once(&time_slices1[1]),
        );
    }

    #[fixture]
    fn time_slices2() -> [TimeSliceID; 4] {
        [
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
        ]
    }

    #[fixture]
    fn time_slice_info2(time_slices2: [TimeSliceID; 4]) -> TimeSliceInfo {
        TimeSliceInfo {
            times_of_day: ["day".into(), "night".into()].into_iter().collect(),
            seasons: [("winter".into(), 0.5), ("summer".into(), 0.5)]
                .into_iter()
                .collect(),
            time_slices: time_slices2.iter().map(|ts| (ts.clone(), 0.25)).collect(),
        }
    }

    macro_rules! check_share {
        ($selection:expr, $expected:expr) => {
            let expected = $expected;
            let actual: IndexMap<_, _> = IndexMap::from_iter(
                time_slice_info2(time_slices2())
                    .calculate_share(&$selection, 8.0)
                    .map(|(ts, share)| (ts.clone(), share)),
            );
            assert!(actual.len() == expected.len());
            for (k, v) in actual {
                assert_approx_eq!(f64, v, *expected.get(&k).unwrap());
            }
        };
    }

    #[rstest]
    fn test_calculate_share_annual(time_slices2: [TimeSliceID; 4]) {
        // Whole year
        let expected: IndexMap<_, _> =
            IndexMap::from_iter(time_slices2.iter().map(|ts| (ts.clone(), 2.0)));
        check_share!(TimeSliceSelection::Annual, expected);
    }

    #[rstest]
    fn test_calculate_share_season(time_slice_info2: TimeSliceInfo) {
        // One season
        let selection = TimeSliceSelection::Season("winter".into());
        let expected: IndexMap<_, _> = IndexMap::from_iter(
            time_slice_info2
                .iter_selection(&selection)
                .map(|(ts, _)| (ts.clone(), 4.0)),
        );
        check_share!(selection, expected);
    }

    #[rstest]
    fn test_calculate_share_single(time_slice_info2: TimeSliceInfo) {
        // Single time slice
        let time_slice = time_slice_info2
            .get_time_slice_id_from_str("winter.day")
            .unwrap();
        let selection = TimeSliceSelection::Single(time_slice.clone());
        let expected: IndexMap<_, _> = IndexMap::from_iter(iter::once((time_slice, 8.0)));
        check_share!(selection, expected);
    }
}
