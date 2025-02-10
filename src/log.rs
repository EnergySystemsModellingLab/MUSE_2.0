//! The `log` module provides initialisation and configuration of the application's logging system.
//!
//! This module sets up logging with various levels (error, warn, info, debug, trace) and optional
//! colourisation based on terminal support. It also allows configuration of the log level through
//! environment variables.
use anyhow::{bail, Result};
use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::{log_file, Dispatch, FormatCallback};
use log::{LevelFilter, Record};
use std::env;
use std::fmt::{Arguments, Display};
use std::io::IsTerminal;
use std::path::Path;

/// The default log level for the program.
const DEFAULT_LOG_LEVEL: &str = "info";

/// The file name for the log file containing messages about the ordinary operation of MUSE 2.0
const LOG_INFO_FILE_NAME: &str = "muse2_info.log";

/// The file name for the log file containing warnings and error messages
const LOG_ERROR_FILE_NAME: &str = "muse2_error.log";

/// Initialise the program logger using the `fern` logging library with colourised output.
///
/// The user can specify their preferred logging level via the `settings.toml` file (defaulting to
/// `info` if not present) or with the `MUSE2_LOG_LEVEL` environment variable. If both are provided,
/// the environment variable takes precedence.
///
/// Possible log level options are:
///
/// * `error`
/// * `warn`
/// * `info`
/// * `debug`
/// * `trace`
///
/// # Arguments
///
/// * `log_level_from_settings`: The log level specified in `settings.toml`
/// * `output_path`: The output path for the simulation
pub fn init(log_level_from_settings: Option<&str>, output_path: &Path) -> Result<()> {
    // Retrieve the log level from the environment variable or settings, or use the default
    let log_level = env::var("MUSE2_LOG_LEVEL").unwrap_or_else(|_| {
        log_level_from_settings
            .unwrap_or(DEFAULT_LOG_LEVEL)
            .to_string()
    });

    // Convert the log level string to a log::LevelFilter
    let log_level = match log_level.to_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        unknown => bail!("Unknown log level: {}", unknown),
    };

    // Set up colours for log levels
    let colours = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Magenta);

    // Automatically apply colours only if the output is a terminal
    let use_colour_stdout = std::io::stdout().is_terminal();
    let use_colour_stderr = std::io::stderr().is_terminal();

    // Create log files
    let info_log_file = log_file(output_path.join(LOG_INFO_FILE_NAME))?;
    let err_log_file = log_file(output_path.join(LOG_ERROR_FILE_NAME))?;

    // Configure the logger
    let dispatch = Dispatch::new()
        .chain(
            // Write non-error messages to stdout
            Dispatch::new()
                .filter(|metadata| metadata.level() > LevelFilter::Warn)
                .format(move |out, message, record| {
                    write_log_colour(out, message, record, use_colour_stdout, &colours);
                })
                .level(log_level)
                .chain(std::io::stdout()),
        )
        .chain(
            // Write error messages to stderr
            Dispatch::new()
                .format(move |out, message, record| {
                    write_log_colour(out, message, record, use_colour_stderr, &colours);
                })
                .level(log_level.min(LevelFilter::Warn))
                .chain(std::io::stderr()),
        )
        .chain(
            // Write non-error messages to log file
            Dispatch::new()
                .filter(|metadata| metadata.level() > LevelFilter::Warn)
                .format(write_log_plain)
                .level(log_level.max(LevelFilter::Info))
                .chain(info_log_file),
        )
        .chain(
            // Write error messages to a different log file
            Dispatch::new()
                .format(write_log_plain)
                .level(LevelFilter::Warn)
                .chain(err_log_file),
        );

    // Apply the logger configuration
    dispatch.apply()?;

    Ok(())
}

/// Write to the log in the format we want for MUSE 2.0
fn write_log<T: Display>(out: FormatCallback, level: T, target: &str, message: &Arguments) {
    let timestamp = Local::now().format("%H:%M:%S");

    out.finish(format_args!(
        "[{} {} {}] {}",
        timestamp, level, target, message
    ));
}

/// Write to the log with no colours
fn write_log_plain(out: FormatCallback, message: &Arguments, record: &Record) {
    write_log(out, record.level(), record.target(), message);
}

/// Write to the log with optional colours
fn write_log_colour(
    out: FormatCallback,
    message: &Arguments,
    record: &Record,
    use_colour: bool,
    colours: &ColoredLevelConfig,
) {
    // Format output with or without colour based on `use_colour`
    if use_colour {
        write_log(out, colours.color(record.level()), record.target(), message);
    } else {
        write_log_plain(out, message, record);
    }
}
