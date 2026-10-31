//! Native APS persistence under the original private root, staging directory and writer lock.
//!
//! The common parent owns canonical validation, indices, reservations and lifecycle policy.
//! Failed mutations retain their exact originals until the same operation retries or the store
//! closes. A committed failure retains the parent protocol's poison-and-restart boundary.

use super::*;
use iroha_fs::{
    AtomicWriteStep, FileIdentity, FileMetadata, FileSnapshot, OwnerDirectory, PrivateDirectory,
};
use std::io::{self, Seek as _, SeekFrom, Write as _};

type Error = PrivateSettlementSidecarStoreErrorV1;

pub(super) struct NativeStore {
    // Originals close before the exclusive lock, and parents close last.
    pending: Mutex<Option<Pending>>,
    #[cfg(test)]
    next_failure: Mutex<Option<AtomicWriteStep>>,
    lock: File,
    lock_snapshot: FileSnapshot,
    root: PrivateDirectory,
    temporary: PrivateDirectory,
    owner: OwnerDirectory,
    temporary_owner: OwnerDirectory,
}

enum Pending {
    Publication(Publication),
    Removal(Removal),
}

struct Publication {
    name: String,
    temporary: String,
    bytes: Vec<u8>,
    replacing: bool,
    original: Option<File>,
    identity: Option<FileIdentity>,
    predecessor: Option<File>,
    predecessor_snapshot: Option<FileSnapshot>,
    predecessor_identity: Option<FileIdentity>,
    selected: bool,
    published: bool,
}

struct Removal {
    name: String,
    temporary: bool,
    original: Option<File>,
    identity: Option<FileIdentity>,
    disposed: bool,
    closed: bool,
}

fn finish<T, E>(result: Result<T, E>, check: Result<(), E>) -> Result<T, E> {
    result.and_then(|value| check.map(|()| value))
}

fn backend(_: io::Error) -> Error {
    Error::Backend
}
fn changed() -> io::Error {
    io::Error::other("APS retained private storage changed")
}

impl NativeStore {
    pub(super) fn open(path: &Path) -> Result<Self, Error> {
        let root = PrivateDirectory::open_or_create(path).map_err(backend)?;
        let opened = OwnerDirectory::open(root.path());
        let private_check = root.revalidate();
        let owner = finish(opened, private_check).map_err(backend)?;
        let mut lock = None;
        let mut lock_snapshot = None;
        let mut temporary = None;
        let mut temporary_owner = None;
        let result = (|| {
            if root.identity()? != owner.identity()? {
                return Err(changed());
            }
            lock = Some(owner.open_exclusive_private_lock(SIDECAR_WRITER_LOCK_FILE_V1)?);
            let held = lock.as_ref().ok_or_else(changed)?;
            lock_snapshot = Some(FileSnapshot::of(held, true)?);
            if held.metadata()?.len() != 0 {
                return Err(changed());
            }
            temporary = Some(
                match owner.open_private_child_optional(SIDECAR_TEMP_DIRECTORY_V1)? {
                    Some(directory) => directory,
                    None => owner.create_private_child(SIDECAR_TEMP_DIRECTORY_V1)?,
                },
            );
            let child = temporary.as_ref().ok_or_else(changed)?;
            temporary_owner = Some(OwnerDirectory::open(child.path())?);
            if child.identity()? != temporary_owner.as_ref().ok_or_else(changed)?.identity()? {
                return Err(changed());
            }
            Ok(())
        })();
        let lock_check = lock.as_ref().map_or(Ok(()), |held| {
            let after = FileSnapshot::of(held, true)?;
            if lock_snapshot.is_some_and(|before| before != after) {
                return Err(changed());
            }
            let named =
                FileMetadata::symlink_metadata(root.path().join(SIDECAR_WRITER_LOCK_FILE_V1))?;
            if named.identity() != FileIdentity::of(held)?
                || !named.is_file()
                || named.file_type().is_symlink()
                || named.links() != 1
            {
                return Err(changed());
            }
            Ok(())
        });
        let root_check = root.revalidate();
        let owner_check = owner.revalidate();
        let child_check = temporary
            .as_ref()
            .map_or(Ok(()), PrivateDirectory::revalidate);
        let child_owner_check = temporary_owner
            .as_ref()
            .map_or(Ok(()), OwnerDirectory::revalidate);
        result
            .and(lock_check)
            .and(root_check)
            .and(owner_check)
            .and(child_check)
            .and(child_owner_check)
            .map_err(|error| {
                if matches!(error.raw_os_error(), Some(32 | 33)) {
                    Error::StoreAlreadyOpen
                } else {
                    backend(error)
                }
            })?;
        let lock = lock.ok_or(Error::Backend)?;
        let lock_snapshot = lock_snapshot.ok_or(Error::Backend)?;
        let temporary = temporary.ok_or(Error::Backend)?;
        let temporary_owner = temporary_owner.ok_or(Error::Backend)?;
        let store = Self {
            pending: Mutex::new(None),
            #[cfg(test)]
            next_failure: Mutex::new(None),
            lock,
            lock_snapshot,
            root,
            temporary,
            owner,
            temporary_owner,
        };
        store.revalidate().map_err(backend)?;
        Ok(store)
    }

