use env_logger::Env;

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
    let env = Env::new()
        .filter_or(
            "MUSE2_LOG_LEVEL",
            log_level_from_settings.unwrap_or(DEFAULT_LOG_LEVEL),
        )
        .write_style("MUSE2_LOG_STYLE");

    // Initialise logger
    env_logger::init_from_env(env);
}
