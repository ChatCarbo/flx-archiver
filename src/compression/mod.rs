use crate::decompression::ProgressUpdate;
use std::fs::{self, File};
use std::io::{Read, Write, BufWriter}; // BufReader supprimé
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Zstd,
    Gzip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionLevel {
    Fast,
    Default,
    Maximum,
}

impl CompressionLevel {
    pub fn to_zstd_level(self) -> i32 {
        match self {
            CompressionLevel::Fast => 1,
            CompressionLevel::Default => 3,
            CompressionLevel::Maximum => 19,
        }
    }

    pub fn to_gzip_level(self) -> flate2::Compression {
        match self {
            CompressionLevel::Fast => flate2::Compression::fast(),
            CompressionLevel::Default => flate2::Compression::default(),
            CompressionLevel::Maximum => flate2::Compression::best(),
        }
    }
}

struct FileItem {
    full_path: PathBuf,
    relative_path: PathBuf,
    size: u64,
}

fn collect_files(inputs: &[PathBuf]) -> std::io::Result<(Vec<FileItem>, u64)> {
    let mut items = Vec::new();
    let mut total_size = 0u64;

    for input in inputs {
        let base_dir = input.parent().unwrap_or(input);

        if input.is_file() {
            let metadata = fs::metadata(input)?;
            let rel = input.strip_prefix(base_dir).unwrap_or(input).to_path_buf();
            total_size += metadata.len();
            items.push(FileItem {
                full_path: input.clone(),
                relative_path: rel,
                size: metadata.len(),
            });
        } else if input.is_dir() {
            for entry in WalkDir::new(input).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let metadata = entry.metadata()?;
                    let rel = path.strip_prefix(base_dir).unwrap_or(path).to_path_buf();
                    total_size += metadata.len();
                    items.push(FileItem {
                        full_path: path.to_path_buf(),
                        relative_path: rel,
                        size: metadata.len(),
                    });
                }
            }
        }
    }

    Ok((items, total_size))
}

pub fn compress(
    files: Vec<PathBuf>,
    dest: PathBuf,
    format: ArchiveFormat,
    level: CompressionLevel,
    tx: mpsc::Sender<ProgressUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match format {
        ArchiveFormat::Zip => compress_zip(&files, &dest, level, &tx),
        ArchiveFormat::SevenZ => compress_7z(&files, &dest, level, &tx),
        ArchiveFormat::Zstd => compress_zstd(&files, &dest, level, &tx),
        ArchiveFormat::Gzip => compress_gzip(&files, &dest, level, &tx),
    }
}

