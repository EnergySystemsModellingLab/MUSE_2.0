//! The main entry point for the `muse2` command-line tool.
use ::log::info;
use clap::{Arg, Command};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

/// The main entry point for the `muse2 run` command.
fn main() {
    let cmd = Command::new("muse2")
        .version("2.0")
        .about("MUSE2 Simulation Tool")
        .subcommand(
            Command::new("run").about("Run a model simulation").arg(
                Arg::new("model_dir")
                    .help("Path to the model directory")
                    .required(true)
                    .index(1)
                    .value_parser(clap::value_parser!(PathBuf)),
            ),
        );

    let matches = cmd.get_matches();

    match matches.subcommand() {
        Some(("run", sub_matches)) => {
            let model_dir = sub_matches
                .get_one::<PathBuf>("model_dir")
                .expect("Required argument");

            // Read program settings
            let settings = Settings::from_path(model_dir).unwrap();
            // Set the program to log level
            log::init(settings.log_level.as_deref());
            log_panics::init();

            let model = Model::from_path(model_dir).unwrap();
            info!("Model loaded successfully.");
            // Run simulation
            muse2::run(&model);
        }
        _ => {
            println!("Use 'muse2 run <MODEL_DIR>' to run a simulation");
            println!("Use 'muse2 --help' for more information");
        }
    }
}
