use env_logger::Env;

/// Initialise the program logger.
///
/// The user can specify their preferred logging level via the `settings.toml` file or with the
/// `MUSE2_LOG_LEVEL` environment variable. If both are provided, the environment variable takes
/// precedence. If neither is supplied, the default log level is `info`.
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
    let fallback_log_level = log_level_from_settings.unwrap_or("info");
    let env = Env::new()
        .filter_or("MUSE2_LOG_LEVEL", fallback_log_level)
        .write_style("MUSE2_LOG_STYLE");

    // Initialise logger
    env_logger::init_from_env(env);
}