fn compress_zip(
    files: &[PathBuf],
    dest: &Path,
    _level: CompressionLevel,
    tx: &mpsc::Sender<ProgressUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (items, total_bytes) = collect_files(files)?;
    let out_file = File::create(dest)?;
    let writer = BufWriter::new(out_file);
    let mut zip = zip::ZipWriter::new(writer);

    // Utilisation de FileOptions au lieu de SimpleFileOptions
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut processed_bytes = 0u64;
    let mut buffer = vec![0u8; 64 * 1024];

    for item in &items {
        let name_str = item.relative_path.to_string_lossy().replace('\\', "/");
        zip.start_file(name_str.clone(), options)?;

        let mut f = File::open(&item.full_path)?;
        loop {
            let n = f.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            zip.write_all(&buffer[..n])?;
            processed_bytes += n as u64;

            let pct = if total_bytes > 0 {
                (processed_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            let _ = tx.blocking_send(ProgressUpdate {
                bytes_processed: processed_bytes,
                bytes_total: total_bytes,
                current_file: name_str.clone(),
                percentage: pct.min(99.9),
            });
        }
    }

    zip.finish()?;

    let _ = tx.blocking_send(ProgressUpdate {
        bytes_processed: total_bytes,
        bytes_total: total_bytes,
        current_file: "Terminé".to_string(),
        percentage: 100.0,
    });

    Ok(())
}

fn compress_7z(
    files: &[PathBuf],
    dest: &Path,
    _level: CompressionLevel,
    tx: &mpsc::Sender<ProgressUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_, total_bytes) = collect_files(files)?;

    if files.len() == 1 {
        sevenz_rust::compress_to_path(&files[0], dest)?;
    } else {
        let parent = files[0].parent().unwrap_or(&files[0]);
        sevenz_rust::compress_to_path(parent, dest)?;
    }

    let _ = tx.blocking_send(ProgressUpdate {
        bytes_processed: total_bytes,
        bytes_total: total_bytes,
        current_file: "Terminé".to_string(),
        percentage: 100.0,
    });

    Ok(())
}

fn compress_zstd(
    files: &[PathBuf],
    dest: &Path,
    level: CompressionLevel,
    tx: &mpsc::Sender<ProgressUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (items, total_bytes) = collect_files(files)?;
    let out_file = File::create(dest)?;
    let buf_writer = BufWriter::new(out_file);
    let encoder = zstd::stream::write::Encoder::new(buf_writer, level.to_zstd_level())?.auto_finish();

    if items.len() == 1 && files.len() == 1 && files[0].is_file() {
        let mut f = File::open(&items[0].full_path)?;
        let mut processed_bytes = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];
        let mut writer = encoder;

        loop {
            let n = f.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n])?;
            processed_bytes += n as u64;

            let pct = if total_bytes > 0 {
                (processed_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            let _ = tx.blocking_send(ProgressUpdate {
                bytes_processed: processed_bytes,
                bytes_total: total_bytes,
                current_file: items[0].relative_path.to_string_lossy().to_string(),
                percentage: pct.min(99.9),
            });
        }
    } else {
        let mut tar = tar::Builder::new(encoder);
        let mut processed_bytes = 0u64;

        for item in &items {
            let name_str = item.relative_path.to_string_lossy().replace('\\', "/");
            let mut f = File::open(&item.full_path)?;
            tar.append_file(&name_str, &mut f)?;

            processed_bytes += item.size;
            let pct = if total_bytes > 0 {
                (processed_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            let _ = tx.blocking_send(ProgressUpdate {
                bytes_processed: processed_bytes,
                bytes_total: total_bytes,
                current_file: name_str,
                percentage: pct.min(99.9),
            });
        }
        tar.finish()?;
    }

    let _ = tx.blocking_send(ProgressUpdate {
        bytes_processed: total_bytes,
        bytes_total: total_bytes,
        current_file: "Terminé".to_string(),
        percentage: 100.0,
    });

    Ok(())
}

fn compress_gzip(
    files: &[PathBuf],
    dest: &Path,
    level: CompressionLevel,
    tx: &mpsc::Sender<ProgressUpdate>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (items, total_bytes) = collect_files(files)?;
    let out_file = File::create(dest)?;
    let buf_writer = BufWriter::new(out_file);
    let encoder = flate2::write::GzEncoder::new(buf_writer, level.to_gzip_level());

    if items.len() == 1 && files.len() == 1 && files[0].is_file() {
        let mut f = File::open(&items[0].full_path)?;
        let mut processed_bytes = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];
        let mut writer = encoder;

        loop {
            let n = f.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n])?;
            processed_bytes += n as u64;

            let pct = if total_bytes > 0 {
                (processed_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            let _ = tx.blocking_send(ProgressUpdate {
                bytes_processed: processed_bytes,
                bytes_total: total_bytes,
                current_file: items[0].relative_path.to_string_lossy().to_string(),
                percentage: pct.min(99.9),
            });
        }
        writer.finish()?;
    } else {
        let mut tar = tar::Builder::new(encoder);
        let mut processed_bytes = 0u64;

        for item in &items {
            let name_str = item.relative_path.to_string_lossy().replace('\\', "/");
            let mut f = File::open(&item.full_path)?;
            tar.append_file(&name_str, &mut f)?;

            processed_bytes += item.size;
            let pct = if total_bytes > 0 {
                (processed_bytes as f32 / total_bytes as f32) * 100.0
            } else {
                100.0
            };

            let _ = tx.blocking_send(ProgressUpdate {
                bytes_processed: processed_bytes,
                bytes_total: total_bytes,
                current_file: name_str,
                percentage: pct.min(99.9),
            });
        }
        let gz_writer = tar.into_inner()?;
        gz_writer.finish()?;
    }

    let _ = tx.blocking_send(ProgressUpdate {
        bytes_processed: total_bytes,
        bytes_total: total_bytes,
        current_file: "Terminé".to_string(),
        percentage: 100.0,
    });

    Ok(())
}