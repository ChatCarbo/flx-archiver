use crate::decompression::DecompressionError;
use std::path::Path;
use unrar::Archive;

pub fn decompress_rar(archive_path: &Path, dest_path: &Path) -> Result<(), DecompressionError> {
    let mut archive = Archive::new(archive_path)
        .open_for_processing()
        .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| DecompressionError::Custom(e.to_string()))?
    {
        let target = dest_path.join(&header.entry().filename);
        archive = if header.entry().is_file() {
            if let Some(parent) = target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            header
                .extract_to(&target)
                .map_err(|e| DecompressionError::Custom(e.to_string()))?
        } else {
            header
                .skip()
                .map_err(|e| DecompressionError::Custom(e.to_string()))?
        };
    }

    Ok(())
}