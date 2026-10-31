//! Windows provenance transactions through retained native directory, lock and file owners.
//!
//! The persistent lock serializes cooperating writers. It does not exclude a malicious same-user
//! writer that ignores the lock; native custody and per-read snapshots do not make a tree atomic.

use super::*;
use iroha_fs::{AtomicWriteStep, OwnerDirectory, PublishMode};
use std::{ffi::OsString, fs::TryLockError, io::Read};

pub(super) fn transact<T>(
    path: &Path,
    log_id: [u8; 16],
    write: bool,
    operation: impl FnOnce(&mut ModerationProvenanceLogV1) -> Result<T, ModerationProvenanceStoreError>,
) -> Result<T, ModerationProvenanceStoreError> {
    transact_observed(path, log_id, write, operation, |_, _| Ok(()))
}

fn transact_observed<T>(
    path: &Path,
    log_id: [u8; 16],
    write: bool,
    operation: impl FnOnce(&mut ModerationProvenanceLogV1) -> Result<T, ModerationProvenanceStoreError>,
    mut observe: impl FnMut(AtomicWriteStep, &Path) -> io::Result<()>,
) -> Result<T, ModerationProvenanceStoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| unsafe_path(path, "store path has no file name"))?;
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Native owner admission precedes every namespace probe; it never repairs permissions.
    let parent = OwnerDirectory::open(parent_path).map_err(|error| io_error(parent_path, error))?;
    let mut lock_name = OsString::from(".");
    lock_name.push(name);
    lock_name.push(".lock");
    let lock = parent
        .open_lock(&lock_name)
        .map_err(|error| io_error(path, error))?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => {
            return Err(ModerationProvenanceStoreError::Locked(path.to_path_buf()));
        }
        Err(TryLockError::Error(error)) => return Err(io_error(path, error)),
    }
    // The lock is a persistent private file. Closing this exact descriptor releases
    // exclusion; unlinking its name would allow two writers to lock different objects.
    let result = (|| {
        parent
            .require_same_lock(&lock_name, &lock)
            .map_err(|error| io_error(path, error))?;
        let (mut log, created) =
            match read_original(&parent, path, |bytes| decode_log(path, bytes))? {
                Some(log) => (log, false),
                None => (ModerationProvenanceLogV1::new(log_id)?, true),
            };
        if log.log_id != log_id {
            return Err(ModerationProvenanceStoreError::LogIdMismatch);
        }
        log.validate_chain()?;
        let output = operation(&mut log)?;
        parent
            .require_same_lock(&lock_name, &lock)
            .map_err(|error| io_error(path, error))?;
        if created || write {
            log.validate_chain()?;
            let bytes = norito::to_bytes(&log)
                .map_err(|error| ModerationProvenanceStoreError::Encode(error.to_string()))?;
            if bytes.is_empty()
                || u64::try_from(bytes.len())
                    .ok()
                    .is_none_or(|length| length > MAX_PROVENANCE_FILE_BYTES)
            {
                return Err(ModerationProvenanceStoreError::InvalidFile {
                    path: path.to_path_buf(),
                    reason: "encoded segment exceeds the durable file bound".to_string(),
                });
            }
            let mode = if created {
                PublishMode::CreateNew
            } else {
                PublishMode::Replace
            };
            parent
                .write_atomic_observed(
                    name,
                    &bytes,
                    mode,
                    u32::try_from(TEMPFILE_ATTEMPTS).expect("64 temporary attempts fit u32"),
                    |step, current| {
                        parent.require_same_lock(&lock_name, &lock)?;
                        observe(step, current)?;
                        parent.require_same_lock(&lock_name, &lock)
                    },
                )
                .map_err(|error| io_error(path, error))?;
            // The atomic writer has closed its original before this bounded readback.
            // Any readback or sync refusal remains an error, even after publication.
            let readback = read_original(&parent, path, |actual| {
                if actual != bytes.as_slice() {
                    return Err(ModerationProvenanceStoreError::InvalidFile {
                        path: path.to_path_buf(),
                        reason: "published provenance differs from its complete canonical bytes"
                            .to_string(),
                    });
                }
                Ok(())
            })?;
            if readback.is_none() {
                return Err(ModerationProvenanceStoreError::InvalidFile {
                    path: path.to_path_buf(),
                    reason: "published provenance disappeared before readback".to_string(),
                });
            }
        }
        Ok(output)
    })();
    // Ordinary failures still close through original custody validation, but a later
    // observation error never replaces the first operation/publication error.
    let final_custody = parent
        .require_same_lock(&lock_name, &lock)
        .map_err(|error| io_error(path, error));
    match result {
        Err(first) => Err(first),
        Ok(output) => final_custody.map(|()| output),
    }
}

fn read_original<T>(
    parent: &OwnerDirectory,
    path: &Path,
    inspect: impl FnOnce(&[u8]) -> Result<T, ModerationProvenanceStoreError>,
) -> Result<Option<T>, ModerationProvenanceStoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| unsafe_path(path, "store path has no file name"))?;
    let Some(mut original) = parent
        .open_retained_regular_optional(name)
        .map_err(|error| io_error(path, error))?
    else {
        return Ok(None);
    };
    let result = (|| {
        let before = original.snapshot().map_err(|error| io_error(path, error))?;
        let length = original
            .file()
            .metadata()
            .map_err(|error| io_error(path, error))?
            .len();
        if length == 0 || length > MAX_PROVENANCE_FILE_BYTES {
            return Err(ModerationProvenanceStoreError::InvalidFile {
                path: path.to_path_buf(),
                reason: format!("file size {length} is outside 1..={MAX_PROVENANCE_FILE_BYTES}"),
            });
        }
        let length =
            usize::try_from(length).map_err(|_| ModerationProvenanceStoreError::InvalidFile {
                path: path.to_path_buf(),
                reason: "file length does not fit usize".to_string(),
            })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|error| {
            ModerationProvenanceStoreError::InvalidFile {
                path: path.to_path_buf(),
                reason: format!("bounded allocation failed: {error}"),
            }
        })?;
        bytes.resize(length, 0);
        original
            .file_mut()
            .read_exact(&mut bytes)
            .map_err(|error| io_error(path, error))?;
        let mut trailing = [0_u8; 1];
        if original
            .file_mut()
            .read(&mut trailing)
            .map_err(|error| io_error(path, error))?
            != 0
            || original.snapshot().map_err(|error| io_error(path, error))? != before
        {
            return Err(ModerationProvenanceStoreError::InvalidFile {
                path: path.to_path_buf(),
                reason: "file changed while being read".to_string(),
            });
        }
        // Canonical decoding/readback runs while this exact reader and ancestry remain live.
        let output = inspect(&bytes)?;
        if original.snapshot().map_err(|error| io_error(path, error))? != before {
            return Err(ModerationProvenanceStoreError::InvalidFile {
                path: path.to_path_buf(),
                reason: "file changed during canonical inspection".to_string(),
            });
        }
        Ok(Some(output))
    })();
    let final_custody = original.revalidate().map_err(|error| io_error(path, error));
    match result {
        Err(first) => Err(first),
        Ok(output) => final_custody.map(|()| output),
    }
}

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
