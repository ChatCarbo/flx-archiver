use crate::decompression::DecompressionError;
use crate::security::safe_join;
use sevenz_rust::SevenZReader;
use std::fs::File;
use std::path::Path;

pub fn decompress_7z(archive_path: &Path, dest_path: &Path) -> Result<(), DecompressionError> {
    let file = File::open(archive_path)?;
    let len = file.metadata()?.len();

    let mut sz = SevenZReader::new(file, len, sevenz_rust::Password::empty())
        .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    let mut rejected = 0usize;
    let mut first_error: Option<String> = None;

    sz.for_each_entries(|entry, reader| {
        let Ok(target_path) = safe_join(dest_path, Path::new(entry.name())) else {
            rejected += 1;
            return Ok(true);
        };
        if entry.is_directory() {
            if let Err(e) = std::fs::create_dir_all(&target_path) {
                first_error = Some(format!("{} : {}", entry.name(), e));
                return Ok(false);
            }
        } else {
            if let Some(parent) = target_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    first_error = Some(format!("{} : {}", entry.name(), e));
                    return Ok(false);
                }
            }
            let result = File::create(&target_path).and_then(|mut out| std::io::copy(reader, &mut out));
            if let Err(e) = result {
                first_error = Some(format!("{} : {}", entry.name(), e));
                return Ok(false);
            }
        }
        Ok(true)
    })
    .map_err(|e| DecompressionError::Custom(e.to_string()))?;

    if let Some(msg) = first_error {
        return Err(DecompressionError::Custom(format!("Extraction interrompue ({msg})")));
    }
    if rejected > 0 {
        return Err(DecompressionError::Custom(format!(
            "{rejected} entrée(s) dangereuse(s) ignorée(s) (chemin hors du dossier de destination)"
        )));
    }
    Ok(())
}