    pub(super) fn path(&self) -> &Path {
        self.root.path()
    }

    fn revalidate(&self) -> io::Result<()> {
        let lock_check = (|| {
            if FileSnapshot::of(&self.lock, true)? != self.lock_snapshot {
                return Err(changed());
            }
            let named =
                FileMetadata::symlink_metadata(self.root.path().join(SIDECAR_WRITER_LOCK_FILE_V1))?;
            if !named.is_file()
                || named.file_type().is_symlink()
                || named.links() != 1
                || named.identity() != FileIdentity::of(&self.lock)?
                || FileSnapshot::of(&self.lock, true)? != self.lock_snapshot
            {
                return Err(changed());
            }
            Ok(())
        })();
        let root = self.root.revalidate();
        let temporary = self.temporary.revalidate();
        let owner = self.owner.revalidate();
        let temporary_owner = self.temporary_owner.revalidate();
        lock_check
            .and(root)
            .and(temporary)
            .and(owner)
            .and(temporary_owner)
    }

    fn observe(
        &self,
        step: AtomicWriteStep,
        observer: &mut impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let result = observer(step);
        let check = self.revalidate();
        result.and(check)
    }

    fn require_original(
        &self,
        temporary: bool,
        name: &str,
        file: &File,
        identity: Option<FileIdentity>,
    ) -> io::Result<()> {
        let result = (|| {
            if identity.is_some_and(|expected| FileIdentity::of(file).ok() != Some(expected)) {
                return Err(changed());
            }
            FileSnapshot::private_journal(file)?;
            let owner = if temporary {
                &self.temporary_owner
            } else {
                &self.owner
            };
            owner.require_same_lock(name, file)
        })();
        let check = self.revalidate();
        result.and(check)
    }

    fn require_retired(
        &self,
        temporary: bool,
        file: &File,
        expected: FileIdentity,
    ) -> io::Result<()> {
        let result = (|| {
            if FileIdentity::of(file)? != expected {
                return Err(changed());
            }
            let owner = if temporary {
                &self.temporary_owner
            } else {
                &self.owner
            };
            owner.require_retired_private_journal(file)
        })();
        let check = self.revalidate();
        result.and(check)
    }

