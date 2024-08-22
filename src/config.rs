use std::{fs, io, path::PathBuf};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AnkiSyncConfigError {
    #[error("Unable to find env var {0}")]
    EnvVarMissing(String),
    #[error("Error reading config file: {0}")]
    ConfigFileError(#[from] io::Error),
}

pub fn load_config(config_path: &PathBuf) -> Result<Vec<PathBuf>, AnkiSyncConfigError> {
    let config_contents = fs::read_to_string(config_path)?;
    Ok(config_contents.lines().map(PathBuf::from).collect())
}
