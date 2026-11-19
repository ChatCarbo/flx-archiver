//! Décompression de flux Gzip et Zstd : `.gz` / `.zst` (un seul fichier) et `.tar.gz` / `.tar.zst` (dossier).
//! Le contenu est reconnu par sa structure (en-tête « ustar »), pas seulement par le nom du fichier.

use crate::decompression::{DecompressionError, ProgressUpdate};
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::Path;
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Gzip,
    Zstd,
}

/// Compte les octets compressés lus pour alimenter la barre de progression.
struct ProgressReader<R> {
    inner: R,
    read: u64,
    total: u64,
    last_pct: i32,
    name: String,
    tx: Option<Sender<ProgressUpdate>>,
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        if let Some(tx) = &self.tx {
            if self.total > 0 {
                // Plafonné à 99 % : le 100 % est envoyé une fois l'écriture terminée.
                let pct = ((self.read as f64 / self.total as f64) * 100.0).min(99.0) as i32;
                if pct != self.last_pct {
                    self.last_pct = pct;
                    let _ = tx.blocking_send(ProgressUpdate {
                        bytes_processed: self.read,
                        bytes_total: self.total,
                        current_file: self.name.clone(),
                        percentage: pct as f32,
                    });
                }
            }
        }
        Ok(n)
    }
}

fn open_decoder(
    path: &Path,
    kind: StreamKind,
    tx: Option<Sender<ProgressUpdate>>,
) -> Result<Box<dyn Read>, DecompressionError> {
    let file = File::open(path)?;
    let total = file.metadata()?.len();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let reader = ProgressReader {
        inner: BufReader::new(file),
        read: 0,
        total,
        last_pct: -1,
        name,
        tx,
    };
    Ok(match kind {
        StreamKind::Gzip => Box::new(flate2::read::MultiGzDecoder::new(reader)),
        StreamKind::Zstd => Box::new(zstd::Decoder::new(reader)?),
    })
}

fn looks_like_tar(path: &Path, kind: StreamKind) -> Result<bool, DecompressionError> {
    let mut decoder = open_decoder(path, kind, None)?;
    let mut head = [0u8; 512];
    let mut filled = 0;
    while filled < head.len() {
        let n = decoder.read(&mut head[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled >= 262 && &head[257..262] == b"ustar")
}

pub fn extract(
    path: &Path,
    dest: &Path,
    kind: StreamKind,
    tx: &Sender<ProgressUpdate>,
) -> Result<(), DecompressionError> {
    std::fs::create_dir_all(dest)?;
    let total = std::fs::metadata(path)?.len();
    let is_tar = looks_like_tar(path, kind)?;
    let mut reader = open_decoder(path, kind, Some(tx.clone()))?;

    if is_tar {
        // `tar` refuse lui-même les chemins qui sortent du dossier de destination.
        tar::Archive::new(reader).unpack(dest)?;
    } else {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "output".to_string());
        let mut out = BufWriter::new(File::create(dest.join(stem))?);
        io::copy(&mut reader, &mut out)?;
        out.flush()?;
    }

    let _ = tx.blocking_send(ProgressUpdate {
        bytes_processed: total,
        bytes_total: total,
        current_file: String::new(),
        percentage: 100.0,
    });
    Ok(())
}