    pub(super) fn read<T>(
        &self,
        name: &str,
        parse: impl FnOnce(&[u8]) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let slot = match self.pending.lock() {
            Ok(slot) => slot,
            Err(_) => return finish(Err(Error::Backend), self.revalidate().map_err(backend)),
        };
        if let Some(pending) = slot.as_ref().filter(|pending| match pending {
            Pending::Publication(p) => p.name == name,
            Pending::Removal(p) => !p.temporary && p.name == name,
        }) {
            let borrowed = match pending {
                Pending::Publication(p) if !p.published && p.name == name => p.predecessor.as_ref(),
                Pending::Removal(p) if !p.disposed && !p.temporary && p.name == name => {
                    p.original.as_ref()
                }
                _ => None,
            };
            let result = match borrowed {
                Some(file) => self.read_pending(name, file, parse),
                None => Err(Error::Backend),
            };
            let originals = self.check_pending(pending).map_err(backend);
            let final_check = self.revalidate().map_err(backend);
            return finish(finish(result, originals), final_check);
        }
        let mut original = None;
        let mut snapshot = None;
        let result = (|| {
            self.revalidate().map_err(backend)?;
            if let Some(pending) = slot.as_ref() {
                self.check_pending(pending).map_err(backend)?;
            }
            original = Some(self.root.open_retained_private(name).map_err(backend)?);
            let held = original.as_ref().ok_or(Error::Backend)?;
            snapshot = Some(FileSnapshot::of(held.file(), true).map_err(backend)?);
            let length = held.file().metadata().map_err(backend)?.len();
            if length == 0 || length > PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 {
                return Err(Error::Corrupt);
            }
            let mut bytes = vec![0; usize::try_from(length).map_err(|_| Error::Corrupt)?];
            iroha_fs::read_exact_at(held.file(), &mut bytes, 0).map_err(backend)?;
            let mut eof = [0];
            if iroha_fs::read_at(held.file(), &mut eof, length).map_err(backend)? != 0 {
                return Err(Error::Corrupt);
            }
            parse(&bytes)
        })();
        let original_check = original.as_ref().map_or(Ok(()), |held| {
            let result = if Some(FileSnapshot::of(held.file(), true).map_err(backend)?) == snapshot
            {
                Ok(())
            } else {
                Err(Error::Corrupt)
            };
            let final_check = held.revalidate().map_err(backend);
            result.and(final_check)
        });
        let pending_check = slot.as_ref().map_or(Ok(()), |pending| {
            self.check_pending(pending).map_err(backend)
        });
        let final_check = self.revalidate().map_err(backend);
        finish(
            finish(finish(result, original_check), pending_check),
            final_check,
        )
    }

    fn read_pending<T>(
        &self,
        name: &str,
        file: &File,
        parse: impl FnOnce(&[u8]) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let captured = FileSnapshot::private_journal(file).map_err(backend)?;
        let identity = FileIdentity::of(file).map_err(backend)?;
        let result = (|| {
            self.require_original(false, name, file, Some(identity))
                .map_err(backend)?;
            let length = file.metadata().map_err(backend)?.len();
            if length == 0 || length > PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 {
                return Err(Error::Corrupt);
            }
            let mut bytes = vec![0; usize::try_from(length).map_err(|_| Error::Corrupt)?];
            iroha_fs::read_exact_at(file, &mut bytes, 0).map_err(backend)?;
            let mut eof = [0];
            if iroha_fs::read_at(file, &mut eof, length).map_err(backend)? != 0 {
                return Err(Error::Corrupt);
            }
            parse(&bytes)
        })();
        let snapshot_check = FileSnapshot::private_journal(file)
            .map_err(backend)
            .and_then(|after| {
                if after == captured {
                    Ok(())
                } else {
                    Err(Error::Corrupt)
                }
            });
        let custody = self
            .require_original(false, name, file, Some(identity))
            .map_err(backend);
        finish(finish(result, snapshot_check), custody)
    }

    pub(super) fn persist(
        &self,
        name: &str,
        bytes: &[u8],
        replacing: bool,
    ) -> Result<(), DurableSidecarMutationFailureV1> {
        #[cfg(test)]
        {
            self.persist_observed(name, bytes, replacing, |step| {
                let mut failure = self.next_failure.lock().map_err(|_| changed())?;
                if *failure == Some(step) {
                    *failure = None;
                    Err(io::Error::other("injected native APS persistence boundary"))
                } else {
                    Ok(())
                }
            })
        }
        #[cfg(not(test))]
        self.persist_observed(name, bytes, replacing, |_| Ok(()))
    }

