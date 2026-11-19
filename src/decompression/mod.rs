use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::sync::mpsc;

pub mod format_7z;
pub mod format_rar;
pub mod format_zip;
pub mod format_stream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Rar,
    Gzip,
    Zstd,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    #[allow(dead_code)] // renseigné par les extracteurs ; l'interface n'affiche que le pourcentage
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
        // 1. Signature (octets magiques) : fiable même si l'extension est fausse ou absente.
        if let Ok(mut f) = File::open(path) {
            let mut b = [0u8; 8];
            let n = f.read(&mut b).unwrap_or(0);
            if n >= 2 {
                match b {
                    [0x50, 0x4B, 0x03, 0x04, ..] | [0x50, 0x4B, 0x05, 0x06, ..] => return Ok(ArchiveFormat::Zip),
                    [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, ..] => return Ok(ArchiveFormat::SevenZ),
                    [0x28, 0xB5, 0x2F, 0xFD, ..] => return Ok(ArchiveFormat::Zstd),
                    [0x1F, 0x8B, ..] => return Ok(ArchiveFormat::Gzip),
                    [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, ..] => return Ok(ArchiveFormat::Rar),
                    _ => {}
                }
            }
        }

        // 2. À défaut, l'extension.
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        
        // Vérification des extensions composées ou spécifiques
        if file_name.ends_with(".tar.zst") || file_name.ends_with(".tzst") {
            return Ok(ArchiveFormat::Zstd);
        }
        if file_name.ends_with(".tar.gz") {
            return Ok(ArchiveFormat::Gzip);
        }

        // Vérification de l'extension simple
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "zip" => Ok(ArchiveFormat::Zip),
                "7z" => Ok(ArchiveFormat::SevenZ),
                "rar" => Ok(ArchiveFormat::Rar),
                "gz" => Ok(ArchiveFormat::Gzip),
                "zst" => Ok(ArchiveFormat::Zstd),
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
    let join_err = |e: tokio::task::JoinError| DecompressionError::Custom(format!("Tâche interrompue : {e}"));

    match format {
        ArchiveFormat::Zip => format_zip::decompress_zip(&archive, &dest, progress_tx).await,
        ArchiveFormat::SevenZ | ArchiveFormat::Rar => {
            let is_rar = format == ArchiveFormat::Rar;
            let name = archive
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            tokio::task::spawn_blocking(move || {
                if is_rar {
                    format_rar::decompress_rar(&archive, &dest)
                } else {
                    format_7z::decompress_7z(&archive, &dest)
                }
            })
            .await
            .map_err(join_err)??;
            // Ces formats n'envoient pas de progression détaillée : on signale la fin.
            let _ = progress_tx
                .send(ProgressUpdate {
                    bytes_processed: 1,
                    bytes_total: 1,
                    current_file: name,
                    percentage: 100.0,
                })
                .await;
            Ok(())
        }
        ArchiveFormat::Gzip | ArchiveFormat::Zstd => {
            let kind = if format == ArchiveFormat::Gzip {
                format_stream::StreamKind::Gzip
            } else {
                format_stream::StreamKind::Zstd
            };
            tokio::task::spawn_blocking(move || format_stream::extract(&archive, &dest, kind, &progress_tx))
                .await
                .map_err(join_err)?
        }
        ArchiveFormat::Unknown => Err(DecompressionError::Custom("Format non supporté".into())),
    }
}
