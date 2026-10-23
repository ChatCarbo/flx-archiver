use crate::compression::{CompressionError, CompressionLevel};
use crate::decompression::ProgressUpdate;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use tokio::sync::mpsc;
use zip::write::FileOptions;
use zip::ZipWriter;

pub async fn compress_zip(
    files: Vec<PathBuf>,
    dest: PathBuf,
    level: CompressionLevel,
    progress_tx: mpsc::Sender<ProgressUpdate>,
) -> Result<(), CompressionError> {
    let file = File::create(&dest)?;
    let mut zip = ZipWriter::new(file);

    let compression_method = match level {
        CompressionLevel::Fast | CompressionLevel::Default | CompressionLevel::Maximum => {
            zip::CompressionMethod::Deflated
        }
    };

    let options = FileOptions::default().compression_method(compression_method);
    let total_files = files.len() as u64;

    for (index, path) in files.into_iter().enumerate() {
        if path.is_file() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_string());

            zip.start_file(&name, options)?;
            let mut f = File::open(&path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;

            let progress = ProgressUpdate {
                bytes_processed: index as u64 + 1,
                bytes_total: total_files,
                current_file: name,
                percentage: ((index + 1) as f32 / total_files as f32) * 100.0,
            };
            let _ = progress_tx.send(progress).await;
        }
    }

    zip.finish()?;
    Ok(())
}