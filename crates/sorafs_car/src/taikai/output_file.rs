//! Symlink-safe regular-file output shared by the Taikai tests and the `taikai_car` tool.
use super::{ensure_parent_dir, set_no_follow_flag, validate_output_writable};
use eyre::{Result, WrapErr, eyre};
use std::{fs, io::Write, path::Path};

pub(super) fn write_output_bytes(path: &Path, label: &str, bytes: &[u8]) -> Result<()> {
    let mut file = open_output_file(path, label)?;
    file.write_all(bytes)
        .wrap_err_with(|| format!("failed to write {label} `{}`", path.display()))
}
pub(super) fn open_output_file(path: &Path, label: &str) -> Result<fs::File> {
    validate_output_writable(path)?;
    ensure_parent_dir(path)?;
    validate_output_writable(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .wrap_err_with(|| format!("failed to open {label} `{}`", path.display()))?;
    let metadata = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect {label} `{}` after open", path.display()))?;
    if !metadata.is_file() {
        return Err(eyre!(
            "failed to write {label} `{}`: output must be a regular file",
            path.display()
        ));
    }
    Ok(file)
}
