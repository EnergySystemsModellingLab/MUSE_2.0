//! The command line interface for the simulation.
use crate::input::load_model;
use crate::log;
use crate::output::{create_output_directory, get_output_dir};
use crate::settings::Settings;
use ::log::info;
use anyhow::{ensure, Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir, DirEntry};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The directory containing the example models.
pub const EXAMPLES_DIR: Dir = include_dir!("examples");

/// The command line interface for the simulation.
#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    /// The available commands.
    #[command(subcommand)]
    pub command: Option<Commands>,
    /// Flag to provide the CLI docs as markdown
    #[arg(long, hide = true)]
    pub markdown_help: bool,
}

/// The available commands.
#[derive(Subcommand)]
pub enum Commands {
    /// Run a simulation model.
    Run {
        /// Path to the model directory.
        model_dir: PathBuf,
        /// Directory for output files
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        /// Whether to write additional information to CSV files
        #[arg(long)]
        debug_model: bool,
    },
    /// Manage example models.
    Example {
        /// The available subcommands for managing example models.
        #[command(subcommand)]
        subcommand: ExampleSubcommands,
    },
}

/// The available subcommands for managing example models.
#[derive(Subcommand)]
pub enum ExampleSubcommands {
    /// List available examples.
    List,
    /// Extract an example model configuration to a new directory.
    Extract {
        /// The name of the example to extract.
        name: String,
        /// The destination folder for the example.
        new_path: Option<PathBuf>,
    },
    /// Run an example.
    Run {
        /// The name of the example to run.
        name: String,
        /// Directory for output files
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        /// Whether to write additional information to CSV files
        #[arg(long)]
        debug_model: bool,
    },
}

/// Handle the `run` command.
pub fn handle_run_command(
    model_path: &Path,
    output_path: Option<&Path>,
    debug_model: bool,
) -> Result<()> {
    // Load program settings
    let mut settings = Settings::load().context("Failed to load settings.")?;

    // This setting can be overridden by command-line argument
    if debug_model {
        settings.debug_model = true;
    }

    // Create output folder
    let output_path = match output_path {
        Some(p) => p.to_owned(),
        None => get_output_dir(model_path)?,
    };
    create_output_directory(&output_path).context("Failed to create output directory.")?;

    // Initialise program logger
    log::init(settings.log_level.as_deref(), &output_path)
        .context("Failed to initialise logging.")?;

    // Load the model to run
    let (model, assets) = load_model(model_path).context("Failed to load model.")?;
    info!("Loaded model from {}", model_path.display());
    info!("Output data will be written to {}", output_path.display());

    // Run the simulation
    crate::simulation::run(model, assets, &output_path, settings.debug_model)?;

    Ok(())
}

/// Handle the `example list` command.
pub fn handle_example_list_command() {
    for entry in EXAMPLES_DIR.dirs() {
        println!("{}", entry.path().display());
    }
}

/// Handle the `example extract` command
pub fn handle_example_extract_command(name: &str, dest: Option<&Path>) -> Result<()> {
    let dest = dest.unwrap_or(Path::new(name));
    extract_example(name, dest)
}

/// Extract the specified example to a new directory
fn extract_example(name: &str, new_path: &Path) -> Result<()> {
    // Find the subdirectory in EXAMPLES_DIR whose name matches `name`.
    let sub_dir = EXAMPLES_DIR.get_dir(name).context("Example not found.")?;

    ensure!(
        !new_path.exists(),
        "Destination directory {} already exists",
        new_path.display()
    );

    // Copy the contents of the subdirectory to the destination
    fs::create_dir(new_path)?;
    for entry in sub_dir.entries() {
        match entry {
            DirEntry::Dir(_) => panic!("Subdirectories in examples not supported"),
            DirEntry::File(f) => {
                let file_name = f.path().file_name().unwrap();
                let file_path = new_path.join(file_name);
                fs::write(&file_path, f.contents())?;
            }
        }
    }

    Ok(())
}

/// Handle the `example run` command.
pub fn handle_example_run_command(
    name: &str,
    output_path: Option<&Path>,
    debug_model: bool,
) -> Result<()> {
    let temp_dir = TempDir::new().context("Failed to create temporary directory.")?;
    let model_path = temp_dir.path().join(name);
    extract_example(name, &model_path)?;
    handle_run_command(&model_path, output_path, debug_model)
}
