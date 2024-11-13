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
/// To control whether the log output is colourised, the `MUSE2_LOG_STYLE` environment variable can
/// be set to:
/// * `always` - Always colourise log output
/// * `auto` - Colourise log output only if the output stream is a terminal
/// * any other value - Disable colourisation
///
/// # Arguments
///
/// * `log_level_from_settings`: The log level specified in `settings.toml`
pub fn init(log_level_from_settings: Option<&str>) {
    // Retrieve the log level from the environment variable or settings, or use the default
    let log_level = env::var("MUSE2_LOG_LEVEL").unwrap_or_else(|_| {
        log_level_from_settings
            .unwrap_or(DEFAULT_LOG_LEVEL)
            .to_string()
    });

    // Convert the log level string to a log::LevelFilter
    let log_level = match log_level.to_lowercase().as_str() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        unknown => panic!("Unknown log level: {}", unknown),
    };

    // Retrieve the log style from the environment variable
    let log_style = env::var("MUSE2_LOG_STYLE").unwrap_or_else(|_| "auto".to_string());

    // Format timestamp as HH:MM:SS
    let timestamp_format = "%H:%M:%S";

    // Check if colour should be applied based on log_style
    let use_colour = match log_style.as_str() {
        "always" => true,
        "auto" => atty::is(atty::Stream::Stdout),
        _ => false,
    };

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
    dispatch.apply().unwrap();
}
