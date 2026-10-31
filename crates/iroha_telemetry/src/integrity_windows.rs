//! Windows custody for the configured durable telemetry checkpoint.
//!
//! One original private directory and exclusive lock survive the complete exporter lifetime.
//! Pending publication originals survive errors and are retried only with the same bytes.

use std::{
    ffi::OsString,
    fs::File,
    io::{self, Seek as _, SeekFrom, Write as _},
    path::{Path, PathBuf},
    sync::Mutex,
};

use iroha_fs::{
    AtomicWriteStep, FileIdentity, FileMetadata, FileSnapshot, OwnerDirectory, PrivateDirectory,
};

use super::{ChainStateSnapshot, IntegrityError, STATE_FILE_MAX_BYTES, decode_state_snapshot};

pub(super) struct StateStore {
    // Rust drops fields in declaration order: publication originals first, then
    // the exclusive lock, and only then their retained parent authorities.
    pending: Mutex<Option<Publication>>,
    lock: File,
    supplied_path: PathBuf,
    directory: PrivateDirectory,
    owner: OwnerDirectory,
    name: OsString,
    temporary: OsString,
    lock_name: OsString,
    lock_snapshot: FileSnapshot,
}

struct Publication {
    payload: Vec<u8>,
    original: Option<File>,
    original_identity: Option<FileIdentity>,
    predecessor: Option<File>,
    predecessor_snapshot: Option<FileSnapshot>,
    predecessor_identity: Option<FileIdentity>,
    predecessor_selected: bool,
    published: bool,
}

fn finish<T>(result: Result<T, String>, final_check: Result<(), String>) -> Result<T, String> {
    result.and_then(|value| final_check.map(|()| value))
}

fn native_error(error: io::Error) -> String {
    error.to_string()
}

impl StateStore {
    pub(super) fn open(path: &Path) -> Result<Self, IntegrityError> {
        let custody = |error: io::Error| IntegrityError::StateCustody(error.to_string());
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let name = path
            .file_name()
            .ok_or_else(|| IntegrityError::StateCustody("state path has no leaf".into()))?
            .to_os_string();
        let temporary = path
            .with_extension("tmp")
            .file_name()
            .ok_or_else(|| IntegrityError::StateCustody("state temporary has no leaf".into()))?
            .to_os_string();
        let lock_name = path
            .with_extension("lock")
            .file_name()
            .ok_or_else(|| IntegrityError::StateCustody("state lock has no leaf".into()))?
            .to_os_string();
        if name == temporary || name == lock_name || temporary == lock_name {
            return Err(IntegrityError::StateCustody(
                "state, temporary and lock names must be distinct".into(),
            ));
        }
        let directory = PrivateDirectory::open_or_create(parent).map_err(custody)?;
        let owner = OwnerDirectory::open(directory.path()).map_err(custody)?;
        let result = (|| {
            if owner.identity()? != directory.identity()? {
                return Err(io::Error::other("state parent original changed"));
            }
            owner.open_exclusive_private_lock(&lock_name)
        })();
        let directory_check = directory.revalidate();
        let owner_check = owner.revalidate();
        let lock = result
            .and_then(|file| directory_check.and(owner_check).map(|()| file))
            .map_err(|error| {
                if matches!(error.raw_os_error(), Some(32 | 33)) {
                    IntegrityError::StateAlreadyInUse(path.display().to_string())
                } else {
                    custody(error)
                }
            })?;
        let lock_snapshot = FileSnapshot::of(&lock, true).map_err(custody)?;
        let store = Self {
            supplied_path: path.to_path_buf(),
            directory,
            owner,
            name,
            temporary,
            lock_name,
            lock,
            lock_snapshot,
            pending: Mutex::new(None),
        };
        store.revalidate().map_err(IntegrityError::StateCustody)?;
        Ok(store)
    }

    pub(super) fn require_path(&self, path: &Path) -> Result<(), String> {
        let result = if path == self.supplied_path {
            Ok(())
        } else {
            Err("state path differs from its retained original".into())
        };
        finish(result, self.revalidate())
    }

    fn revalidate(&self) -> Result<(), String> {
        let original_check = (|| {
            if FileSnapshot::of(&self.lock, true)? != self.lock_snapshot {
                return Err(io::Error::other("telemetry lock original changed"));
            }
            // A data reader would conflict with SHARE_NONE. This observation requests
            // metadata only; retained directory custody supplies the ancestry authority.
            let named =
                FileMetadata::symlink_metadata(self.directory.path().join(&self.lock_name))?;
            if !named.is_file()
                || named.file_type().is_symlink()
                || named.links() != 1
                || named.identity() != FileIdentity::of(&self.lock)?
                || FileSnapshot::of(&self.lock, true)? != self.lock_snapshot
            {
                return Err(io::Error::other("telemetry lock name changed"));
            }
            Ok(())
        })();
        let private_check = self.directory.revalidate();
        let owner_check = self.owner.revalidate();
        original_check
            .and(private_check)
            .and(owner_check)
            .map_err(native_error)
    }

