# User Guide

## Setting the log level

MUSE uses the [`env_logger`] crate for logging. The default log level is `info`, though this can be
configured either via the `log_level` option in [`settings.toml`] or by setting the
`MUSE2_LOG_LEVEL` environment variable. (If both are used, the environment variable takes
precedence.)

The possible options are:

- `error`
- `warn`
- `info`
- `debug`
- `trace`
- `off`

By default, MUSE will colourise the log output if this is available (i.e. it is outputting to a
terminal rather than a file), but this can be overridden by modifying the `MUSE2_LOG_STYLE`
environment variable.

For more information, please consult [the `env_logger` documentation].

[`env_logger`]: https://crates.io/crates/env_logger
[`settings.toml`]: ../examples/simple/settings.toml
[the `env_logger` documentation]: https://docs.rs/env_logger/latest/env_logger
