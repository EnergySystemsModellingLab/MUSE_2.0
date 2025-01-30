//! The command line interface for the simulation.
use crate::settings::Settings;
use crate::{input::load_model, log};
use ::log::info;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir, DirEntry};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

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
    /// Run an example.
    Run {
        /// The name of the example to run.
        name: String,
    },
}

/// Handle the `run` command.
pub fn handle_run_command(model_dir: &PathBuf) -> Result<()> {
    let settings = Settings::from_path(model_dir)?;
    log::init(settings.log_level.as_deref()).context("Failed to initialize logging.")?;
    let (model, assets) = load_model(model_dir).context("Failed to load model.")?;
    info!("Model loaded successfully.");
    crate::simulation::run(model, assets);
    Ok(())
}

/// Handle the `example run` command.
pub fn handle_example_run_command(name: &str) -> Result<()> {
    // Find the subdirectory in EXAMPLES_DIR whose name matches `name`.
    let sub_dir = EXAMPLES_DIR.get_dir(name).context("Directory not found.")?;

    // Creates temporary directory
    let temp_dir = TempDir::new().context("Failed to create temporary directory.")?;
    // Copies the contents of the subdirectory to the temporary directory
    for entry in sub_dir.entries() {
        match entry {
            DirEntry::Dir(d) => {
                for sub_entry in d.entries() {
                    match sub_entry {
                        DirEntry::File(f) => {
                            let file_name = f.path().file_name().unwrap();
                            let file_path = temp_dir.path().join(file_name);
                            fs::write(&file_path, f.contents())?;
                        }
                        DirEntry::Dir(_) => {
                            return Err(anyhow::anyhow!("Nested directories are not supported."));
                        }
                    }
                }
            }
            DirEntry::File(f) => {
                let file_name = f.path().file_name().unwrap();
                let file_path = temp_dir.path().join(file_name);
                fs::write(&file_path, f.contents())?;
            }
        }
    }

    let settings = Settings::from_path(temp_dir.path())?;
    log::init(settings.log_level.as_deref()).context("Failed to initialize logging.")?;
    let (model, assets) = load_model(temp_dir.path()).context("Failed to load model.")?;
    info!("Model loaded successfully from directory: {}", name);
    crate::simulation::run(model, assets);
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
