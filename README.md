# MUSE 2.0

MUSE 2.0 is a tool for running simulations of energy systems, written in Rust. It is a slimmer and
faster version of [the older MUSE tool].

:construction: Note that this repository is under heavy development and not suitable for end users!
:construction:

[the older MUSE tool]: https://github.com/EnergySystemsModellingLab/MUSE_OS

## Getting started

### Installing the Rust toolchain

We recommend that developers use `rustup` to install the Rust toolchain. Follow the instructions on
[the `rustup` website](https://rustup.rs/).

Once you have done so, select the `stable` toolchain (used by this project) as your default with:

```sh
rustup default stable
```

### Working with the project

To build the project, run:

```sh
cargo build
```

To run MUSE, you can run:

```sh
cargo run
```

Tests can be run with:

```sh
cargo test
```

More information is available in [the official `cargo` book](https://doc.rust-lang.org/cargo/).

### Installing `pre-commit` (optional)

It is recommended that developers install the `pre-commit` tool in order to automatically run this
repository's hooks when making a new Git commit. Follow [the instructions on the `pre-commit`
website] in order to get started.

Once you have installed `pre-commit`, you need to enable its use for this repository by installing
the hooks, like so:

```sh
pre-commit install
```

Thereafter, a series of checks should be run every time you commit with Git. In addition, the
`pre-commit` hooks are also run as part of the CI pipeline.

[the instructions on the `pre-commit` website]: https://pre-commit.com/#installation

## Copyright

Copyright © 2024 Imperial College London
