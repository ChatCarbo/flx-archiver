use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::benchmark::data::{BenchmarkHistory, BenchmarkRecord, FileCategory};
use crate::benchmark::ContentProfile;
use crate::compression::{ArchiveFormat, CompressionLevel};

pub trait IntoCategory {
    fn into_category(&self) -> FileCategory;
}

impl IntoCategory for FileCategory {
    fn into_category(&self) -> FileCategory {
        *self
    }
}

impl IntoCategory for &FileCategory {
    fn into_category(&self) -> FileCategory {
        **self
    }
}

impl IntoCategory for ContentProfile {
    fn into_category(&self) -> FileCategory {
        self.category
    }
}

impl IntoCategory for &ContentProfile {
    fn into_category(&self) -> FileCategory {
        self.category
    }
}

#[derive(Debug, Clone)]
pub struct Recommendation {
    pub format: ArchiveFormat,
    pub level: CompressionLevel,
    pub reason: String,
}

pub struct LearningEngine {
    history: BenchmarkHistory,
    history_path: PathBuf,
}

impl LearningEngine {
    pub fn new<P: AsRef<Path>>(history_path: P) -> Self {
        let path_buf = history_path.as_ref().to_path_buf();
        let history = BenchmarkHistory::load_or_create(&path_buf);
        Self {
            history,
            history_path: path_buf,
        }
    }

    pub fn recommend<C: IntoCategory>(&self, target: C) -> Recommendation {
        let category = target.into_category();
        let matching_records: Vec<&BenchmarkRecord> = self
            .history
            .records
            .iter()
            .filter(|r| r.category == category)
            .collect();

        if matching_records.is_empty() {
            return self.default_recommendation(category);
        }

        let mut best_record = matching_records[0];
        let mut best_score = -1.0f64;

        for record in matching_records {
            let score = (100.0 - record.ratio_percent) * 0.6 + (record.speed_mb_s * 0.4);
            if score > best_score {
                best_score = score;
                best_record = record;
            }
        }

        Recommendation {
            format: parse_format(&best_record.format),
            level: parse_level(&best_record.level),
            reason: format!(
                "Recommandation basée sur l'historique ({:.1}% ratio, {:.1} MB/s)",
                best_record.ratio_percent, best_record.speed_mb_s
            ),
        }
    }

    fn default_recommendation(&self, category: FileCategory) -> Recommendation {
        match category {
            FileCategory::MediaCompressed | FileCategory::Archive => Recommendation {
                format: ArchiveFormat::Zip,
                level: CompressionLevel::Fast,
                reason: "Fichiers déjà compressés : compression rapide préconisée".to_string(),
            },
            FileCategory::Text | FileCategory::TextOrCode | FileCategory::Code => Recommendation {
                format: ArchiveFormat::Zstd,
                level: CompressionLevel::Maximum,
                reason: "Fichiers texte/code : excellente compressibilité avec Zstd".to_string(),
            },
            FileCategory::BinaryExec | FileCategory::Binary | FileCategory::Mixed => Recommendation {
                format: ArchiveFormat::Zstd,
                level: CompressionLevel::Default,
                reason: "Fichiers binaires/mixtes : bon compromis vitesse/ratio".to_string(),
            },
            _ => Recommendation {
                format: ArchiveFormat::Zip,
                level: CompressionLevel::Default,
                reason: "Profil standard".to_string(),
            },
        }
    }

    pub fn record_result<C: IntoCategory>(
        &mut self,
        target: C,
        format: ArchiveFormat,
        level: CompressionLevel,
        original_size: u64,
        compressed_size: u64,
        duration_ms: u64,
    ) {
        let category = target.into_category();
        let ratio_percent = if original_size > 0 {
            (compressed_size as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };
        let duration_sec = duration_ms as f64 / 1000.0;
        let speed_mb_s = if duration_sec > 0.0 {
            (original_size as f64 / (1024.0 * 1024.0)) / duration_sec
        } else {
            0.0
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let fmt_str = format!("{:?}", format);
        let lvl_str = format!("{:?}", level);

        let record = BenchmarkRecord {
            timestamp: now,
            category,
            original_size_bytes: original_size,
            compressed_size_bytes: compressed_size,
            duration_ms,
            format: fmt_str.clone(),
            level: lvl_str,
            ratio_percent,
            speed_mb_s,
            algorithm: fmt_str,
        };

        self.history.add_record(record);
        let _ = self.history.save(&self.history_path);
    }
}

fn parse_format(s: &str) -> ArchiveFormat {
    let lower = s.to_lowercase();
    if lower.contains("zstd") || lower.contains("zst") {
        ArchiveFormat::Zstd
    } else if lower.contains("sevenz") || lower.contains("7z") || lower.contains("seven") {
        ArchiveFormat::SevenZ
    } else if lower.contains("gzip") || lower.contains("gz") {
        ArchiveFormat::Gzip
    } else {
        ArchiveFormat::Zip
    }
}

fn parse_level(s: &str) -> CompressionLevel {
    let lower = s.to_lowercase();
    if lower.contains("fast") {
        CompressionLevel::Fast
    } else if lower.contains("max") || lower.contains("high") {
        CompressionLevel::Maximum
    } else {
        CompressionLevel::Default
    }
}