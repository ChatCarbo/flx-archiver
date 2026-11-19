use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub theme: String,
    pub max_threads: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            max_threads: num_cpus::get_physical(),
        }
    }
}

/// Emplacement de l'historique des compressions : dossier de données de l'utilisateur
/// (et non le dossier courant, qui change selon la façon de lancer l'application).
pub fn history_path() -> PathBuf {
    if let Some(base) = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME")) {
        return PathBuf::from(base).join("FLX Archiver").join("compression_history.json");
    }
    PathBuf::from("compression_history.json")
}
