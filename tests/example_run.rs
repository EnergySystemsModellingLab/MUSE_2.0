//! Integration tests for the `example run` command.
//!
//! If you add a new example, you must add a test case below.
use float_cmp::approx_eq;
use itertools::Itertools;
use muse2::commands::handle_example_run_command;
use regex::Regex;
use rstest::rstest;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// An integration test for the `example run` command.
#[rstest]
#[case("simple")]
fn test_handle_example_run_command(#[case] example_name: &str) {
    std::env::set_var("MUSE2_LOG_LEVEL", "off");

    let is_float = Regex::new(r"^-?[0-9]+\.[0-9]+$").unwrap();
    let tempdir = tempdir().unwrap();
    let output_dir = tempdir.path();
    handle_example_run_command(example_name, Some(output_dir)).unwrap();

    let file_names = get_csv_file_names(output_dir);
    let test_data_dir = PathBuf::from(format!("tests/data/{example_name}"));
    let expected_file_names = get_csv_file_names(&test_data_dir);

    // Check that output files haven't been added/removed
    assert!(file_names == expected_file_names);

    let mut errors = Vec::new();
    for file_name in file_names {
        let lines1 = read_lines(&output_dir.join(&file_name));
        let lines2 = read_lines(&test_data_dir.join(&file_name));

        // Check for different number of lines
        if lines1.len() != lines2.len() {
            errors.push(format!(
                "{}: Different number of lines: {} vs {}",
                file_name,
                lines1.len(),
                lines2.len()
            ));
        }

        // Compare each line
        for (num, (line1, line2)) in lines1.into_iter().zip(lines2).enumerate() {
            let fields1 = line1.split(",").collect_vec();
            let fields2 = line2.split(",").collect_vec();
            if fields1.len() != fields2.len() {
                errors.push(format!(
                    "{}: line {}: Different number of fields: {} vs {}",
                    file_name,
                    num,
                    fields1.len(),
                    fields2.len()
                ));
            }

            // Lambda for approximately comparing floating-point fields
            let compare_float_fields = |field1: &str, field2: &str| {
                // Use a regex to filter out non-floating point values, as well as things that are
                // technically valid but have strange properties (e.g. inf, NaN)
                if !is_float.is_match(field1) || !is_float.is_match(field2) {
                    return None;
                }

                let float1: f64 = field1.parse().unwrap();
                let float2: f64 = field2.parse().unwrap();
                Some(approx_eq!(f64, float1, float2))
            };

            // Compare each field in turn
            let mut line_matches = true;
            for (field1, field2) in fields1.into_iter().zip(fields2) {
                // First try to compare fields as floating-point values, falling back on string
                // comparison
                let result =
                    compare_float_fields(field1, field2).unwrap_or_else(|| field1 == field2);
                if !result {
                    line_matches = false;
                }
            }

            if !line_matches {
                errors.push(format!(
                    "{}: line {}:\n    + \"{}\"\n    - \"{}\"",
                    file_name, num, line1, line2
                ))
            }
        }
    }

    assert!(
        errors.is_empty(),
        "The following errors occurred:\n  * {}",
        errors.join("\n  * ")
    );
}

/// Get the names of CSV files expected to appear in the given folder
fn get_csv_file_names(dir_path: &Path) -> Vec<String> {
    let entries = read_dir(dir_path).unwrap();
    let mut file_names = Vec::new();
    for entry in entries {
        let file_name = entry.unwrap().file_name();
        let file_name = file_name.to_str().unwrap();
        if file_name.ends_with(".csv") {
            file_names.push(file_name.to_string());
        }
    }

    file_names.sort();
    file_names
}

// Read all lines from a file into a `Vec`
fn read_lines(path: &Path) -> Vec<String> {
    let file1 = File::open(path).unwrap();
    BufReader::new(file1)
        .lines()
        .map_while(Result::ok)
        .collect()
}