    fn require_original(
        &self,
        name: &OsString,
        file: &File,
        identity: FileIdentity,
    ) -> Result<(), String> {
        let result = (|| {
            if FileIdentity::of(file)? != identity {
                return Err(io::Error::other("telemetry original identity changed"));
            }
            FileSnapshot::private_journal(file)?;
            self.owner.require_same_lock(name, file)
        })()
        .map_err(native_error);
        finish(result, self.revalidate())
    }

    fn read_original(&self, name: &OsString, file: &File) -> Result<Vec<u8>, String> {
        let snapshot = FileSnapshot::private_journal(file).map_err(native_error)?;
        let identity = FileIdentity::of(file).map_err(native_error)?;
        let result = (|| {
            self.require_original(name, file, identity)?;
            let length = file.metadata().map_err(native_error)?.len();
            if length > STATE_FILE_MAX_BYTES {
                return Err(format!(
                    "state file is {length} bytes, exceeding the {STATE_FILE_MAX_BYTES}-byte limit"
                ));
            }
            let mut bytes = vec![0; usize::try_from(length).map_err(|e| e.to_string())?];
            iroha_fs::read_exact_at(file, &mut bytes, 0).map_err(native_error)?;
            let mut eof = [0];
            if iroha_fs::read_at(file, &mut eof, length).map_err(native_error)? != 0 {
                return Err("state file grew beyond its admitted extent".into());
            }
            Ok(bytes)
        })();
        let original_check = FileSnapshot::private_journal(file)
            .map_err(native_error)
            .and_then(|after| {
                if after == snapshot {
                    Ok(())
                } else {
                    Err("state original changed during read".into())
                }
            });
        let name_check = self.require_original(name, file, identity);
        finish(finish(result, original_check), name_check)
    }

    pub(super) fn load<T>(
        &self,
        path: &Path,
        use_snapshot: impl FnOnce(Option<ChainStateSnapshot>) -> Result<T, String>,
    ) -> Result<T, String> {
        self.require_path(path)?;
        let mut original = None;
        let mut captured = None;
        let result = (|| {
            original = self
                .owner
                .open_private_journal_optional(&self.name)
                .map_err(native_error)?;
            original
                .as_ref()
                .map(|file| {
                    captured = Some(FileSnapshot::private_journal(file).map_err(native_error)?);
                    let bytes = self.read_original(&self.name, file)?;
                    decode_state_snapshot(&bytes)
                })
                .transpose()
        })();
        // Sink binding and pending-record hash/signature validation finish while this
        // exact original is still retained, including every parser refusal.
        let result = result.and_then(use_snapshot);
        let original_check = original.as_ref().map_or(Ok(()), |file| {
            let after = FileSnapshot::private_journal(file).map_err(native_error)?;
            if captured.is_some_and(|before| before != after) {
                return Err("state original changed during decode".into());
            }
            FileIdentity::of(file)
                .map_err(native_error)
                .and_then(|id| self.require_original(&self.name, file, id))
        });
        let final_check = self.revalidate();
        finish(finish(result, original_check), final_check)
    }

    pub(super) fn persist(&self, payload: Vec<u8>) -> Result<(), String> {
        self.persist_observed(payload, |_| Ok(()))
    }

    fn persist_observed(
        &self,
        payload: Vec<u8>,
        mut observe: impl FnMut(AtomicWriteStep) -> Result<(), String>,
    ) -> Result<(), String> {
        self.revalidate()?;
        if u64::try_from(payload.len()).unwrap_or(u64::MAX) > STATE_FILE_MAX_BYTES {
            return finish(
                Err("serialized state exceeds its byte limit".into()),
                self.revalidate(),
            );
        }
        let mut slot = match self.pending.lock() {
            Ok(slot) => slot,
            Err(_) => {
                return finish(
                    Err("native telemetry publication owner was poisoned".into()),
                    self.revalidate(),
                );
            }
        };
        if slot
            .as_ref()
            .is_some_and(|pending| pending.payload != payload)
        {
            return finish(
                Err("an uncertain telemetry publication requires its exact original bytes".into()),
                self.revalidate(),
            );
        }
        let pending = slot.get_or_insert_with(|| Publication {
            payload,
            original: None,
            original_identity: None,
            predecessor: None,
            predecessor_snapshot: None,
            predecessor_identity: None,
            predecessor_selected: false,
            published: false,
        });
        let result = self.advance(pending, &mut observe);
        let original_check = self.check_publication(pending);
        let final_check = self.revalidate();
        let result = result.and(original_check).and(final_check);
        if result.is_ok() {
            // The held predecessor and new original survive the final readback/barrier and
            // all custody checks. Only an accepted phase releases them for the next record.
            *slot = None;
        }
        result
    }

