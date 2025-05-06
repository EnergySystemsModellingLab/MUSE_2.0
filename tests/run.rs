//! Integration tests for the `run` command.
use muse2::commands::handle_run_command;
use std::path::PathBuf;

/// Get the path to the example model.
fn get_model_dir() -> PathBuf {
    PathBuf::from("examples/simple")
}

/// An integration test for the `run` command.
#[test]
fn test_handle_run_command() {
    std::env::set_var("MUSE2_LOG_LEVEL", "off");
    handle_run_command(&get_model_dir(), None).unwrap();

    // Second time will fail because the logging is already initialised
    assert_eq!(
        handle_run_command(&get_model_dir(), None)
            .unwrap_err()
            .chain()
            .next()
            .unwrap()
            .to_string(),
        "Failed to initialise logging."
    );
}
