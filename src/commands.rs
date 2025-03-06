//! The command line interface for the simulation.
use crate::output::create_output_directory;
use crate::settings::Settings;
use crate::{input::load_model, log};
use ::log::{error, info};
use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir, DirEntry};
use std::fs;
use std::path::{Path, PathBuf};
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
    /// Copy an example model configuration.
    Copy {
        /// The name of the example to copy.
        name: String,
        /// The destination folder for the example.
        dest: Option<PathBuf>,
    },
    /// Run an example.
    Run {
        /// The name of the example to run.
        name: String,
    },
}

/// Handle the `run` command.
pub fn handle_run_command(model_path: &Path) -> Result<()> {
    // Load program settings
    let settings = Settings::from_path(model_path).context("Failed to load settings.")?;

    // Create output folder
    let output_path =
        create_output_directory(model_path).context("Failed to create output directory.")?;

    // Initialise program logger
    log::init(settings.log_level.as_deref(), &output_path)
        .context("Failed to initialise logging.")?;

    let load_and_run_model = || {
        // Load the model to run
        let (model, assets) = load_model(model_path).context("Failed to load model.")?;
        info!("Loaded model from {}", model_path.display());
        info!("Output data will be written to {}", output_path.display());

        // Run the simulation
        crate::simulation::run(model, assets, &output_path)
    };

    // Once the logger is initialised, we can write fatal errors to log
    if let Err(err) = load_and_run_model() {
        error!("{err:?}");
    }

    Ok(())
}

/// Handle the `example list` command.
pub fn handle_example_list_command() {
    for entry in EXAMPLES_DIR.dirs() {
        println!("{}", entry.path().display());
    }
}

/// Handle the `example copy` command
pub fn handle_example_copy_command(name: &str, dest: Option<&Path>) -> Result<()> {
    let dest = dest.unwrap_or(Path::new(name));
    copy_example(name, dest)
}

/// Copy the specified example to a new directory
fn copy_example(name: &str, dest: &Path) -> Result<()> {
    // Find the subdirectory in EXAMPLES_DIR whose name matches `name`.
    let sub_dir = EXAMPLES_DIR.get_dir(name).context("Example not found.")?;

    ensure!(
        !dest.exists(),
        "Destination directory {} already exists, not overwriting",
        dest.display()
    );

    // Copy the contents of the subdirectory to the destination
    fs::create_dir(dest)?;
    for entry in sub_dir.entries() {
        match entry {
            DirEntry::Dir(_) => panic!("Subdirectories in examples not supported"),
            DirEntry::File(f) => {
                let file_name = f.path().file_name().unwrap();
                let file_path = dest.join(file_name);
                fs::write(&file_path, f.contents())?;
            }
        }
    }

    Ok(())
}

/// Handle the `example run` command.
pub fn handle_example_run_command(name: &str) -> Result<()> {
    let temp_dir = TempDir::new().context("Failed to create temporary directory.")?;
    let model_path = temp_dir.path().join(name);
    copy_example(name, &model_path)?;
    handle_run_command(&model_path)
}
