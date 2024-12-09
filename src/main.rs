//! The main entry point for the `muse2` command-line tool.
use ::log::info;
use clap::{Arg, Command};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

fn build_run_command() -> Command {
    Command::new("run")
        .about("Use 'muse2 run <MODEL_DIR>' to run a simulation")
        .arg(
            Arg::new("MODEL_DIR")
                .help("Path to the model directory")
                .required(true)
                .index(1)
                .value_parser(clap::value_parser!(PathBuf)),
        )
}

fn handle_run_command(sub_matches: &clap::ArgMatches) {
    let model_dir = sub_matches
        .get_one::<PathBuf>("MODEL_DIR")
        .expect("Required argument");

    // Read program settings
    let settings = Settings::from_path(model_dir).unwrap();

    // Set up logging
    log::init(settings.log_level.as_deref()).expect("Failed to initialize logging");

    // Load and run model
    let model = Model::from_path(model_dir).unwrap();
    info!("Model loaded successfully.");
    muse2::run(&model);
}

fn main() {
    let cmd = Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .arg_required_else_help(true)
        .subcommand(build_run_command());

    let matches = cmd.get_matches();
    match matches.subcommand() {
        Some(("run", sub_matches)) => handle_run_command(sub_matches),
        _ => {
            std::process::exit(1);
        }
    }
}
