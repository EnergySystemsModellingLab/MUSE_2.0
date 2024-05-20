//! Common functionality used by tests.
use std::path::{Path, PathBuf};

/// Get the path to the example folder in this repository.
pub fn get_example_path() -> PathBuf {
    Path::new(file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("example")
}
