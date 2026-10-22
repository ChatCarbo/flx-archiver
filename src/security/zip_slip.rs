use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum SecurityError {
#[error("Tentative de traversée de répertoire détectée (Zip Slip) : {0}")]
    InvalidPath(String),
}
}

/// Valide qu'un chemin issu d'une archive s'extrait strictement à l'intérieur du dossier cible.
/// Empêche la création de fichiers en dehors du répertoire de destination via des sépateurs `..` ou des chemins absolus.
#[allow(dead_code)]
pub fn validate_archive_path(entry_name: &str, dest_path: &Path) -> Result<PathBuf, SecurityError> {
    let target = dest_path.join(entry_name);
    if target.starts_with(dest_path) {
        Ok(target)
    } else {
        Err(SecurityError::InvalidPath(entry_name.to_string()))
    }
}

    // Analyse chaque composant du chemin
    for component in path.components() {
        match component {
            Component::ParentDir => {
                // Interdiction stricte des remontées de répertoire ("..")
                return Err(SecurityError::ZipSlipAttempt(entry_path.to_string()));
            }
            Component::RootDir | Component::Prefix(_) => {
                // Interdiction des chemins absolus (ex: C:\Windows ou /usr/bin)
                return Err(SecurityError::ZipSlipAttempt(entry_path.to_string()));
            }
            Component::Normal(_) | Component::CurDir => {}
        }
    }

    // Construction du chemin de destination final
    let full_path = target_dir.join(path);

    // Vérification de sécurité complémentaire
    if !full_path.starts_with(target_dir) {
        return Err(SecurityError::ZipSlipAttempt(entry_path.to_string()));
    }

    Ok(full_path)
}