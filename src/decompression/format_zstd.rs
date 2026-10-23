use crate::decompression::ProgressUpdate;
use crate::decompression::DecompressionError;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

pub struct ZstdDecompressor {
    archive_path: PathBuf,
}

impl ZstdDecompressor {
    pub fn new(archive_path: PathBuf) -> Self {
        Self { archive_path }
    }

    pub async fn extract(
        &self,
        dest: PathBuf,
        progress_tx: Sender<ProgressUpdate>,
    ) -> Result<(), DecompressionError> {
        let archive_path = self.archive_path.clone();

        tokio::task::spawn_blocking(move || {
            let src_file = File::open(&archive_path)?;
            let total_bytes = src_file.metadata()?.len();

            let target_filename = archive_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let target_path = dest.join(&target_filename);

            let reader = BufReader::new(src_file);
            let mut decoder = zstd::Decoder::new(reader)?;

            let out_file = File::create(&target_path)?;
            let mut writer = BufWriter::new(out_file);

            let mut buffer = [0u8; 65536]; // Tampon de 64 Ko
            let mut bytes_processed: u64 = 0;

            loop {
                use std::io::Read;
                let read_bytes = decoder.read(&mut buffer)?;
                if read_bytes == 0 {
                    break;
                }

                use std::io::Write;
                writer.write_all(&buffer[..read_bytes])?;
                bytes_processed += read_bytes as u64;

                let percentage = if total_bytes > 0 {
                    (bytes_processed as f32 / total_bytes as f32) * 100.0
                } else {
                    50.0
                };

                let _ = progress_tx.blocking_send(ProgressUpdate {
                    bytes_processed,
                    bytes_total: total_bytes,
                    current_file: target_filename.clone(),
                    percentage: percentage.min(100.0),
                });
            }

            Ok::<(), DecompressionError>(())
        })
        .await
        .map_err(|_| DecompressionError::Custom("Task cancelled or failed".into()))?
    }
}