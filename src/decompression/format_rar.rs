use crate::decompression::DecompressionError;
use crate::security::safe_join;
use std::path::Path;
use unrar::Archive;

pub fn decompress_rar(archive_path: &Path, dest_path: &Path) -> Result<(), DecompressionError> {
    let mut archive = Archive::new(archive_path)
        .open_for_processing()
        .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    let mut rejected = 0usize;

    while let Some(header) = archive
        .read_header()
        .map_err(|e| DecompressionError::Custom(e.to_string()))?
    {
        let is_file = header.entry().is_file();
        let target = safe_join(dest_path, &header.entry().filename);

        archive = match (target, is_file) {
            (Ok(target), true) => {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                header
                    .extract_to(&target)
                    .map_err(|e| DecompressionError::Custom(e.to_string()))?
            }
            (Err(_), _) => {
                rejected += 1;
                header
                    .skip()
                    .map_err(|e| DecompressionError::Custom(e.to_string()))?
            }
            (Ok(_), false) => header
                .skip()
                .map_err(|e| DecompressionError::Custom(e.to_string()))?,
        };
    }

    if rejected > 0 {
        return Err(DecompressionError::Custom(format!(
            "{rejected} entrée(s) dangereuse(s) ignorée(s) (chemin hors du dossier de destination)"
        )));
    }
    Ok(())
}
