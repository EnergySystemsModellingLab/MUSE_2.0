# Developer Guide

This is a guide for those who wish to contribute to the MUSE 2.0 project or make local changes to
the code.

[The API documentation is available here.](./api/muse2)

## Installing the Rust toolchain

We recommend that developers use `rustup` to install the Rust toolchain. Follow the instructions on
[the `rustup` website](https://rustup.rs/).

Once you have done so, select the `stable` toolchain (used by this project) as your default with:

```sh
rustup default stable
```

## Working with the project

To build the project, run:

```sh
cargo build
```

To run MUSE with the example input files, you can run:

```sh
cargo run examples/simple
```

Tests can be run with:

```sh
cargo test
```

More information is available in [the official `cargo` book](https://doc.rust-lang.org/cargo/).

## Checking test coverage

We use [Codecov](https://about.codecov.io/) to check whether pull requests introduce code without
tests.

To check coverage locally (i.e. to make sure newly written code has tests), we recommend using
[cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov).

It can be installed with:

```sh
cargo install cargo-llvm-cov
```

Once installed, you can use it like so:

```sh
cargo llvm-cov --open
```

This will generate a report in HTML format showing which lines are not currently covered by tests
and open it in your default browser.

## Developing the documentation

We use [mdBook](https://rust-lang.github.io/mdBook/) for generating technical documentation.

If you are developing the documentation locally, you may want to check that your changes render
correctly (especially if you are working with equations).

To do this, you first need to install mdBook:

```sh
cargo install mdbook
```

You can then view the documentation in your browser like so:

```sh
mdbook serve -o
```

## Pre-Commit hooks

Developers must install the `pre-commit` tool in order to automatically run this
repository's hooks when making a new Git commit. Follow [the instructions on the `pre-commit`
website](https://pre-commit.com/#install) in order to get started.

Once you have installed `pre-commit`, you need to enable its use for this repository by installing
the hooks, like so:

```sh
pre-commit install
```

Thereafter, a series of checks should be run every time you commit with Git. In addition, the
`pre-commit` hooks are also run as part of the CI pipeline.
