//! The main entry point for the `muse2` command-line tool.
use ::log::info;
use clap::{Parser, Subcommand};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = clap::crate_name!())]
#[command(version = clap::crate_version!())]
#[command(about = clap::crate_description!())]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(help = "Path to the model directory")]
        model_dir: PathBuf,
    },
}

fn handle_run_command(model_dir: &PathBuf) {
    // Read program settings
    let settings = Settings::from_path(model_dir).unwrap();

    // Set up logging
    log::init(settings.log_level.as_deref());
    log_panics::init();

    // Load and run model
    let model = Model::from_path(model_dir).unwrap();
    info!("Model loaded successfully.");
    muse2::run(&model);
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
    }
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
        handle_run_command(&get_model_dir());
    }
}
