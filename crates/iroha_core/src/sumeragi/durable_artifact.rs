//! Exact-byte, no-clobber artifact publication. Authentication belongs to the caller.
//!
//! The caller retains its prepared original-funded bytes through every error and retry.
//! This module borrows those bytes and never clones, decodes, or re-signs their contents.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub use super::availability_schedule::AvailabilitySchedule;
pub use super::body_read::{BodyReadError, BodyReadJob, BodyReadPoll, BodyReader};

use crate::sumeragi::records::{Faults, FsStep, create_dir_durable, sync_dir};

const COMPARE_BYTES: usize = 8192;
const TEMP_ATTEMPTS: u64 = 16;
static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TemporaryFile(PathBuf);

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        // A leftover .tmp never represents committed custody; reopening removes it.
        let _ = fs::remove_file(&self.0);
    }
}

fn conflict(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "artifact differs from original prepared bytes: {}",
            path.display()
        ),
    )
}

/// Compare every original byte, including the certificate, without a file-sized allocation.
/// A successful retry re-establishes both file and directory durability before reporting it.
fn open_exact(target: &Path, bytes: &[u8]) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
        options.custom_flags(i32::from_ne_bytes(flags.bits().to_ne_bytes()));
    }
    #[cfg(not(unix))]
    if !fs::symlink_metadata(target)?.file_type().is_file() {
        return Err(conflict(target));
    }
    let mut file = options.open(target)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != bytes.len() as u64 {
        return Err(conflict(target));
    }
    let mut scratch = [0; COMPARE_BYTES];
    for expected in bytes.chunks(COMPARE_BYTES) {
        file.read_exact(&mut scratch[..expected.len()])?;
        if scratch[..expected.len()] != *expected {
            return Err(conflict(target));
        }
    }
    if file.read(&mut scratch[..1])? != 0 {
        return Err(conflict(target));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let current = fs::symlink_metadata(target)?;
        if !current.file_type().is_file()
            || current.dev() != metadata.dev()
            || current.ino() != metadata.ino()
        {
            return Err(conflict(target));
        }
    }
    Ok(file)
}

/// Compare a complete existing artifact with the exact prepared bytes using bounded scratch.
/// This proves byte equality only; the caller must authenticate the original artifact.
pub(super) fn check_exact(target: &Path, bytes: &[u8]) -> io::Result<()> {
    open_exact(target, bytes).map(drop)
}

/// Establish a store directory and repair all ancestor durability after an interrupted open.
///
/// An earlier attempt may have created any ancestor before its parent fsync failed. Therefore
/// an existing directory is not proof that its entire path is durable. This runs at store open,
/// not on each artifact write, and also resolves relative paths through the filesystem root.
pub(super) fn establish_dir(faults: &dyn Faults, dir: &Path) -> io::Result<()> {
    let absolute = std::path::absolute(dir)?;
    create_dir_durable(faults, &absolute)?;
    for ancestor in absolute.ancestors() {
        sync_dir(faults, ancestor)?;
    }
    Ok(())
}

fn sync_publication_path(faults: &dyn Faults, dir: &Path) -> io::Result<()> {
    sync_dir(faults, dir)?;
    // A failed create_dir_durable may leave dir visible without its parent's entry durable.
    // The store root and its older ancestors are established by the store initializer.
    if let Some(parent) = dir.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        sync_dir(faults, parent)?;
    }
    Ok(())
}

fn verify_existing(faults: &dyn Faults, dir: &Path, target: &Path, bytes: &[u8]) -> io::Result<()> {
    let file = open_exact(target, bytes)?;
    faults.before(FsStep::SyncFile, target)?;
    file.sync_all()?;
    sync_publication_path(faults, dir)
}

fn create_temporary(
    faults: &dyn Faults,
    dir: &Path,
    name: &str,
) -> io::Result<(TemporaryFile, File)> {
    for _ in 0..TEMP_ATTEMPTS {
        let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("{name}.{}.{serial}.tmp", std::process::id()));
        faults.before(FsStep::CreateTemp, &path)?;
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((TemporaryFile(path), file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "artifact temporary-file namespace is busy",
    ))
}

/// Publish exactly the borrowed original frame; an existing different frame is never replaced.
///
/// Filesystem refusal never consumes or mutates the caller's prepared owner. After a visible
/// publication followed by fsync refusal, retry verifies the complete retained bytes and
/// completes fsync. Callers may advance the durable tip only after this returns success.
pub(super) fn publish(faults: &dyn Faults, dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "artifact name must be one path component",
        ));
    }
    let target = dir.join(name);
    match verify_existing(faults, dir, &target, bytes) {
        Ok(()) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let (temporary, mut file) = create_temporary(faults, dir, name)?;
    let (head, tail) = bytes.split_at(bytes.len() / 2);
    faults.before(FsStep::WriteTemp, &temporary.0)?;
    file.write_all(head)?;
    faults.before(FsStep::WriteTempRest, &temporary.0)?;
    file.write_all(tail)?;
    faults.before(FsStep::SyncFile, &temporary.0)?;
    file.sync_all()?;
    drop(file);
    // The existing fault-injection publication boundary is named Rename. Unlike replacement
    // rename, a hard link atomically refuses an already-present target, including a racing writer.
    faults.before(FsStep::Rename, &target)?;
    match fs::hard_link(&temporary.0, &target) {
        Ok(()) => {
            fs::remove_file(&temporary.0)?;
            sync_publication_path(faults, dir)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            verify_existing(faults, dir, &target, bytes)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "durable_artifact/tests.rs"]
mod tests;
