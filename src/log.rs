use fern::Dispatch;
use std::env;

pub(crate) const DEFAULT_LOG_LEVEL: &str = "info";

/// Initialise the program logger.
///
/// The user can specify their preferred logging level via the `settings.toml` file (defaulting to
/// `info` if not present) or with the `MUSE2_LOG_LEVEL` environment variable. If both are provided,
/// the environment variable takes precedence.
///
/// Possible options are:
///
/// * `error`
/// * `warn`
/// * `info`
/// * `debug`
/// * `trace`
///
/// To choose whether or not to colourise the log output, the `MUSE2_LOG_STYLE` environment
/// variable can be used. See [the `env_logger`
/// documentation](https://docs.rs/env_logger/latest/env_logger/index.html#disabling-colors) for
/// details.
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
        _ => log::LevelFilter::Info,
    };

    // Retrieve the log style from the environment variable
    let log_style = env::var("MUSE2_LOG_STYLE").unwrap_or_else(|_| "auto".to_string());

    // Configure the logger
    let dispatch = Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                // humantime::format_rfc3339_seconds(SystemTime::now()),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stdout());

    // Apply the logger configuration
    dispatch.apply().unwrap();
}
