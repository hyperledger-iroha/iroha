//! Symlink-safe output files shared by the `sorafs_fetch` and `sorafs_manifest_builder` binaries.
//!
//! Each binary includes this file with `#[path]`; `autobins = false` keeps it from becoming a
//! target of its own, and the library API stays unchanged.

use std::{
    fs::{self, File},
    io,
    path::Path,
};

use sorafs_car::set_no_follow_flag;

/// Open `path` for truncating writes after symlink-safe path checks.
pub(crate) fn open_output_file(path: &Path, label: &str) -> Result<File, String> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|err| format!("failed to open {label} {path:?}: {err}"))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect {label} {path:?} after open: {err}"))?;
    if !metadata.is_file() {
        return Err(format!(
            "failed to write {label} {path:?}: output must be a regular file"
        ));
    }
    Ok(file)
}
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
