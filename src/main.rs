use clap::Parser;
use human_panic::{metadata, setup_panic};
use muse2::commands;

use commands::{
    handle_example_list_command, handle_example_run_command, handle_run_command, Cli, Commands,
    ExampleSubcommands,
};

fn main() {
    setup_panic!(metadata!().support(format!(
        "Open an issue on Github: {}/issues/new?template=bug_report.md",
        env!("CARGO_PKG_REPOSITORY")
    )));

    let cli = Cli::parse();

    match cli.command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
        Commands::Example { subcommand } => match subcommand {
            ExampleSubcommands::List => handle_example_list_command(),
            ExampleSubcommands::Run { name } => handle_example_run_command(&name),
        },
    }
    .unwrap_or_else(|err| eprintln!("{:?}", err))
}
