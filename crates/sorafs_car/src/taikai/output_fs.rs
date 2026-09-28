//! Symlink-safe output-path checks shared by [`crate::taikai`] and the `taikai_car` binary.
//!
//! The library declares this file as `taikai::output_fs`; `src/bin/taikai_car.rs` includes the
//! same file with `#[path]`, so the binary reuses it without widening the library API.

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{fs, io, path::Path};

use eyre::{Result, WrapErr, eyre};

/// Create the missing parent directories of `path`.
pub(super) fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("failed to create output parent `{}`", parent.display()))?;
    }
    Ok(())
}
/// Reject symlinked or non-regular outputs and symlinked or non-directory ancestors.
pub(super) fn validate_output_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(eyre!("output `{}` must not be a symlink", path.display()));
            }
            if !metadata.is_file() {
                return Err(eyre!("output `{}` must be a regular file", path.display()));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(eyre!(
                "failed to inspect output `{}`: {err}",
                path.display()
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(eyre!(
                            "output parent `{}` must not be a symlink",
                            ancestor.display()
                        ));
                    }
                    if !metadata.is_dir() {
                        return Err(eyre!(
                            "output parent `{}` must be a directory",
                            ancestor.display()
                        ));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(eyre!(
                        "failed to inspect output parent `{}`: {err}",
                        ancestor.display()
                    ));
                }
            }
        }
    }
    Ok(())
}
/// [`validate_output_path`] plus a no-follow writability probe of an existing output.
pub(super) fn validate_output_writable(path: &Path) -> Result<()> {
    validate_output_path(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(eyre!(
                "failed to inspect output `{}`: {err}",
                path.display()
            ));
        }
    };
    if metadata.permissions().readonly() {
        return Err(eyre!("output `{}` must be writable", path.display()));
    }
    let mut options = fs::OpenOptions::new();
    options.write(true);
    set_no_follow_flag(&mut options);
    options
        .open(path)
        .wrap_err_with(|| format!("output `{}` must be writable", path.display()))?;
    Ok(())
}
/// Refuse to follow a final-component symlink when opening an output.
#[cfg(unix)]
pub(super) fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
/// Refuse to follow a final-component symlink when opening an output.
#[cfg(not(unix))]
pub(super) fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}
/// Platform `O_NOFOLLOW` bit (zero where no such flag exists).
#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    rustix::fs::OFlags::NOFOLLOW.bits() as i32
}
/// Platform `O_NOFOLLOW` bit (zero where no such flag exists).
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
/// Platform `O_NOFOLLOW` bit (zero where no such flag exists).
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}
