//! The command line interface for the simulation.
use crate::input::load_model;
use crate::log;
use crate::output::{create_output_directory, get_output_dir};
use crate::settings::Settings;
use ::log::info;
use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::{Path, PathBuf};

pub mod example;
use example::ExampleSubcommands;

/// The command line interface for the simulation.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The available commands.
    #[command(subcommand)]
    command: Option<Commands>,
    /// Flag to provide the CLI docs as markdown
    #[arg(long, hide = true)]
    markdown_help: bool,
}

/// The available commands.
#[derive(Subcommand)]
enum Commands {
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

impl Commands {
    /// Execute the supplied CLI command
    fn execute(self) -> Result<()> {
        match self {
            Self::Run {
                model_dir,
                output_dir,
                debug_model,
            } => handle_run_command(&model_dir, output_dir.as_deref(), debug_model, None),
            Self::Example { subcommand } => subcommand.execute(),
        }
    }
}

/// Parse CLI arguments and start MUSE2
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    // Invoked as: `$ muse2 --markdown-help`
    if cli.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return Ok(());
    }

    let Some(command) = cli.command else {
        // Output program help in markdown format
        let help_str = Cli::command().render_long_help().to_string();
        println!("{help_str}");
        return Ok(());
    };

    command.execute()
}

/// Handle the `run` command.
pub fn handle_run_command(
    model_path: &Path,
    output_path: Option<&Path>,
    debug_model: bool,
    settings: Option<Settings>,
) -> Result<()> {
    // Load program settings, if not provided
    let mut settings = if let Some(settings) = settings {
        settings
    } else {
        Settings::load().context("Failed to load settings.")?
    };

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
