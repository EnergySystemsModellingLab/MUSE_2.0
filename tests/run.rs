//! Integration tests for the `run` command.
use muse2::commands::handle_run_command;
use std::path::{Path, PathBuf};

/// Get the path to the example model.
fn get_model_dir() -> PathBuf {
    Path::new(file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join("simple")
}

/// An integration test for the `run` command.
#[test]
fn test_handle_run_command() {
    handle_run_command(&get_model_dir()).unwrap();

    // Second time will fail because the logging is already initialised
    assert_eq!(
        handle_run_command(&get_model_dir())
            .unwrap_err()
            .chain()
            .next()
            .unwrap()
            .to_string(),
        "Failed to initialize logging."
    );
}
