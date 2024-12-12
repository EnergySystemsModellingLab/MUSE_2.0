use ::log::info;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

const EXAMPLES_DIR: Dir = include_dir!("examples");

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Run a simulation model.")]
    Run {
        #[arg(help = "Path to the model directory")]
        model_dir: PathBuf,
    },
    #[command(about = "Run an example model.")]
    Example {
        #[arg(help = "Name of the example model")]
        example_name: Option<String>,
    },
}

fn handle_run_command(model_dir: &PathBuf) -> Result<()> {
    // Read program settings
    let settings = Settings::from_path(model_dir)?;

    // Set up logging
    log::init(settings.log_level.as_deref()).context("Failed to initialize logging.")?;

    // Load and run model
    let model = Model::from_path(model_dir).context("Failed to load model.")?;
    info!("Model loaded successfully.");
    muse2::run(&model);

    Ok(())
}

fn handle_example_command(example_name: Option<String>) -> Result<()> {
    if let Some("list") = example_name.as_deref() {
        for entry in EXAMPLES_DIR.dirs() {
            println!("{}", entry.path().display());
        }
        Ok(())
    } else {
        println!("Please provide an example name or 'list' to list available examples.");
        Ok(())
    }
}
fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
        Commands::Example { example_name } => handle_example_command(example_name),
    }
    .unwrap_or_else(|err| print!("{:?}", err))
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
