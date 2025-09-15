# User Guide

## Running MUSE 2.0

Once you have installed MUSE 2.0, you should be able to run it via the `muse2` command-line program.
For details of the command-line interface, [see here](./command_line_help.md).

You can also configure the behaviour of MUSE 2.0 by creating a `settings.toml` file. For more
information, see [the documentation for this file][settings.toml-docs].

[settings.toml-docs]: https://energysystemsmodellinglab.github.io/MUSE_2.0/file_formats/program_settings.html

## Setting the log level

MUSE uses the [`fern`] crate for logging. The default log level is `info`, though this can be
configured either via the `log_level` option in `settings.toml` or by setting the
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
[the `fern` documentation]: https://docs.rs/fern/latest/fern/
