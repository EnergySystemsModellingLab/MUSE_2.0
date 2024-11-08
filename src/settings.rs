//! Code for loading program settings.
use crate::input::read_toml;
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

const SETTINGS_FILE_NAME: &str = "settings.toml";

/// Program settings
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Settings {
    pub log_level: Option<String>,
}

impl Settings {
    /// Read the contents of a settings file from the model directory.
    ///
    /// If the file is not present, default values for settings will be used
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    pub fn from_path<P: AsRef<Path>>(model_dir: P) -> Result<Settings> {
        let file_path = model_dir.as_ref().join(SETTINGS_FILE_NAME);
        if !file_path.is_file() {
            return Ok(Settings::default());
        }

        read_toml(&file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_settings_from_path_no_file() {
        let dir = tempdir().unwrap();
        assert_eq!(
            Settings::from_path(dir.path()).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn test_settings_from_path() {
        let dir = tempdir().unwrap();
        {
            let mut file = File::create(dir.path().join(SETTINGS_FILE_NAME)).unwrap();
            writeln!(file, "log_level = \"warn\"").unwrap();
        }
        assert_eq!(
            Settings::from_path(dir.path()).unwrap(),
            Settings {
                log_level: Some("warn".to_string())
            }
        );
    }
}
