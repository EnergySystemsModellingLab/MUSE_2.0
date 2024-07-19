use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::config::{Appender, Root};
use log4rs::Config;
use std::env;
use std::error::Error;
use std::str::FromStr;

/// TODO - FIX THIS UP
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
pub fn init(log_level_from_settings: Option<&str>) -> Result<(), Box<dyn Error>> {
    let level = match env::var("MUSE2_LOG_LEVEL") {
        Err(_) => match log_level_from_settings {
            None => LevelFilter::Info,
            Some(level_str) => LevelFilter::from_str(level_str)?,
        },
        Ok(ref level_str) => LevelFilter::from_str(level_str)?,
    };

    let stdout = ConsoleAppender::builder().target(Target::Stdout).build();
    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(level))
        .unwrap();

    let _handle = log4rs::init_config(config).unwrap();

    Ok(())
}
