//! Integration tests for the `example run` command.
//!
//! If you add a new example, you must add a test case below.
use muse2::commands::handle_example_run_command;
use rstest::rstest;
use tempfile::tempdir;

/// An integration test for the `example run` command.
#[rstest]
#[case("simple")]
fn test_handle_example_run_command(#[case] example_name: &str) {
    std::env::set_var("MUSE2_LOG_LEVEL", "off");
    handle_example_run_command(example_name, Some(tempdir().unwrap().path())).unwrap();
}
