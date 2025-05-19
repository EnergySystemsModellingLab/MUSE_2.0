//! Integration tests for the `run` command.
use muse2::commands::handle_run_command;
use std::path::PathBuf;
use tempfile::tempdir;

/// Get the path to the example model.
fn get_model_dir() -> PathBuf {
    PathBuf::from("examples/simple")
}

/// An integration test for the `run` command.
#[test]
fn test_handle_run_command() {
    std::env::set_var("MUSE2_LOG_LEVEL", "off");

    {
        // Save results to non-existent directory to check that directory creation works
        let tempdir = tempdir().unwrap();
        let output_dir = tempdir.path().join("results");
        handle_run_command(&get_model_dir(), Some(&output_dir), false).unwrap();
    }

    // Second time will fail because the logging is already initialised
    assert_eq!(
        handle_run_command(&get_model_dir(), Some(tempdir().unwrap().path()), false)
            .unwrap_err()
            .chain()
            .next()
            .unwrap()
            .to_string(),
        "Failed to initialise logging."
    );
}
