//! Symlink-safe output files shared by the `sorafs_fetch` and `sorafs_manifest_builder` binaries.
//!
//! Each binary includes this file with `#[path]`; `autobins = false` keeps it from becoming a
//! target of its own, and the library API stays unchanged.

#[cfg(not(windows))]
use std::{
    fs::{self, File},
    path::Path,
};

#[cfg(not(windows))]
use super::output_path::{ensure_parent_dir, validate_output_path};
#[cfg(not(windows))]
use sorafs_car::set_no_follow_flag;

/// Open `path` for truncating writes after symlink-safe path checks.
#[cfg(not(windows))]
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

/// Retain a native mutable output after admitting its original before truncation.
#[cfg(windows)]
pub(crate) fn open_output_file(
    path: &std::path::Path,
    label: &str,
) -> Result<iroha_fs::RetainedOutputFile, String> {
    let name = path
        .file_name()
        .ok_or_else(|| format!("output {path:?} has no file name"))?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let parent = iroha_fs::OwnerDirectory::open_or_create(parent)
        .map_err(|error| format!("failed to admit {label} parent {parent:?}: {error}"))?;
    parent
        .open_truncating_output(name)
        .map_err(|error| format!("failed to open {label} {path:?}: {error}"))
}

#[cfg(all(test, windows))]
#[path = "output_fs_windows_tests.rs"]
mod windows_tests;
