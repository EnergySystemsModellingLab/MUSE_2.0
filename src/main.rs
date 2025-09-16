use human_panic::{metadata, setup_panic};
use log::error;
use muse2::cli::run_cli;
use muse2::log::is_logger_initialised;

fn main() {
    setup_panic!(metadata!().support(format!(
        "Open an issue on Github: {}/issues/new?template=bug_report.md",
        env!("CARGO_PKG_REPOSITORY")
    )));

    if let Err(err) = run_cli() {
        if is_logger_initialised() {
            error!("{err:?}");
        } else {
            eprintln!("Error: {err:?}");
        }

        // Terminate program, signalling an error
        std::process::exit(1);
    }
}
