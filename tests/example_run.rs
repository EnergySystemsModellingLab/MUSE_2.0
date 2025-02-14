/// Integration tests for the `example run` command.
use muse2::commands::handle_example_run_command;

/// An integration test for the `example run` command.
#[test]
fn test_handle_example_run_command() {
    std::env::set_var("MUSE2_LOG_LEVEL", "off");
    handle_example_run_command("simple").unwrap();
}
