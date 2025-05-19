use anyhow::Result;
use clap::{CommandFactory, Parser};
use human_panic::{metadata, setup_panic};
use muse2::commands::{
    handle_example_extract_command, handle_example_list_command, handle_example_run_command,
    handle_run_command, Cli, Commands, ExampleSubcommands,
};

fn main() {
    setup_panic!(metadata!().support(format!(
        "Open an issue on Github: {}/issues/new?template=bug_report.md",
        env!("CARGO_PKG_REPOSITORY")
    )));

    let cli = Cli::parse();

    // Invoked as: `$ muse2 --markdown-help`
    if cli.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return;
    }

    execute_cli_command(cli.command).unwrap_or_else(|err| eprintln!("Error: {:?}", err));
}

fn execute_cli_command(command: Option<Commands>) -> Result<()> {
    let Some(command) = command else {
        // Output program help in markdown format
        let help_str = Cli::command().render_long_help().to_string();
        println!("{}", help_str);
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
