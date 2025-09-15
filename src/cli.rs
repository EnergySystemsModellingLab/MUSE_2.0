//! The command line interface for the simulation.
use crate::input::load_model;
use crate::log;
use crate::output::{create_output_directory, get_output_dir};
use crate::settings::Settings;
use ::log::info;
use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, Parser, Subcommand};
use include_dir::{Dir, DirEntry, include_dir};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The directory containing the example models.
const EXAMPLES_DIR: Dir = include_dir!("examples");

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

/// The available subcommands for managing example models.
#[derive(Subcommand)]
enum ExampleSubcommands {
    /// List available examples.
    List,
    /// Provide information about the specified example.
    Info {
        /// The name of the example.
        name: String,
    },
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

/// Parse CLI arguments and start MUSE2
pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    // Invoked as: `$ muse2 --markdown-help`
    if cli.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return Ok(());
    }

    execute_cli_command(cli.command)
}

fn execute_cli_command(command: Option<Commands>) -> Result<()> {
    let Some(command) = command else {
        // Output program help in markdown format
        let help_str = Cli::command().render_long_help().to_string();
        println!("{help_str}");
        return Ok(());
    };

    match command {
        Commands::Run {
            model_dir,
            output_dir,
            debug_model,
        } => handle_run_command(&model_dir, output_dir.as_deref(), debug_model)?,
        Commands::Example { subcommand } => match subcommand {
            ExampleSubcommands::List => handle_example_list_command(),
            ExampleSubcommands::Info { name } => handle_example_info_command(&name)?,
            ExampleSubcommands::Extract {
                name,
                new_path: dest,
            } => handle_example_extract_command(&name, dest.as_deref())?,
            ExampleSubcommands::Run {
                name,
                output_dir,
                debug_model,
            } => handle_example_run_command(&name, output_dir.as_deref(), debug_model)?,
        },
    }

    Ok(())
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
fn handle_example_list_command() {
    for entry in EXAMPLES_DIR.dirs() {
        println!("{}", entry.path().display());
    }
}

/// Handle the `example info` command.
fn handle_example_info_command(name: &str) -> Result<()> {
    let path: PathBuf = [name, "README.txt"].iter().collect();
    let readme = EXAMPLES_DIR
        .get_file(path)
        .context("Example not found.")?
        .contents_utf8()
        .expect("README.txt is not UTF-8 encoded");

    println!("{}", readme);

    Ok(())
}

/// Handle the `example extract` command
fn handle_example_extract_command(name: &str, dest: Option<&Path>) -> Result<()> {
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
