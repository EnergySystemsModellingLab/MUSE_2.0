//! The `log` module provides initialisation and configuration of the application's logging system.
//!
//! This module sets up logging with various levels (error, warn, info, debug, trace) and optional
//! colourisation based on terminal support. It also allows configuration of the log level through
//! environment variables.
use anyhow::{bail, Result};
use atty;
use chrono::Local;
use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use std::env;

pub(crate) const DEFAULT_LOG_LEVEL: &str = "info";

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

    // Format timestamp as HH:MM:SS
    let timestamp_format = "%H:%M:%S";

    // Automatically apply colours only if the output is a terminal
    let use_colour = atty::is(atty::Stream::Stdout);

    // Set up colours for log levels
    let colours = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Magenta);

    // Configure the logger
    let dispatch = Dispatch::new()
        .format(move |out, message, record| {
            // Generate the current timestamp
            let timestamp = Local::now().format(timestamp_format);

            // Format output with or without colour based on `use_colour`
            if use_colour {
                out.finish(format_args!(
                    "[{} {} {}] {}",
                    timestamp,
                    colours.color(record.level()),
                    record.target(),
                    message
                ))
            } else {
                out.finish(format_args!(
                    "[{} {} {}] {}",
                    timestamp,
                    record.level(),
                    record.target(),
                    message
                ))
            }
        })
        .level(log_level)
        .chain(std::io::stdout());

    // Apply the logger configuration
    dispatch.apply()?;

    Ok(())
}
