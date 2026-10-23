use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SecurityError {
    #[error("Tentative de traversée de répertoire détectée (Zip Slip) : {0}")]
    InvalidPath(String),
}

#[allow(dead_code)]
pub fn validate_archive_path(entry_name: &str, dest_path: &Path) -> Result<PathBuf, SecurityError> {
    let target = dest_path.join(entry_name);
    if target.starts_with(dest_path) {
        Ok(target)
    } else {
        Err(SecurityError::InvalidPath(entry_name.to_string()))
    }
}