//! The `log` module provides initialisation and configuration of the application's logging system.
//!
//! This module sets up logging with various levels (error, warn, info, debug, trace) and optional
//! colourisation based on terminal support. It also allows configuration of the log level through
//! environment variables.
use anyhow::{bail, Result};
use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::{Dispatch, FormatCallback};
use log::Record;
use std::env;
use std::fmt::{Arguments, Display};
use std::io::IsTerminal;

/// The default log level for the program.
///
/// Note that we disable logging when running tests.
const DEFAULT_LOG_LEVEL: &str = if cfg!(test) { "off" } else { "info" };

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
pub fn init(log_level_from_settings: Option<&str>) -> Result<()> {
    // Retrieve the log level from the environment variable or settings, or use the default
    let log_level = env::var("MUSE2_LOG_LEVEL").unwrap_or_else(|_| {
        log_level_from_settings
            .unwrap_or(DEFAULT_LOG_LEVEL)
            .to_string()
    });

    // Convert the log level string to a log::LevelFilter
    let log_level = match log_level.to_lowercase().as_str() {
        "off" => log::LevelFilter::Off,
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        unknown => bail!("Unknown log level: {}", unknown),
    };

    // Automatically apply colours only if the output is a terminal
    let use_colour = std::io::stdout().is_terminal();

    // Set up colours for log levels
    let colours = if use_colour {
        Some(
            ColoredLevelConfig::new()
                .error(Color::Red)
                .warn(Color::Yellow)
                .info(Color::Green)
                .debug(Color::Blue)
                .trace(Color::Magenta),
        )
    } else {
        None
    };

    // Configure the logger
    let dispatch = Dispatch::new()
        .format(move |out, message, record| {
            write_log_colour(out, message, record, &colours);
        })
        .level(log_level)
        .chain(std::io::stdout());

    // Apply the logger configuration
    dispatch.apply()?;

    Ok(())
}

/// Write to the log in the format we want for MUSE 2.0
fn write_log<T: Display>(out: FormatCallback, level: T, target: &str, message: &Arguments) {
    // Format timestamp as HH:MM:SS
    let timestamp_format = "%H:%M:%S";

    // Generate the current timestamp
    let timestamp = Local::now().format(timestamp_format);

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
    colours: &Option<ColoredLevelConfig>,
) {
    // Format output with or without colour based on `use_colour`
    if let Some(colours) = colours {
        write_log(out, colours.color(record.level()), record.target(), message);
    } else {
        write_log_plain(out, message, record);
    }
}
