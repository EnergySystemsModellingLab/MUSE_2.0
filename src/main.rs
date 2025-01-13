use clap::Parser;
use muse2::commands;

use commands::{
    handle_example_list_command, handle_run_command, Cli, Commands, ExampleSubcommands,
};

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
        Commands::Example { subcommand } => match subcommand {
            ExampleSubcommands::List => handle_example_list_command(),
        },
    }
    .unwrap_or_else(|err| eprintln!("{:?}", err))
}
