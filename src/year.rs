//! Code for working with years.
use anyhow::{ensure, Result};

/// Parse a string of years separated by semicolons into a vector of u32 years.
/// The string can be either "all" (case-insensitive), a single year, or a semicolon-separated list
/// of years (e.g. "2020;2021;2022" or "2020; 2021; 2022")
pub fn parse_year_str(s: &str, milestone_years: &[u32]) -> Result<Vec<u32>> {
    if s.trim().eq_ignore_ascii_case("all") {
        return Ok(Vec::from_iter(milestone_years.iter().copied()));
    }

    s.split(";")
        .map(|y| {
            let year = y.trim().parse::<u32>()?;
            ensure!(
                milestone_years.binary_search(&year).is_ok(),
                "Invalid year {year}"
            );
            Ok(year)
        })
        .collect()
}
