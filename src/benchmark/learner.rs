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

/// Nombre minimal de mesures d'une catégorie avant de s'appuyer sur l'historique.
const MIN_RECORDS: usize = 3;

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
        // On ignore les mesures sans valeur : compressé == original provient d'une ancienne version
        // qui enregistrait la taille d'origine comme taille compressée.
        let matching_records: Vec<&BenchmarkRecord> = self
            .history
            .records
            .iter()
            .filter(|r| {
                r.category == category
                    && r.original_size_bytes > 0
                    && r.compressed_size_bytes > 0
                    && r.compressed_size_bytes != r.original_size_bytes
            })
            .collect();

        // Pas assez de mesures : les règles par défaut sont plus fiables qu'un seul essai.
        if matching_records.len() < MIN_RECORDS {
            return self.default_recommendation(category);
        }

        // Moyenne par couple (format, niveau), pour qu'une mesure isolée ne décide pas seule.
        let mut groups: Vec<(String, String, Vec<&BenchmarkRecord>)> = Vec::new();
        for record in matching_records {
            match groups
                .iter_mut()
                .find(|(f, l, _)| *f == record.format && *l == record.level)
            {
                Some((_, _, list)) => list.push(record),
                None => groups.push((record.format.clone(), record.level.clone(), vec![record])),
            }
        }

        let mut best: Option<(f64, String, String, f64, f64)> = None;
        for (format, level, list) in &groups {
            let n = list.len() as f64;
            let ratio = list.iter().map(|r| r.ratio_percent).sum::<f64>() / n;
            let speed = list.iter().map(|r| r.speed_mb_s).sum::<f64>() / n;
            // La vitesse est plafonnée à 200 MB/s pour ne pas écraser le critère de taille.
            let score = (100.0 - ratio).max(0.0) * 0.6 + (speed.min(200.0) / 200.0 * 100.0) * 0.4;
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, format.clone(), level.clone(), ratio, speed));
            }
        }

        let (_, format, level, ratio, speed) = best.expect("au moins un groupe");
        Recommendation {
            format: parse_format(&format),
            level: parse_level(&level),
            reason: format!(
                "Recommendation based on history ({:.1}% ratio, {:.1} MB/s)",
                ratio, speed
            ),
        }
    }

    fn default_recommendation(&self, category: FileCategory) -> Recommendation {
        match category {
            FileCategory::MediaCompressed | FileCategory::Archive => Recommendation {
                format: ArchiveFormat::Zip,
                level: CompressionLevel::Fast,
                reason: "Already compressed files: fast compression recommended".to_string(),
            },
            FileCategory::Text | FileCategory::TextOrCode | FileCategory::Code => Recommendation {
                format: ArchiveFormat::Zstd,
                level: CompressionLevel::Maximum,
                reason: "Text/code files: excellent compressibility with Zstd".to_string(),
            },
            FileCategory::BinaryExec | FileCategory::Binary | FileCategory::Mixed | FileCategory::Media => Recommendation {
                format: ArchiveFormat::Zstd,
                level: CompressionLevel::Default,
                reason: "Binary/mixed files: good speed/ratio trade-off".to_string(),
            },
            _ => Recommendation {
                format: ArchiveFormat::Zip,
                level: CompressionLevel::Default,
                reason: "Standard profile".to_string(),
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