use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileCategory {
    Text,
    Binary,
    Media,
    Archive,
    Code,
    Mixed,
    Unknown,
    MediaCompressed,
    TextOrCode,
    BinaryExec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRecord {
    pub timestamp: u64,
    pub category: FileCategory,
    pub original_size_bytes: u64,
    pub compressed_size_bytes: u64,
    pub duration_ms: u64,
    pub format: String,
    pub level: String,
    pub ratio_percent: f64,
    pub speed_mb_s: f64,
    pub algorithm: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BenchmarkHistory {
    pub records: Vec<BenchmarkRecord>,
}

impl BenchmarkHistory {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(history) = serde_json::from_str(&content) {
                return history;
            }
        }
        BenchmarkHistory::default()
    }

    pub fn add_record(&mut self, record: BenchmarkRecord) {
        self.records.push(record);
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}