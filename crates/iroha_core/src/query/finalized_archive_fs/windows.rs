//! Retained native originals for the two immutable finalized archives.
//!
//! Logical immutability is enforced by no-replace publication and exact same-byte replay.
//! Files keep protected private writable custody, as on Unix (0600). The archive writer
//! serializes mutations. A failed mutation keeps its original and phase until reconciliation;
//! ordinary readers are opened only after that writable original has completed its barriers.

use super::{STAGED_FILE_PREFIX, is_canonical_digest_file_name};
use iroha_fs::{
    AtomicWriteStep, FileIdentity, FileMetadata, FileSnapshot, OwnerDirectory, RetainedFile,
};
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Mutex,
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Full native metadata observations; this is observation, never private-file authority.
pub(in crate::query) mod metadata {
    use super::*;
    pub(crate) type SecureMetadata = FileMetadata;
    pub(crate) fn from_file(file: &File) -> io::Result<SecureMetadata> {
        FileMetadata::of(file)
    }
    pub(crate) fn from_path(path: &Path) -> io::Result<SecureMetadata> {
        FileMetadata::symlink_metadata(path)
    }
    pub(crate) fn is_single_link(value: &SecureMetadata) -> bool {
        value.links() == 1
    }
}

#[derive(Debug)]
struct Pending {
    name: OsString,
    target: Option<OsString>,
    original: Option<File>,
    identity: Option<FileIdentity>,
    published: bool,
    disposed: bool,
    retention: bool,
    expected: Vec<u8>,
}

#[derive(Debug)]
struct Directory {
    // Pending leaf originals close before their retained parent chain is released.
    pending: Mutex<Option<Pending>>,
    selected: Mutex<BTreeMap<OsString, FileIdentity>>,
    owner: OwnerDirectory,
    suffix: &'static str,
    maximum_bytes: u64,
    maximum_entries: usize,
}

/// Fixed retained archive directories, never a process-global path registry.
#[derive(Debug)]
pub(in crate::query) struct NativeArchive {
    directories: Vec<Directory>,
    root: OwnerDirectory,
}

