//! Protection contre le « Zip Slip » : une archive piégée ne doit jamais pouvoir écrire hors du dossier de destination.

use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Chemin dangereux refusé (Zip Slip) : {0}")]
    UnsafePath(String),
}

/// Joint `entry` à `dest` en refusant les chemins absolus, les lecteurs (`C:`), les remontées (`..`)
/// et, sous Windows, les flux de données alternatifs (`fichier:flux`).
pub fn safe_join(dest: &Path, entry: &Path) -> Result<PathBuf, SecurityError> {
    let mut out = dest.to_path_buf();
    for component in entry.components() {
        match component {
            Component::Normal(part) => {
                #[cfg(windows)]
                {
                    if part.to_string_lossy().contains(':') {
                        return Err(SecurityError::UnsafePath(entry.display().to_string()));
                    }
                }
                out.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SecurityError::UnsafePath(entry.display().to_string()));
            }
        }
    }
    if out == dest {
        return Err(SecurityError::UnsafePath(entry.display().to_string()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_paths() {
        let dest = Path::new("out");
        assert_eq!(safe_join(dest, Path::new("a/b.txt")).unwrap(), Path::new("out/a/b.txt"));
        assert_eq!(safe_join(dest, Path::new("./a.txt")).unwrap(), Path::new("out/a.txt"));
    }

    #[test]
    fn rejects_traversal_and_absolute() {
        let dest = Path::new("out");
        assert!(safe_join(dest, Path::new("../evil.txt")).is_err());
        assert!(safe_join(dest, Path::new("a/../../evil.txt")).is_err());
        assert!(safe_join(dest, Path::new("/etc/passwd")).is_err());
        assert!(safe_join(dest, Path::new("")).is_err());
    }
}
