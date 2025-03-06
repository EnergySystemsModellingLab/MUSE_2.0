use anyhow::Result;
use clap::Parser;
use commands::{
    handle_example_list_command, handle_example_run_command, handle_run_command, Cli, Commands,
    ExampleSubcommands,
};
use human_panic::{metadata, setup_panic};
use muse2::commands;

fn main() {
    setup_panic!(metadata!().support(format!(
        "Open an issue on Github: {}/issues/new?template=bug_report.md",
        env!("CARGO_PKG_REPOSITORY")
    )));

    let cli = Cli::parse();
    execute_cli_command(cli.command).unwrap_or_else(|err| eprintln!("Error: {:?}", err));
}

fn execute_cli_command(command: Commands) -> Result<()> {
    match command {
        Commands::Run { model_dir } => handle_run_command(&model_dir),
        Commands::Example { subcommand } => match subcommand {
            ExampleSubcommands::List => handle_example_list_command(),
            ExampleSubcommands::Run { name } => handle_example_run_command(&name),
        },
    }
}
