use muse2::model::Model;
use std::path::{Path, PathBuf};

/// Get the path to the example model.
fn get_model_dir() -> PathBuf {
    Path::new(file!())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join("simple")
}

/// An integration test which attempts to load the example model
#[test]
fn test_model_from_path() {
    Model::from_path(get_model_dir()).unwrap();
}
