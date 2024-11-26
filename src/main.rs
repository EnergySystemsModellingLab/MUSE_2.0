use ::log::info;
use clap::{Arg, Command};
use muse2::log;
use muse2::model::Model;
use muse2::settings::Settings;
use std::path::PathBuf;

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

            // Your existing simulation logic
            let settings = Settings::from_path(model_dir).unwrap();
            log::init(settings.log_level.as_deref());
            log_panics::init();

            let model = Model::from_path(model_dir).unwrap();
            info!("Model loaded successfully.");
            muse2::run(&model);
        }
        _ => {
            // No need to handle help explicitly - clap does it automatically
            println!("Use 'muse2 run <MODEL_DIR>' to run a simulation");
            println!("Use 'muse2 --help' for more information");
        }
    }
}
