//! Pathname checks retained for non-Windows direct outputs and the separate fetch spool.

use std::{fs, io, path::Path};

/// Create the missing parent directories of `path`.
pub(crate) fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    Ok(())
}
/// Reject symlinked or directory outputs and symlinked or non-directory ancestors.
pub(crate) fn validate_output_path(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!("output {path:?} must not be a symlink"));
            }
            if metadata.is_dir() {
                return Err(format!("output {path:?} must not be a directory"));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("failed to inspect output {path:?}: {err}")),
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(format!("output parent {ancestor:?} must not be a symlink"));
                    }
                    if !metadata.is_dir() {
                        return Err(format!("output parent {ancestor:?} must be a directory"));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "failed to inspect output parent {ancestor:?}: {err}"
                    ));
                }
            }
        }
    }
    Ok(())
}
