use chrono::Local; // Used for timestamp formatting
use colored::Colorize;
use fern::Dispatch;
use std::env;

pub(crate) const DEFAULT_LOG_LEVEL: &str = "info";

/// Initialise the program logger using the `fern` logging library with colored output.
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
/// To control whether the log output is colorized, the `MUSE2_LOG_STYLE` environment variable can
/// be set to:
/// * `always` - Always colorize log output
/// * `auto` - Colorize log output only if the output stream is a terminal
/// * any other value - Disable colorization
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

    // Configure the logger
    let dispatch = Dispatch::new()
        .format(move |out, message, record| {
            // Format the log level with color
            let level = match record.level() {
                log::Level::Error => "ERROR".red(),
                log::Level::Warn => "WARN".yellow(),
                log::Level::Info => "INFO".green(),
                log::Level::Debug => "DEBUG".blue(),
                log::Level::Trace => "TRACE".purple(),
            };

            // Format timestamp as HH:MM:SS
            let timestamp = Local::now().format("%H:%M:%S");

            // Check if color should be applied based on log_style
            let use_color = match log_style.as_str() {
                "always" => true,
                "auto" => atty::is(atty::Stream::Stdout),
                _ => false,
            };

            // Format the output with or without color based on `use_color`
            if use_color {
                out.finish(format_args!(
                    "[{} {} {}] {}",
                    timestamp,
                    level,
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
