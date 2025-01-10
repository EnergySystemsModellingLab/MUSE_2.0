use clap::Parser;

mod commands;

use commands::{
    handle_example_list_command, handle_run_command, Cli, Commands, ExampleSubcommands,
};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
        Commands::Example { subcommand } => match subcommand {
            ExampleSubcommands::List => handle_example_list_command(),
        },
    }
    .unwrap_or_else(|err| eprintln!("{:?}", err))
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
