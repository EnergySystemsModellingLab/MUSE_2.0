//! The command line interface for the simulation.
use crate::output::create_output_directory;
use crate::settings::Settings;
use crate::{input::load_model, log};
use ::log::info;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use std::path::PathBuf;

/// The directory containing the example models.
pub const EXAMPLES_DIR: Dir = include_dir!("examples");

#[derive(Parser)]
#[command(version, about)]
/// The command line interface for the simulation.
pub struct Cli {
    #[command(subcommand)]
    /// The available commands.
    pub command: Commands,
}

#[derive(Subcommand)]
/// The available commands.
pub enum Commands {
    /// Run a simulation model.
    Run {
        #[arg(help = "Path to the model directory")]
        /// Path to the model directory.
        model_dir: PathBuf,
    },
    /// Manage example models.
    Example {
        #[command(subcommand)]
        /// The available subcommands for managing example models.
        subcommand: ExampleSubcommands,
    },
}

#[derive(Subcommand)]
/// The available subcommands for managing example models.
pub enum ExampleSubcommands {
    /// List available examples.
    List,
}

/// Handle the `run` command.
pub fn handle_run_command(model_dir: &PathBuf) -> Result<()> {
    let settings = Settings::from_path(model_dir).context("Failed to load settings.")?;
    let output_path =
        create_output_directory(model_dir).context("Failed to create output directory.")?;
    log::init(settings.log_level.as_deref(), &output_path)
        .context("Failed to initialize logging.")?;
    info!("Output directory created: {}", output_path.display());
    let (model, assets) = load_model(model_dir).context("Failed to load model.")?;
    info!("Model loaded successfully.");
    crate::simulation::run(&model, &assets);
    Ok(())
}

/// Handle the `example list` command.
pub fn handle_example_list_command() -> Result<()> {
    for entry in EXAMPLES_DIR.dirs() {
        println!("{}", entry.path().display());
    }
    Ok(())
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