impl NativeArchive {
    pub(in crate::query) fn open(
        root: &Path,
        children: &[(&str, &'static str)],
        maximum_bytes: u64,
        maximum_entries: usize,
    ) -> io::Result<Self> {
        let root = OwnerDirectory::open_or_create(root)?;
        let mut directories = Vec::with_capacity(children.len());
        for &(name, suffix) in children {
            let owner = root.ensure_child(name)?;
            directories.push(Directory {
                owner,
                suffix,
                maximum_bytes,
                maximum_entries,
                pending: Mutex::new(None),
                selected: Mutex::new(BTreeMap::new()),
            });
        }
        let value = Self { root, directories };
        value.revalidate()?;
        Ok(value)
    }

    pub(in crate::query) fn revalidate(&self) -> io::Result<()> {
        let result = (|| {
            self.root.revalidate()?;
            for directory in &self.directories {
                let mut pending = directory
                    .pending
                    .lock()
                    .map_err(|_| invalid("archive pending phase lock poisoned"))?;
                directory.reconcile(&mut pending)?;
                directory.validate_selected()?;
                directory.owner.revalidate()?;
            }
            Ok(())
        })();
        let final_check = self.root.revalidate();
        result.and(final_check)
    }

    /// Complete only previously authenticated retention intents before an inventory rescan.
    /// A rescan can omit a deleted name, so it cannot supply the old bytes again. The pending
    /// phase already owns those bytes, the selected full identity and the disposition latch.
    /// No name supplied by this operation can authorize a new deletion.
    pub(in crate::query) fn finish_retention(&self) -> io::Result<bool> {
        let result = (|| {
            self.root.revalidate()?;
            let mut completed = false;
            for directory in &self.directories {
                let mut pending = directory
                    .pending
                    .lock()
                    .map_err(|_| invalid("archive pending phase lock poisoned"))?;
                if !pending.as_ref().is_some_and(|phase| phase.retention) {
                    continue;
                }
                directory.reconcile(&mut pending)?;
                // Reconciliation preserves the full identity until the authoritative original
                // closes, then refuses a replacement name and finishes the directory barrier.
                // Keep the intent if this final root observation fails too.
                self.root.revalidate()?;
                *pending = None;
                completed = true;
            }
            Ok(completed)
        })();
        let final_check = self.root.revalidate();
        match result {
            Ok(completed) => final_check.map(|()| completed),
            Err(error) => {
                let _ = final_check;
                Err(error)
            }
        }
    }

    #[cfg(test)]
    pub(in crate::query) fn unlink_after_close_observed(
        &self,
        path: &Path,
        identity: FileIdentity,
        target: &Path,
        expected: &[u8],
        after_close: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let directory = self.directory(path, identity)?;
        let name = directory.canonical_name(target)?;
        directory
            .unlink_phases_observed(name, Some(expected), |_| Ok(()), after_close)
            .and(self.root.revalidate())
    }

    fn directory(&self, path: &Path, identity: FileIdentity) -> io::Result<&Directory> {
        self.root.revalidate()?;
        let directory = self
            .directories
            .iter()
            .find(|directory| directory.owner.path() == path)
            .ok_or_else(|| invalid("archive directory is outside the retained owner"))?;
        directory.owner.revalidate()?;
        if directory.owner.identity()? != identity {
            return Err(invalid("archive directory identity changed"));
        }
        Ok(directory)
    }

    pub(in crate::query) fn publish(
        &self,
        path: &Path,
        identity: FileIdentity,
        target: &Path,
        bytes: &[u8],
    ) -> io::Result<()> {
        self.publish_observed(path, identity, target, bytes, |_| Ok(()))
    }

    fn publish_observed(
        &self,
        path: &Path,
        identity: FileIdentity,
        target: &Path,
        bytes: &[u8],
        mut observe: impl FnMut(AtomicWriteStep) -> io::Result<()>,
    ) -> io::Result<()> {
        let directory = self.directory(path, identity)?;
        let target_name = directory.canonical_name(target)?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > directory.maximum_bytes {
            return Err(invalid("archive publication exceeds its byte bound"));
        }
        let mut pending = directory
            .pending
            .lock()
            .map_err(|_| invalid("archive pending phase lock poisoned"))?;
        let result = (|| {
            directory.reconcile(&mut pending)?;
            if pending.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "retention must acknowledge its completed deletion before another mutation",
                ));
            }
            if let Some(existing) = directory
                .owner
                .open_retained_regular_optional(target_name)?
            {
                // Narrow the already-retained ordinary original to protected private custody.
                FileSnapshot::of(existing.file(), true)?;
                directory.require_selected(target_name, existing.identity()?)?;
                let before = existing.snapshot()?;
                let same = read_original(existing.file(), directory.maximum_bytes)? == bytes;
                let final_check = existing.revalidate().and_then(|()| {
                    if existing.snapshot()? != before {
                        return Err(invalid("archive replay original changed"));
                    }
                    FileSnapshot::of(existing.file(), true).map(|_| ())
                });
                let comparison = if same {
                    Ok(())
                } else {
                    Err(invalid("immutable archive bytes conflict"))
                };
                return comparison.and(final_check);
            }
            directory.require_unselected(target_name)?;
            for _ in 0..128 {
                let name = OsString::from(format!(
                    "{STAGED_FILE_PREFIX}{:08x}-{:016x}",
                    std::process::id(),
                    rand::random::<u64>()
                ));
                *pending = Some(Pending {
                    name,
                    target: Some(target_name.to_owned()),
                    original: None,
                    identity: None,
                    published: false,
                    disposed: false,
                    retention: false,
                    expected: bytes.to_vec(),
                });
                let phase = pending
                    .as_mut()
                    .ok_or_else(|| invalid("missing archive creator phase"))?;
                let creation = observe(AtomicWriteStep::CreateTemp).and_then(|()| {
                    directory
                        .owner
                        .create_private_journal_into(&phase.name, &mut phase.original)
                });
                match creation {
                    Err(error) if phase.original.is_none() => {
                        *pending = None;
                        if error.kind() == io::ErrorKind::AlreadyExists {
                            continue;
                        }
                        return Err(error);
                    }
                    result => {
                        result?;
                        break;
                    }
                }
            }
            let phase = pending.as_mut().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "archive staged-name attempts exhausted",
                )
            })?;
            let file = phase
                .original
                .as_mut()
                .ok_or_else(|| invalid("archive creator lost its original"))?;
            phase.identity = Some(FileMetadata::of(file)?.identity());
            file.write_all(bytes)?;
            file.sync_all()?;
            if read_original(file, directory.maximum_bytes)? != bytes {
                return Err(invalid("archive staged readback differs"));
            }
            directory.owner.require_same_lock(&phase.name, file)?;
            let publication = directory.owner.publish_private_journal_observed(
                &phase.name,
                target_name,
                file,
                None,
                &mut phase.published,
                &mut observe,
            );
            let identity_join = if phase.published {
                directory.select(
                    target_name,
                    phase
                        .identity
                        .ok_or_else(|| invalid("published archive lacks captured identity"))?,
                )
            } else {
                Ok(())
            };
            publication.and(identity_join)?;
            directory.finish_publication(phase)?;
            Ok(())
        })();
        let original_check = pending
            .as_ref()
            .filter(|phase| phase.original.is_some())
            .map_or(Ok(()), |phase| directory.check_original(phase));
        let final_check = directory.owner.revalidate().and(self.root.revalidate());
        let result = result.and(original_check).and(final_check);
        if result.is_ok() {
            *pending = None;
        }
        result
    }

    pub(in crate::query) fn unlink(
        &self,
        path: &Path,
        identity: FileIdentity,
        target: &Path,
        expected: &[u8],
    ) -> io::Result<()> {
        let directory = self.directory(path, identity)?;
        let name = directory.canonical_name(target)?;
        directory
            .unlink(name, Some(expected))
            .and(self.root.revalidate())
    }

    pub(in crate::query) fn recover(
        &self,
        path: &Path,
        identity: FileIdentity,
        maximum_bytes: u64,
        suffix: &str,
    ) -> io::Result<()> {
        let directory = self.directory(path, identity)?;
        if maximum_bytes != directory.maximum_bytes || suffix != directory.suffix {
            return Err(invalid("archive recovery bounds changed"));
        }
        let result = (|| {
            {
                let mut pending = directory
                    .pending
                    .lock()
                    .map_err(|_| invalid("archive pending phase lock poisoned"))?;
                directory.reconcile(&mut pending)?;
            }
            // Bound enumeration with the archive's configured capacity, including crash stages.
            // This is deliberately independent of the generic atomic writer's 16-stage policy.
            let maximum = directory
                .maximum_entries
                .checked_mul(2)
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| invalid("archive recovery entry bound overflow"))?;
            for name in directory.owner.entries(maximum)? {
                let text = name
                    .to_str()
                    .ok_or_else(|| invalid("archive filename is not UTF-8"))?;
                if text.starts_with(STAGED_FILE_PREFIX) {
                    if !staged_name(text) {
                        return Err(invalid("foreign staged archive name"));
                    }
                    directory.unlink(&name, None)?;
                } else if is_canonical_digest_file_name(text, directory.suffix) {
                    let original = RetainedFile::open_private(directory.owner.path().join(&name))?;
                    let result = read_original(original.file(), maximum_bytes)
                        .and_then(|_| directory.select(&name, original.identity()?));
                    let final_check = original.revalidate().and(directory.owner.revalidate());
                    result.and(final_check)?;
                }
            }
            directory.owner.sync()
        })();
        let final_check = directory.owner.revalidate().and(self.root.revalidate());
        result.and(final_check)
    }
}

