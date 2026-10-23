use crate::decompression::{DecompressionError, ProgressUpdate};
use std::fs::File;
use std::io;
use std::path::Path;
use tokio::sync::mpsc;
use zip::ZipArchive;

pub async fn decompress_zip(
    archive_path: &Path,
    dest_path: &Path,
    progress_tx: mpsc::Sender<ProgressUpdate>,
) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;
    let total_files = archive.len() as u64;

    for i in 0..archive.len() {
        let mut file_name = String::new();
        let mut processed = false;

        {
            let mut file = archive.by_index(i)?;
            if let Some(enclosed_path) = file.enclosed_name() {
                let outpath = dest_path.join(enclosed_path);
                file_name = file.name().to_string();

                if file.name().ends_with('/') {
                    std::fs::create_dir_all(&outpath)?;
                } else {
                    if let Some(p) = outpath.parent() {
                        if !p.exists() {
                            std::fs::create_dir_all(p)?;
                        }
                    }
                    let mut outfile = File::create(&outpath)?;
                    io::copy(&mut file, &mut outfile)?;
                }
                processed = true;
            }
        } // `file` est libéré ici avant l'await

        if processed {
            let progress = ProgressUpdate {
                bytes_processed: i as u64 + 1,
                bytes_total: total_files,
                current_file: file_name,
                percentage: ((i + 1) as f32 / total_files as f32) * 100.0,
            };
            let _ = progress_tx.send(progress).await;
        }
    }

    Ok(())
}