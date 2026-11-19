use std::path::{Path, PathBuf};

use crate::benchmark::data::FileCategory;

#[derive(Debug, Clone)]
pub struct ContentProfile {
    pub total_size: u64,
    #[allow(dead_code)]
    pub file_count: usize,
    pub category: FileCategory,
}

pub struct ContentAnalyzer;

/// Catégorie d'un fichier d'après son extension.
fn category_of(path: &Path) -> FileCategory {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        // Médias déjà compressés
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "heic" | "avif" | "mp3" | "aac" | "m4a" | "ogg" | "opus"
        | "flac" | "mp4" | "mkv" | "avi" | "mov" | "webm" | "wmv" => FileCategory::MediaCompressed,
        // Médias non compressés
        "wav" | "bmp" | "tif" | "tiff" | "raw" | "aiff" => FileCategory::Media,
        // Archives et formats qui sont eux-mêmes des archives zip
        "zip" | "7z" | "rar" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "cab" | "jar" | "apk" | "docx" | "xlsx"
        | "pptx" | "odt" | "ods" | "epub" => FileCategory::Archive,
        // Texte et code
        "txt" | "md" | "csv" | "log" | "ini" | "cfg" | "tsv" | "rtf" => FileCategory::Text,
        "rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "c" | "h" | "cpp" | "hpp" | "cs" | "java" | "go" | "php"
        | "rb" | "html" | "htm" | "css" | "json" | "xml" | "yaml" | "yml" | "toml" | "sh" | "bat" | "ps1"
        | "sql" => FileCategory::Code,
        // Binaires exécutables
        "exe" | "dll" | "so" | "dylib" | "sys" | "msi" | "o" | "obj" | "lib" => FileCategory::BinaryExec,
        _ => FileCategory::Binary,
    }
}

impl ContentAnalyzer {
    pub fn profile(paths: &[PathBuf]) -> ContentProfile {
        let mut total_size = 0u64;
        let mut file_count = 0usize;
        let mut by_category: Vec<(FileCategory, u64)> = Vec::new();

        let mut add = |path: &Path, size: u64| {
            total_size += size;
            file_count += 1;
            let cat = category_of(path);
            match by_category.iter_mut().find(|(c, _)| *c == cat) {
                Some((_, bytes)) => *bytes += size,
                None => by_category.push((cat, size)),
            }
        };

        for path in paths {
            if path.is_file() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    add(path, metadata.len());
                }
            } else if path.is_dir() {
                for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        add(entry.path(), entry.metadata().map(|m| m.len()).unwrap_or(0));
                    }
                }
            }
        }

        // Catégorie dominante (en octets) si elle représente au moins 60 % du contenu, sinon « mixte ».
        let category = match by_category.iter().max_by_key(|(_, bytes)| *bytes) {
            None => FileCategory::Unknown,
            Some((cat, bytes)) => {
                if total_size == 0 || (*bytes as f64) / (total_size as f64) >= 0.6 || by_category.len() == 1 {
                    *cat
                } else {
                    FileCategory::Mixed
                }
            }
        };

        ContentProfile {
            total_size,
            file_count,
            category,
        }
    }
}