impl Directory {
    fn require_unselected(&self, name: &OsStr) -> io::Result<()> {
        let selected = self
            .selected
            .lock()
            .map_err(|_| invalid("archive identity table poisoned"))?;
        if selected.contains_key(name) {
            return Err(invalid(
                "selected archive original disappeared before publication",
            ));
        }
        if selected.len() >= self.maximum_entries {
            return Err(invalid("archive selected-original count exceeds bound"));
        }
        Ok(())
    }
    fn select(&self, name: &OsStr, identity: FileIdentity) -> io::Result<()> {
        let mut selected = self
            .selected
            .lock()
            .map_err(|_| invalid("archive identity table poisoned"))?;
        match selected.get(name) {
            Some(previous) if *previous != identity => {
                return Err(invalid("archive canonical original was substituted"));
            }
            Some(_) => return Ok(()),
            None => {}
        }
        if selected.len() >= self.maximum_entries {
            return Err(invalid("archive selected-original count exceeds bound"));
        }
        selected.insert(name.to_owned(), identity);
        Ok(())
    }

    fn require_selected(&self, name: &OsStr, identity: FileIdentity) -> io::Result<()> {
        let selected = self
            .selected
            .lock()
            .map_err(|_| invalid("archive identity table poisoned"))?;
        if selected.get(name) != Some(&identity) {
            return Err(invalid("archive canonical original differs from admission"));
        }
        Ok(())
    }

