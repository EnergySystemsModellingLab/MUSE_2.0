# Command-Line Help for `muse2`

This document contains the help content for the `muse2` command-line program.

**Command Overview:**

* [`muse2`↴](#muse2)
* [`muse2 run`↴](#muse2-run)
* [`muse2 example`↴](#muse2-example)
* [`muse2 example list`↴](#muse2-example-list)
* [`muse2 example extract`↴](#muse2-example-extract)
* [`muse2 example run`↴](#muse2-example-run)

## `muse2`

A tool for running simulations of energy systems

**Usage:** `muse2 [COMMAND]`

###### **Subcommands:**

* `run` — Run a simulation model
* `example` — Manage example models

## `muse2 run`

Run a simulation model

**Usage:** `muse2 run [OPTIONS] <MODEL_DIR>`

###### **Arguments:**

* `<MODEL_DIR>` — Path to the model directory

###### **Options:**

* `-o`, `--output-dir <OUTPUT_DIR>` — Directory for output files

## `muse2 example`

Manage example models

**Usage:** `muse2 example <COMMAND>`

###### **Subcommands:**

* `list` — List available examples
* `extract` — Extract an example model configuration to a new directory
* `run` — Run an example

## `muse2 example list`

List available examples

**Usage:** `muse2 example list`

## `muse2 example extract`

Extract an example model configuration to a new directory

**Usage:** `muse2 example extract <NAME> [NEW_PATH]`

###### **Arguments:**

* `<NAME>` — The name of the example to extract
* `<NEW_PATH>` — The destination folder for the example

## `muse2 example run`

Run an example

**Usage:** `muse2 example run [OPTIONS] <NAME>`

###### **Arguments:**

* `<NAME>` — The name of the example to run

###### **Options:**

* `-o`, `--output-dir <OUTPUT_DIR>` — Directory for output files

<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