    fn advance(
        &self,
        pending: &mut Publication,
        observe: &mut impl FnMut(AtomicWriteStep) -> Result<(), String>,
    ) -> Result<(), String> {
        if !pending.predecessor_selected {
            if pending.predecessor.is_none() {
                pending.predecessor = self
                    .owner
                    .open_private_journal_optional(&self.name)
                    .map_err(native_error)?;
            }
            if pending.predecessor_snapshot.is_none() {
                pending.predecessor_snapshot = pending
                    .predecessor
                    .as_ref()
                    .map(FileSnapshot::private_journal)
                    .transpose()
                    .map_err(native_error)?;
            }
            if pending.predecessor_identity.is_none() {
                pending.predecessor_identity = pending
                    .predecessor
                    .as_ref()
                    .map(FileIdentity::of)
                    .transpose()
                    .map_err(native_error)?;
            }
            pending.predecessor_selected = true;
        }
        if pending.original.is_none() {
            // An interrupted pre-publication stage is uncommitted. Admit its exact private
            // original and bound its extent before overwriting; never remove a pathname.
            pending.original = self
                .owner
                .open_private_journal_optional(&self.temporary)
                .map_err(native_error)?;
            if pending.original.is_none() {
                self.owner
                    .create_private_journal_into(&self.temporary, &mut pending.original)
                    .map_err(native_error)?;
            }
        }
        self.check_publication(pending)?;
        let file = pending
            .original
            .as_mut()
            .ok_or_else(|| "missing telemetry stage original".to_string())?;
        let identity = FileIdentity::of(file).map_err(native_error)?;
        if let Some(expected) = pending.original_identity {
            if identity != expected {
                return Err("telemetry stage identity changed".into());
            }
        } else {
            pending.original_identity = Some(identity);
        }
        if !pending.published {
            self.require_original(&self.temporary, file, identity)?;
            if file.metadata().map_err(native_error)?.len() > STATE_FILE_MAX_BYTES {
                return Err("abandoned telemetry stage exceeds its byte limit".into());
            }
            // Retry owns this same unpublished original; it cannot replace committed state.
            observe(AtomicWriteStep::WriteHead)?;
            self.require_original(&self.temporary, file, identity)?;
            file.seek(SeekFrom::Start(0)).map_err(native_error)?;
            file.set_len(0).map_err(native_error)?;
            let (head, tail) = pending.payload.split_at(pending.payload.len() / 2);
            file.write_all(head).map_err(native_error)?;
            observe(AtomicWriteStep::WriteTail)?;
            self.require_original(&self.temporary, file, identity)?;
            file.write_all(tail).map_err(native_error)?;
            observe(AtomicWriteStep::SyncFile)?;
            self.require_original(&self.temporary, file, identity)?;
            file.sync_all().map_err(native_error)?;
            if self.read_original(&self.temporary, file)? != pending.payload {
                return Err("telemetry stage readback differs".into());
            }
            if let Some(predecessor) = pending.predecessor.as_ref() {
                self.owner
                    .require_same_lock(&self.name, predecessor)
                    .map_err(native_error)?;
                if Some(FileSnapshot::private_journal(predecessor).map_err(native_error)?)
                    != pending.predecessor_snapshot
                {
                    return Err("telemetry predecessor changed before publication".into());
                }
            }
            self.owner
                .publish_private_journal_observed(
                    &self.temporary,
                    &self.name,
                    file,
                    pending.predecessor.as_ref(),
                    &mut pending.published,
                    |step| {
                        let result = observe(step);
                        let final_check = self.revalidate();
                        result.and(final_check).map_err(io::Error::other)
                    },
                )
                .map_err(native_error)?;
        }
        // A post-publication error resumes here with the same new/displaced originals.
        self.require_original(&self.name, file, identity)?;
        file.sync_all().map_err(native_error)?;
        if self.read_original(&self.name, file)? != pending.payload {
            return Err("published telemetry readback differs".into());
        }
        observe(AtomicWriteStep::SyncDirectory)?;
        self.revalidate()?;
        self.owner.sync().map_err(native_error)
    }

    fn check_publication(&self, pending: &Publication) -> Result<(), String> {
        let new_check = pending.original.as_ref().map_or(Ok(()), |file| {
            let id = FileIdentity::of(file).map_err(native_error)?;
            if pending
                .original_identity
                .is_some_and(|expected| expected != id)
            {
                return Err("telemetry stage identity changed".into());
            }
            self.require_original(
                if pending.published {
                    &self.name
                } else {
                    &self.temporary
                },
                file,
                id,
            )
        });
        let old_check = pending.predecessor.as_ref().map_or(Ok(()), |file| {
            if pending.published {
                let identity = FileIdentity::of(file).map_err(native_error)?;
                if pending.predecessor_identity != Some(identity) {
                    return Err("displaced telemetry original identity changed".into());
                }
                self.owner
                    .require_retired_private_journal(file)
                    .map_err(native_error)
            } else {
                let snapshot = FileSnapshot::private_journal(file).map_err(native_error)?;
                if pending
                    .predecessor_snapshot
                    .is_some_and(|before| before != snapshot)
                {
                    return Err("telemetry predecessor changed".into());
                }
                self.owner
                    .require_same_lock(&self.name, file)
                    .map_err(native_error)
            }
        });
        new_check.and(old_check)
    }
}

#[cfg(test)]
#[path = "integrity_windows_tests.rs"]
mod tests;
