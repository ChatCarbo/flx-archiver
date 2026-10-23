use crate::compression::{CompressionError, CompressionLevel, CompressionResult};
use crate::decompression::format_zip::ProgressUpdate;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

pub struct GzipCompressor {
    inputs: Vec<PathBuf>,
    level: CompressionLevel,
}

impl GzipCompressor {
    pub fn new(inputs: Vec<PathBuf>, level: CompressionLevel) -> Self {
        Self { inputs, level }
    }

    pub async fn compress(
        &self,
        dest: PathBuf,
        progress_tx: Sender<ProgressUpdate>,
    ) -> Result<CompressionResult, CompressionError> {
        let input_path = self.inputs.first().cloned().ok_or(CompressionError::EmptyInput)?;
        let level = self.level;

        tokio::task::spawn_blocking(move || {
            let gz_level = match level {
                CompressionLevel::Fast => Compression::fast(),
                CompressionLevel::Default => Compression::default(),
                CompressionLevel::Maximum => Compression::best(),
            };

            let src_file = File::open(&input_path)?;
            let total_bytes = src_file.metadata()?.len();
            let mut reader = BufReader::new(src_file);

            let out_file = File::create(&dest)?;
            let writer = BufWriter::new(out_file);
            let mut encoder = GzEncoder::new(writer, gz_level);

            let mut buffer = [0u8; 65536];
            let mut bytes_processed: u64 = 0;
            let file_name = input_path.file_name().unwrap_or_default().to_string_lossy().into_owned();

            loop {
                let read_bytes = reader.read(&mut buffer)?;
                if read_bytes == 0 {
                    break;
                }
                encoder.write_all(&buffer[..read_bytes])?;
                bytes_processed += read_bytes as u64;

                let percentage = if total_bytes > 0 {
                    (bytes_processed as f32 / total_bytes as f32) * 100.0
                } else {
                    0.0
                };

                let _ = progress_tx.blocking_send(ProgressUpdate {
                    bytes_processed,
                    bytes_total: total_bytes,
                    current_file: file_name.clone(),
                    percentage: percentage.min(100.0),
                });
            }

            encoder.finish()?;

            let bytes_compressed = fs::metadata(&dest)?.len();
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