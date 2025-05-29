//! Code for working with years.
use crate::input::is_sorted_and_unique;
use anyhow::{ensure, Context, Result};
use itertools::Itertools;

/// Parse a single year from a string and check it is in `valid_years`
fn parse_and_validate_year(s: &str, valid_years: &[u32]) -> Option<u32> {
    let year = s.trim().parse::<u32>().ok()?;
    if valid_years.binary_search(&year).is_ok() {
        Some(year)
    } else {
        None
    }
}

/// Parse a string of years separated by semicolons into a vector of u32 years.
///
/// The string can be either "all" (case-insensitive), a single year, or a semicolon-separated list
/// of years (e.g. "2020;2021;2022" or "2020; 2021; 2022")
///
/// # Arguments
///
/// - `s` - Input string to parse
/// - `valid_years` - The possible years which can be referenced in `s` (must be sorted and unique)
///
/// # Returns
///
/// A [`Vec`] of years or an error.
///
/// # Panics
///
/// If `valid_years` is unsorted or non-unique.
pub fn parse_year_str(s: &str, valid_years: &[u32]) -> Result<Vec<u32>> {
    // We depend on this in `parse_and_validate_year`
    assert!(
        is_sorted_and_unique(valid_years),
        "`valid_years` must be sorted and unique"
    );

    let s = s.trim();
    ensure!(!s.is_empty(), "No years provided");

    if s.eq_ignore_ascii_case("all") {
        return Ok(Vec::from_iter(valid_years.iter().copied()));
    }

    let years: Vec<_> = s
        .split(";")
        .map(|y| {
            parse_and_validate_year(y, valid_years).with_context(|| format!("Invalid year: {y}"))
        })
        .try_collect()?;

    ensure!(
        is_sorted_and_unique(&years),
        "Years must be in order and unique"
    );

    Ok(years)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::assert_error;
    use rstest::rstest;

    #[rstest]
    #[case("2020", &[2020, 2021], &[2020])]
    #[case("all", &[2020, 2021], &[2020,2021])]
    #[case("ALL", &[2020, 2021], &[2020,2021])]
    #[case(" ALL ", &[2020, 2021], &[2020,2021])]
    #[case("2020;2021", &[2020, 2021], &[2020,2021])]
    #[case("  2020;  2021", &[2020, 2021], &[2020,2021])] // whitespace should be stripped
    fn test_parse_year_str_valid(
        #[case] input: &str,
        #[case] milestone_years: &[u32],
        #[case] expected: &[u32],
    ) {
        assert_eq!(parse_year_str(input, milestone_years).unwrap(), expected);
    }

    #[rstest]
    #[case("", &[2020], "No years provided")]
    #[case("2021", &[2020], "Invalid year: 2021")]
    #[case("a;2020", &[2020], "Invalid year: a")]
    #[case("2021;2020", &[2020, 2021],"Years must be in order and unique")] // out of order
    #[case("2021;2020;2021", &[2020, 2021],"Years must be in order and unique")] // duplicate
    fn test_parse_year_str_invalid(
        #[case] input: &str,
        #[case] milestone_years: &[u32],
        #[case] error_msg: &str,
    ) {
        assert_error!(parse_year_str(input, milestone_years), error_msg);
    }
}
