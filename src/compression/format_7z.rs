use crate::compression::{CompressionError, CompressionLevel, CompressionResult};
use crate::decompression::format_zip::ProgressUpdate;
use std::fs;
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

pub struct SevenZCompressor {
    inputs: Vec<PathBuf>,
    _level: CompressionLevel,
}

impl SevenZCompressor {
    pub fn new(inputs: Vec<PathBuf>, level: CompressionLevel) -> Self {
        Self { inputs, _level: level }
    }

    pub async fn compress(
        &self,
        dest: PathBuf,
        progress_tx: Sender<ProgressUpdate>,
    ) -> Result<CompressionResult, CompressionError> {
        let inputs = self.inputs.clone();

        tokio::task::spawn_blocking(move || {
            let mut total_bytes: u64 = 0;
            for input in &inputs {
                if let Ok(meta) = fs::metadata(input) {
                    total_bytes += meta.len();
                }
            }

            let first_name = inputs.first()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();

            let _ = progress_tx.blocking_send(ProgressUpdate {
                bytes_processed: 0,
                bytes_total: total_bytes,
                current_file: first_name.clone(),
                percentage: 10.0,
            });

            // Compression des fichiers cibles via sevenz-rs
            if inputs.len() == 1 && inputs[0].is_dir() {
                sevenz_rust::compress_to_path(&inputs[0], &dest)
                    .map_err(|e| CompressionError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
            } else {
                sevenz_rust::compress_to_path(&inputs[0], &dest)
                    .map_err(|e| CompressionError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
            }

            let bytes_compressed = fs::metadata(&dest)?.len();

            let _ = progress_tx.blocking_send(ProgressUpdate {
                bytes_processed: total_bytes,
                bytes_total: total_bytes,
                current_file: first_name,
                percentage: 100.0,
            });

            let ratio = if total_bytes > 0 {
                (bytes_compressed as f32 / total_bytes as f32) * 100.0
            } else {
                0.0
            };

            Ok(CompressionResult {
                archive_path: dest,
                bytes_original: total_bytes,
                bytes_compressed,
                ratio,
            })
        })
        .await
        .map_err(|_| CompressionError::Cancelled)?
    }
}