    fn validate_selected(&self) -> io::Result<()> {
        let selected = self
            .selected
            .lock()
            .map_err(|_| invalid("archive identity table poisoned"))?;
        for (name, identity) in selected.iter() {
            let original = RetainedFile::open_private(self.owner.path().join(name))?;
            let result = (|| {
                if original.identity()? == *identity {
                    Ok(())
                } else {
                    Err(invalid("archive selected original was replaced"))
                }
            })();
            let final_check = original.revalidate();
            result.and(final_check)?;
        }
        self.owner.revalidate()
    }
    fn canonical_name<'a>(&self, path: &'a Path) -> io::Result<&'a OsStr> {
        if path.parent() != Some(self.owner.path()) {
            return Err(invalid("archive target parent differs"));
        }
        let name = path
            .file_name()
            .ok_or_else(|| invalid("archive target has no name"))?;
        if !name
            .to_str()
            .is_some_and(|name| is_canonical_digest_file_name(name, self.suffix))
        {
            return Err(invalid("archive target is not a canonical digest name"));
        }
        Ok(name)
    }

    fn check_original(&self, phase: &Pending) -> io::Result<()> {
        let original = phase
            .original
            .as_ref()
            .ok_or_else(|| invalid("archive phase has no retained original"))?;
        let identity = FileMetadata::of(original)?.identity();
        if phase.identity.is_some_and(|expected| expected != identity) {
            return Err(invalid("archive retained identity changed"));
        }
        if phase.disposed {
            self.owner.require_retired_private_journal(original)
        } else {
            let name = if phase.published {
                phase
                    .target
                    .as_deref()
                    .ok_or_else(|| invalid("published archive has no target"))?
            } else {
                &phase.name
            };
            self.owner.require_same_lock(name, original)
        }
    }

    fn finish_publication(&self, phase: &Pending) -> io::Result<()> {
        let result = (|| {
            self.select(
                phase
                    .target
                    .as_deref()
                    .ok_or_else(|| invalid("published archive lacks target"))?,
                phase
                    .identity
                    .ok_or_else(|| invalid("published archive lacks identity"))?,
            )?;
            self.check_original(phase)?;
            let original = phase
                .original
                .as_ref()
                .ok_or_else(|| invalid("published archive lost its original"))?;
            if read_original(original, self.maximum_bytes)? != phase.expected {
                return Err(invalid("published archive readback differs"));
            }
            original.sync_all()?;
            self.owner.sync()?;
            self.check_original(phase)
        })();
        let original_check = self.check_original(phase);
        let final_check = self.owner.revalidate();
        result.and(original_check).and(final_check)
    }

    fn finish_disposition(&self, phase: &mut Pending) -> io::Result<()> {
        self.finish_disposition_observed(phase, |_| Ok(()))
    }

    fn finish_disposition_observed(
        &self,
        phase: &mut Pending,
        after_disposition: impl FnMut(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        self.finish_disposition_phases_observed(phase, after_disposition, || Ok(()))
    }

    fn finish_disposition_phases_observed(
        &self,
        phase: &mut Pending,
        mut after_disposition: impl FnMut(&File) -> io::Result<()>,
        mut after_close: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let result = (|| {
            if phase.original.is_some() {
                self.check_original(phase)?;
                if !phase.disposed {
                    let original = phase
                        .original
                        .as_ref()
                        .ok_or_else(|| invalid("archive disposal lost original"))?;
                    let identity = FileMetadata::of(original)?.identity();
                    if phase.identity.is_some_and(|expected| expected != identity) {
                        return Err(invalid("archive disposal identity changed"));
                    }
                    phase.identity = Some(identity);
                    if phase.retention {
                        self.require_selected(&phase.name, identity)?;
                    }
                    let bytes = read_original(original, self.maximum_bytes)?;
                    if phase.retention && bytes != phase.expected {
                        return Err(invalid(
                            "archive retention original differs from authenticated bytes",
                        ));
                    }
                    let disposition = self.owner.retire_private_journal(
                        &phase.name,
                        phase
                            .original
                            .as_ref()
                            .ok_or_else(|| invalid("archive retirement lost original"))?,
                        &mut phase.disposed,
                    );
                    let identity_join = if phase.disposed && phase.retention {
                        self.selected
                            .lock()
                            .map_err(|_| invalid("archive identity table poisoned"))
                            .map(|mut selected| {
                                selected.remove(&phase.name);
                            })
                    } else {
                        Ok(())
                    };
                    disposition.and(identity_join)?;
                }
                after_disposition(
                    phase
                        .original
                        .as_ref()
                        .ok_or_else(|| invalid("archive disposal lost original"))?,
                )?;
                self.check_original(phase)?;
                self.owner.revalidate()?;
                // Closing the authoritative delete-pending original is a separate explicit phase.
                // A later failure retries only absence/sync, never disposition against another name.
                phase.original = None;
            }
            if !phase.disposed {
                return Err(invalid("archive close lacks disposition latch"));
            }
            after_close()?;
            if self
                .owner
                .open_retained_regular_optional(&phase.name)?
                .is_some()
            {
                return Err(invalid("archive retired name was replaced"));
            }
            self.owner.sync()?;
            self.owner.revalidate()
        })();
        let original_check = if phase.original.is_some() {
            self.check_original(phase)
        } else {
            Ok(())
        };
        let final_check = self.owner.revalidate();
        result.and(original_check).and(final_check)
    }

    fn reconcile(&self, pending: &mut Option<Pending>) -> io::Result<()> {
        let Some(phase) = pending.as_mut() else {
            return self.owner.revalidate();
        };
        let result = if phase.published {
            self.finish_publication(phase)
        } else {
            self.finish_disposition(phase)
        };
        let final_check = self.owner.revalidate();
        let result = result.and(final_check);
        if result.is_ok() && !phase.retention {
            *pending = None;
        }
        result
    }

    fn unlink(&self, name: &OsStr, expected: Option<&[u8]>) -> io::Result<()> {
        self.unlink_observed(name, expected, |_| Ok(()))
    }

    fn unlink_observed(
        &self,
        name: &OsStr,
        expected: Option<&[u8]>,
        after_disposition: impl FnMut(&File) -> io::Result<()>,
    ) -> io::Result<()> {
        self.unlink_phases_observed(name, expected, after_disposition, || Ok(()))
    }

    fn unlink_phases_observed(
        &self,
        name: &OsStr,
        expected: Option<&[u8]>,
        after_disposition: impl FnMut(&File) -> io::Result<()>,
        after_close: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        let mut pending = self
            .pending
            .lock()
            .map_err(|_| invalid("archive pending phase lock poisoned"))?;
        // If this exact deletion was already applied, finish it without reopening its name.
        if pending
            .as_ref()
            .is_some_and(|phase| phase.target.is_none() && phase.name == name)
        {
            if pending.as_ref().is_some_and(|phase| {
                phase.retention != expected.is_some()
                    || expected.is_some_and(|bytes| bytes != phase.expected)
            }) {
                return Err(invalid(
                    "archive deletion retry changed its authenticated bytes",
                ));
            }
            let phase = pending
                .as_mut()
                .ok_or_else(|| invalid("missing retention retry"))?;
            self.finish_disposition_phases_observed(phase, after_disposition, after_close)?;
            *pending = None;
            return Ok(());
        }
        self.reconcile(&mut pending)?;
        if pending.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "another retention deletion awaits acknowledgement",
            ));
        }
        let original = self
            .owner
            .open_private_journal_optional(name)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        *pending = Some(Pending {
            name: name.to_owned(),
            target: None,
            identity: None,
            original: Some(original),
            published: false,
            disposed: false,
            retention: expected.is_some(),
            expected: expected.unwrap_or_default().to_vec(),
        });
        let phase = pending
            .as_mut()
            .ok_or_else(|| invalid("missing archive retirement phase"))?;
        let result = self.finish_disposition_phases_observed(phase, after_disposition, after_close);
        let final_check = self.owner.revalidate();
        let result = result.and(final_check);
        if result.is_ok() {
            *pending = None;
        }
        result
    }
}

