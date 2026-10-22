use crate::decompression::DecompressionError;
use sevenz_rust::SevenZReader;
use std::fs::File;
use std::path::{Path, PathBuf};

fn sanitize_extract_path(entry_name: &str, dest_path: &Path) -> Option<PathBuf> {
    let path = Path::new(entry_name);
    if path.is_absolute() || path.components().any(|c| c == std::path::Component::ParentDir) {
        return None;
    }
    Some(dest_path.join(path))
}

pub fn decompress_7z(archive_path: &Path, dest_path: &Path) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let len = file.metadata()?.len();

    let mut sz = SevenZReader::new(file, len, sevenz_rust::Password::empty())
        .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    sz.for_each_entries(|entry, reader| {
        let entry_name = entry.name();
        if let Some(target_path) = sanitize_extract_path(entry_name, dest_path) {
            if entry.is_directory() {
                let _ = std::fs::create_dir_all(&target_path);
            } else {
                if let Some(parent) = target_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Ok(mut out_file) = File::create(&target_path) {
                    let _ = std::io::copy(reader, &mut out_file);
                }
            }
        }
        Ok(true)
    })
    .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    Ok(())
}