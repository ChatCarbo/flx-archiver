use std::path::PathBuf;
use crate::benchmark::data::FileCategory;

#[derive(Debug, Clone)]
pub struct ContentProfile {
    pub total_size: u64,
    #[allow(dead_code)]
    pub file_count: usize,
    pub category: FileCategory,
}

pub struct ContentAnalyzer;

impl ContentAnalyzer {
    pub fn profile(paths: &[PathBuf]) -> ContentProfile {
        let mut total_size = 0u64;
        let mut file_count = 0usize;

        for path in paths {
            if path.is_file() {
                if let Ok(metadata) = std::fs::metadata(path) {
                    total_size += metadata.len();
                    file_count += 1;
                }
            } else if path.is_dir() {
                for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
                        file_count += 1;
                    }
                }
            }
        }

        ContentProfile {
            total_size,
            file_count,
            category: FileCategory::Text,
        }
    }

    pub fn analyze(paths: &[PathBuf]) -> ContentProfile {
        Self::profile(paths)
    }
}