fn staged_name(name: &str) -> bool {
    let Some(body) = name.strip_prefix(STAGED_FILE_PREFIX) else {
        return false;
    };
    body.len() == 25
        && body.as_bytes()[8] == b'-'
        && body.bytes().enumerate().all(|(index, byte)| {
            index == 8 || byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        })
}

fn read_original(original: &File, maximum: u64) -> io::Result<Vec<u8>> {
    let before = FileSnapshot::private_journal(original)?;
    let length = before.local_parts().2;
    if length > maximum {
        return Err(invalid("archive original exceeds read bound"));
    }
    let result = (|| {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(
                usize::try_from(length)
                    .map_err(|_| invalid("archive extent exceeds address space"))?,
            )
            .map_err(|_| io::Error::other("archive read allocation failed"))?;
        let mut reader = original;
        reader.seek(SeekFrom::Start(0))?;
        reader
            .take(maximum.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != length {
            return Err(invalid("archive extent changed while reading"));
        }
        Ok(bytes)
    })();
    let final_check = FileSnapshot::private_journal(original).and_then(|after| {
        if after == before {
            Ok(())
        } else {
            Err(invalid("archive original changed while reading"))
        }
    });
    match result {
        Ok(bytes) => final_check.map(|()| bytes),
        Err(error) => Err(error),
    }
}

pub(in crate::query) fn read(path: &Path, maximum: u64) -> io::Result<Vec<u8>> {
    let original = RetainedFile::open_private(path)?;
    let result = read_original(original.file(), maximum);
    let final_check = original.revalidate();
    match result {
        Ok(bytes) => final_check.map(|()| bytes),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests;
