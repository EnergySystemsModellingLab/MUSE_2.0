use ::log::info;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use include_dir::{include_dir, Dir};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

pub const EXAMPLES_DIR: Dir = include_dir!("examples");

#[derive(Parser)]
#[command(version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Run a simulation model.")]
    Run {
        #[arg(help = "Path to the model directory")]
        model_dir: PathBuf,
    },
    #[command(about = "Manage example models.")]
    Example {
        #[command(subcommand)]
        subcommand: ExampleSubcommands,
    },
}

#[derive(Subcommand)]
pub enum ExampleSubcommands {
    #[command(about = "List available examples.")]
    List,
}

pub fn handle_run_command(model_dir: &PathBuf) -> Result<()> {
    let settings = Settings::from_path(model_dir)?;
    log::init(settings.log_level.as_deref()).context("Failed to initialize logging.")?;
    let model = Model::from_path(model_dir).context("Failed to load model.")?;
    info!("Model loaded successfully.");
    muse2::run(&model);
    Ok(())
}

pub fn handle_example_list_command() -> Result<()> {
    for entry in EXAMPLES_DIR.dirs() {
        println!("{}", entry.path().display());
    }
    Ok(())
}
