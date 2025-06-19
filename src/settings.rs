//! Code for loading program settings.
use crate::input::read_toml;
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

const SETTINGS_FILE_NAME: &str = "settings.toml";

/// Program settings from config file
#[derive(Debug, Default, Deserialize, PartialEq)]
pub struct Settings {
    /// The user's preferred logging level
    pub log_level: Option<String>,
    /// Whether to write additional information to CSV files
    #[serde(default)]
    pub debug_model: bool,
}

impl Settings {
    /// Read the contents of a settings file from the model directory.
    ///
    /// If the file is not present, default values for settings will be used
    ///
    /// # Arguments
    ///
    /// * `model_dir` - Folder containing model configuration files
    ///
    /// # Returns
    ///
    /// The program settings as a `Settings` struct or an error if the file is invalid
    pub fn load() -> Result<Settings> {
        let file_path = Path::new(SETTINGS_FILE_NAME);
        if !file_path.is_file() {
            return Ok(Settings::default());
        }

        read_toml(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use current_dir::Cwd;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_settings_from_path_no_file() {
        let dir = tempdir().unwrap();
        let mut cwd = Cwd::mutex().lock().unwrap();
        cwd.set(dir.path()).unwrap();
        assert_eq!(Settings::load().unwrap(), Settings::default());
    }

    #[test]
    fn test_settings_from_path() {
        let dir = tempdir().unwrap();
        let mut cwd = Cwd::mutex().lock().unwrap();
        cwd.set(dir.path()).unwrap();

        {
            let mut file = File::create(Path::new(SETTINGS_FILE_NAME)).unwrap();
            writeln!(file, "log_level = \"warn\"").unwrap();
        }

        assert_eq!(
            Settings::load().unwrap(),
            Settings {
                log_level: Some("warn".to_string()),
                debug_model: false
            }
        );
    }
}
