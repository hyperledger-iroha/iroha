//! Output-directory binding shared by the SoraFS fixture generator binaries.
//!
//! `generate_pdp_fixtures` and `generate_por_fixtures` include this file with
//! `#[path]`; it is not a separate binary target (`autobins = false`).

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::{
    env,
    error::Error,
    fs::{self, File},
    path::{Path, PathBuf},
};

/// One existing output directory whose lexical path, open handle, and canonical
/// path stay bound together for the whole generator run.
pub(crate) struct BoundDirectory {
    pub(crate) display_path: PathBuf,
    pub(crate) canonical_path: PathBuf,
    pub(crate) handle: File,
}

impl BoundDirectory {
    /// Bind an existing non-symlink directory reached only through real directories.
    pub(crate) fn bind(path: &Path, label: &str) -> Result<Self, Box<dyn Error>> {
        require_real_directory_ancestry(path, label)?;
        let before = fs::symlink_metadata(path)
            .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(format!("{label} must be an existing non-symlink directory").into());
        }
        let handle = File::open(path)
            .map_err(|error| format!("failed to open {label} `{}`: {error}", path.display()))?;
        let opened = handle.metadata().map_err(|error| {
            format!(
                "failed to inspect opened {label} `{}`: {error}",
                path.display()
            )
        })?;
        let after = fs::symlink_metadata(path).map_err(|error| {
            format!(
                "failed to reinspect {label} `{}` after opening: {error}",
                path.display()
            )
        })?;
        require_real_directory_ancestry(path, label)?;
        if after.file_type().is_symlink()
            || !after.is_dir()
            || !same_directory_identity(&before, &opened)
            || !same_directory_identity(&before, &after)
        {
            return Err(format!("{label} changed identity while it was bound").into());
        }
        let canonical_path = fs::canonicalize(path).map_err(|error| {
            format!(
                "failed to canonicalize {label} `{}`: {error}",
                path.display()
            )
        })?;
        Ok(Self {
            display_path: path.to_path_buf(),
            canonical_path,
            handle,
        })
    }

    /// Re-check that the bound path still names the same real directory.
    pub(crate) fn verify(&self, label: &str) -> Result<(), Box<dyn Error>> {
        require_real_directory_ancestry(&self.display_path, label)?;
        let lexical = fs::symlink_metadata(&self.display_path).map_err(|error| {
            format!(
                "failed to reinspect {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        let opened = self.handle.metadata().map_err(|error| {
            format!(
                "failed to reinspect bound {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        let canonical = fs::canonicalize(&self.display_path).map_err(|error| {
            format!(
                "failed to recanonicalize {label} `{}`: {error}",
                self.display_path.display()
            )
        })?;
        if lexical.file_type().is_symlink()
            || !lexical.is_dir()
            || !same_directory_identity(&lexical, &opened)
            || canonical != self.canonical_path
        {
            return Err(format!("{label} changed identity during fixture generation").into());
        }
        Ok(())
    }
}

/// Require every ancestor of `path` (after absolutizing it) to be a real directory.
pub(crate) fn require_real_directory_ancestry(
    path: &Path,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("failed to resolve current directory: {error}"))?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component);
        let metadata = fs::symlink_metadata(&current).map_err(|error| {
            format!(
                "failed to inspect {label} ancestry `{}`: {error}",
                current.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "{label} ancestry must not contain a symbolic link: {}",
                current.display()
            )
            .into());
        }
        if !metadata.is_dir() {
            return Err(format!(
                "{label} ancestry must contain directories only: {}",
                current.display()
            )
            .into());
        }
    }
    Ok(())
}

/// Whether two metadata snapshots describe the same directory.
#[cfg(unix)]
pub(crate) fn same_directory_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_dir() && right.is_dir() && left.dev() == right.dev() && left.ino() == right.ino()
}

/// Whether two metadata snapshots describe the same directory.
#[cfg(not(unix))]
pub(crate) fn same_directory_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_dir()
        && right.is_dir()
        && left.created().ok() == right.created().ok()
        && left.modified().ok() == right.modified().ok()
}