    fn persist_observed(
        &self,
        name: &str,
        bytes: &[u8],
        replacing: bool,
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> Result<(), DurableSidecarMutationFailureV1> {
        let mut slot = match self.pending.lock() {
            Ok(slot) => slot,
            Err(_) => {
                let _final_check = self.revalidate();
                return Err(DurableSidecarMutationFailureV1::before());
            }
        };
        let result = (|| {
            self.revalidate()?;
            if bytes.is_empty()
                || bytes.len() as u64 > PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1
            {
                return Err(changed());
            }
            if slot.is_none() {
                *slot = Some(Pending::Publication(Publication {
                    name: name.into(),
                    temporary: format!("{name}{SIDECAR_TEMP_EXTENSION_V1}"),
                    bytes: bytes.into(),
                    replacing,
                    original: None,
                    identity: None,
                    predecessor: None,
                    predecessor_snapshot: None,
                    predecessor_identity: None,
                    selected: false,
                    published: false,
                }));
            }
            let Some(Pending::Publication(pending)) = slot.as_mut() else {
                return Err(changed());
            };
            if pending.name != name || pending.bytes != bytes || pending.replacing != replacing {
                return Err(changed());
            }
            self.advance_publication(pending, &mut observe)
        })();
        let original_check = slot
            .as_ref()
            .map_or(Ok(()), |pending| self.check_pending(pending));
        let final_check = self.revalidate();
        let result = result.and(original_check).and(final_check);
        let committed = matches!(slot.as_ref(), Some(Pending::Publication(p)) if p.published && p.name == name && p.bytes == bytes && p.replacing == replacing);
        if result.is_ok() {
            *slot = None;
        }
        result.map_err(|_| {
            if committed {
                DurableSidecarMutationFailureV1::after()
            } else {
                DurableSidecarMutationFailureV1::before()
            }
        })
    }

    fn advance_publication(
        &self,
        pending: &mut Publication,
        observe: &mut impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if !pending.selected {
            if pending.predecessor.is_none() {
                pending.predecessor = self.owner.open_private_journal_optional(&pending.name)?;
            }
            if pending.predecessor.is_some() != pending.replacing {
                return Err(changed());
            }
            if pending.predecessor_snapshot.is_none() {
                pending.predecessor_snapshot = pending
                    .predecessor
                    .as_ref()
                    .map(FileSnapshot::private_journal)
                    .transpose()?;
            }
            if pending.predecessor_identity.is_none() {
                pending.predecessor_identity = pending
                    .predecessor
                    .as_ref()
                    .map(FileIdentity::of)
                    .transpose()?;
            }
            pending.selected = true;
        }
        if pending.original.is_none() {
            self.observe(AtomicWriteStep::CreateTemp, observe)?;
            self.temporary_owner
                .create_private_journal_into(&pending.temporary, &mut pending.original)?;
        }
        let original = pending.original.as_mut().ok_or_else(changed)?;
        if pending.identity.is_none() {
            pending.identity = Some(FileIdentity::of(original)?);
        }
        if !pending.published {
            self.require_original(true, &pending.temporary, original, pending.identity)?;
            self.observe(AtomicWriteStep::WriteHead, observe)?;
            self.require_original(true, &pending.temporary, original, pending.identity)?;
            original.seek(SeekFrom::Start(0))?;
            original.set_len(0)?;
            let (head, tail) = pending.bytes.split_at(pending.bytes.len() / 2);
            original.write_all(head)?;
            self.observe(AtomicWriteStep::WriteTail, observe)?;
            self.require_original(true, &pending.temporary, original, pending.identity)?;
            original.write_all(tail)?;
            self.observe(AtomicWriteStep::SyncFile, observe)?;
            self.require_original(true, &pending.temporary, original, pending.identity)?;
            original.sync_all()?;
            self.require_bytes(true, &pending.temporary, original, &pending.bytes)?;
            if let Some(previous) = pending.predecessor.as_ref() {
                if Some(FileSnapshot::private_journal(previous)?) != pending.predecessor_snapshot {
                    return Err(changed());
                }
            }
            self.owner.publish_private_journal_from_observed(
                (&self.temporary_owner, &pending.temporary),
                &pending.name,
                original,
                pending.predecessor.as_ref(),
                &mut pending.published,
                |step| self.observe(step, observe),
            )?;
        }
        self.require_bytes(false, &pending.name, original, &pending.bytes)?;
        self.observe(AtomicWriteStep::SyncDirectory, observe)?;
        let root_sync = self.owner.sync();
        let temp_sync = self.temporary_owner.sync();
        root_sync.and(temp_sync)
    }

    fn require_bytes(
        &self,
        temporary: bool,
        name: &str,
        original: &File,
        expected: &[u8],
    ) -> io::Result<()> {
        let before = FileSnapshot::private_journal(original)?;
        let identity = FileIdentity::of(original)?;
        let result = (|| {
            self.require_original(temporary, name, original, Some(identity))?;
            if original.metadata()?.len() != expected.len() as u64 {
                return Err(changed());
            }
            let mut bytes = vec![0; expected.len()];
            iroha_fs::read_exact_at(original, &mut bytes, 0)?;
            if bytes != expected {
                return Err(changed());
            }
            Ok(())
        })();
        let snapshot_check = FileSnapshot::private_journal(original).and_then(|after| {
            if before == after {
                Ok(())
            } else {
                Err(changed())
            }
        });
        let name_check = self.require_original(temporary, name, original, Some(identity));
        result.and(snapshot_check).and(name_check)
    }

    fn check_pending(&self, pending: &Pending) -> io::Result<()> {
        match pending {
            Pending::Publication(p) => {
                let new_check = p.original.as_ref().map_or(Ok(()), |file| {
                    self.require_original(
                        !p.published,
                        if p.published { &p.name } else { &p.temporary },
                        file,
                        p.identity,
                    )
                });
                let old_check = p.predecessor.as_ref().map_or(Ok(()), |file| {
                    if p.published {
                        self.require_retired(
                            false,
                            file,
                            p.predecessor_identity.ok_or_else(changed)?,
                        )
                    } else {
                        let result =
                            self.require_original(false, &p.name, file, p.predecessor_identity);
                        let snapshot = FileSnapshot::private_journal(file).and_then(|after| {
                            if p.predecessor_snapshot.is_some_and(|before| before != after) {
                                Err(changed())
                            } else {
                                Ok(())
                            }
                        });
                        result.and(snapshot)
                    }
                });
                new_check.and(old_check)
            }
            Pending::Removal(p) => p.original.as_ref().map_or(Ok(()), |file| {
                if p.disposed {
                    self.require_retired(p.temporary, file, p.identity.ok_or_else(changed)?)
                } else {
                    self.require_original(p.temporary, &p.name, file, p.identity)
                }
            }),
        }
    }

    pub(super) fn remove(&self, name: &str) -> Result<(), DurableSidecarMutationFailureV1> {
        self.remove_observed(name, false, |_| Ok(()))
    }

    fn remove_observed(
        &self,
        name: &str,
        temporary: bool,
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> Result<(), DurableSidecarMutationFailureV1> {
        let mut slot = match self.pending.lock() {
            Ok(slot) => slot,
            Err(_) => {
                let _final_check = self.revalidate();
                return Err(DurableSidecarMutationFailureV1::before());
            }
        };
        let result = (|| {
            self.revalidate()?;
            if slot.is_none() {
                *slot = Some(Pending::Removal(Removal {
                    name: name.into(),
                    temporary,
                    original: None,
                    identity: None,
                    disposed: false,
                    closed: false,
                }));
            }
            let Some(Pending::Removal(pending)) = slot.as_mut() else {
                return Err(changed());
            };
            if pending.name != name || pending.temporary != temporary {
                return Err(changed());
            }
            let owner = if temporary {
                &self.temporary_owner
            } else {
                &self.owner
            };
            if !pending.closed {
                if pending.original.is_none() {
                    pending.original = owner.open_private_journal_optional(name)?;
                }
                let file = pending.original.as_ref().ok_or_else(changed)?;
                if pending.identity.is_none() {
                    pending.identity = Some(FileIdentity::of(file)?);
                }
                if !pending.disposed {
                    self.require_original(temporary, name, file, pending.identity)?;
                    if file.metadata()?.len() > PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 {
                        return Err(changed());
                    }
                    self.observe(AtomicWriteStep::Publish, &mut observe)?;
                    self.require_original(temporary, name, file, pending.identity)?;
                    owner.retire_private_journal(name, file, &mut pending.disposed)?;
                }
                self.require_retired(temporary, file, pending.identity.ok_or_else(changed)?)?;
                self.observe(AtomicWriteStep::Published, &mut observe)?;
                self.require_retired(temporary, file, pending.identity.ok_or_else(changed)?)?;
                drop(pending.original.take());
                pending.closed = true;
            }
            // A failed post-close barrier retries from this explicit state, never a reopen.
            self.observe(AtomicWriteStep::SyncDirectory, &mut observe)?;
            let absence = match FileMetadata::symlink_metadata(owner.path().join(name)) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                Ok(_) => Err(changed()),
                Err(error) => Err(error),
            };
            let barrier = owner.sync();
            absence.and(barrier)
        })();
        let original_check = slot
            .as_ref()
            .map_or(Ok(()), |pending| self.check_pending(pending));
        let final_check = self.revalidate();
        let result = result.and(original_check).and(final_check);
        let committed = matches!(slot.as_ref(), Some(Pending::Removal(p)) if p.disposed && p.name == name && p.temporary == temporary);
        if result.is_ok() {
            *slot = None;
        }
        result.map_err(|_| {
            if committed {
                DurableSidecarMutationFailureV1::after()
            } else {
                DurableSidecarMutationFailureV1::before()
            }
        })
    }

    pub(super) fn recover(
        &self,
        config: PrivateSettlementSidecarStoreConfigV1,
    ) -> Result<SidecarStoreStateV1, Error> {
        let result = (|| {
            self.revalidate().map_err(backend)?;
            // Every interrupted operation can leave at most one stage per live record;
            // promotion can temporarily retain both certified and provisional records.
            let maximum = PRIVATE_SETTLEMENT_SIDECAR_HARD_MAX_RECORDS_V1 * 2;
            for name in self.temporary.entries(maximum).map_err(backend)? {
                let name = name.to_str().ok_or(Error::Corrupt)?;
                let record = name
                    .strip_suffix(SIDECAR_TEMP_EXTENSION_V1)
                    .ok_or(Error::Corrupt)?;
                if parse_sidecar_file_name_v1(record).is_err()
                    && parse_provisional_file_name_v1(record).is_err()
                {
                    return Err(Error::Corrupt);
                }
                self.remove_observed(name, true, |_| Ok(()))
                    .map_err(|_| Error::Backend)?;
            }
            // Repeat recovery's barrier even when a prior attempt removed the final stage.
            self.temporary_owner.sync().map_err(backend)?;
            let mut entries = self.root.entries(maximum + 2).map_err(backend)?;
            entries.sort_by_key(|name| {
                (
                    !name
                        .to_str()
                        .is_some_and(|name| name.ends_with(PROVISIONAL_RECORD_EXTENSION_V1)),
                    name.clone(),
                )
            });
            let mut state = empty_file_store_state_v1();
            let mut provisional_records = BTreeMap::new();
            let mut final_records = BTreeMap::new();
            for name in entries {
                let name = name.to_str().ok_or(Error::Corrupt)?;
                if name == SIDECAR_TEMP_DIRECTORY_V1 || name == SIDECAR_WRITER_LOCK_FILE_V1 {
                    self.revalidate().map_err(backend)?;
                    continue;
                }
                self.read(name, |bytes| {
                    index_canonical_file_v1(
                        &mut state,
                        &mut provisional_records,
                        &mut final_records,
                        name,
                        bytes,
                    )
                })?;
            }
            reconcile_loaded_records_v1(&mut state, provisional_records, final_records, |name| {
                self.remove(name).map_err(|_| Error::Backend)
            })?;
            self.owner.sync().map_err(backend)?;
            check_file_store_capacity_v1(&state, config)?;
            Ok(state)
        })();
        let check = self.revalidate().map_err(backend);
        finish(result, check)
    }
}

#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;
