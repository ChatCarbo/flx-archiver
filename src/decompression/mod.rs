use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::mpsc;

pub mod format_7z;
pub mod format_rar;
pub mod format_zip;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Rar,
    Gzip,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub bytes_processed: u64,
    #[allow(dead_code)]
    pub bytes_total: u64,
    pub current_file: String,
    pub percentage: f32,
}

#[derive(Error, Debug)]
pub enum DecompressionError {
    #[error("Erreur d'E/S : {0}")]
    Io(#[from] std::io::Error),

    #[error("Erreur Zip : {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Erreur de décompression : {0}")]
    Custom(String),
}

pub struct FormatDetector;

impl FormatDetector {
    pub fn detect_format(path: &Path) -> Result<ArchiveFormat, DecompressionError> {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "zip" => Ok(ArchiveFormat::Zip),
                "7z" => Ok(ArchiveFormat::SevenZ),
                "rar" => Ok(ArchiveFormat::Rar),
                "gz" => Ok(ArchiveFormat::Gzip),
                _ => Ok(ArchiveFormat::Unknown),
            }
        } else {
            Ok(ArchiveFormat::Unknown)
        }
    }
}

pub async fn decompress(
    archive: PathBuf,
    dest: PathBuf,
    progress_tx: mpsc::Sender<ProgressUpdate>,
) -> Result<(), DecompressionError> {
    let format = FormatDetector::detect_format(&archive)?;
    match format {
        ArchiveFormat::Zip => format_zip::decompress_zip(&archive, &dest, progress_tx).await,
        ArchiveFormat::SevenZ => format_7z::decompress_7z(&archive, &dest),
        ArchiveFormat::Rar => format_rar::decompress_rar(&archive, &dest),
        _ => Err(DecompressionError::Custom("Format non supporté".into())),
    }
}