# User Guide

## Setting the log level

MUSE uses the [`fern`] crate for logging. The default log level is `info`, though this can be
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
terminal rather than a file).

For more information, please consult [the `fern` documentation].

[`fern`]: https://crates.io/crates/fern
[`settings.toml`]: ../examples/simple/settings.toml
[the `fern` documentation]: https://docs.rs/fern/latest/fern/
