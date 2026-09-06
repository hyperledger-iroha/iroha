//! Private descriptor-owned append-only journals shared by offline Core stores.
//!
//! This layer owns durable bytes, not monetary authority. Record owners supply their own exact
//! Norito schema and authenticate recovered state against current hardware before using it.

use rustix::fs::{Mode, OFlags};
use sha2::{Digest as _, Sha256};
use std::{
    cell::Cell,
    fs::{File, Metadata, TryLockError},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::MetadataExt as _,
    path::{Component, Path, PathBuf},
};

pub(crate) const FRAME_HEADER_BYTES: usize = 88;
type DigestV1 = [u8; 32];

/// Fixed owner-selected frame format; never selected by an on-disk record.
#[derive(Clone, Copy)]
pub(crate) struct PrivateJournalFormat {
    pub(crate) filename: &'static str,
    pub(crate) magic: &'static [u8; 8],
    pub(crate) hash_domain: &'static [u8],
    pub(crate) maximum_payload_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum PrivateJournalError {
    #[error("private journal storage unavailable")]
    StorageUnavailable,
    #[error("private journal is already open")]
    AlreadyOpen,
    #[error("private journal integrity failed")]
    Corrupt,
    #[error("private journal durability is uncertain")]
    Uncertain,
}

pub(crate) struct PrivateJournal {
    format: PrivateJournalFormat,
    directory_path: PathBuf,
    directory: File,
    journal: File,
    file_identity: (u64, u64),
    observed_version: JournalFileVersion,
    acknowledged_bytes: u64,
    read_bytes: u64,
    next_sequence: u64,
    previous_frame_hash: DigestV1,
    poisoned: Cell<bool>,
    #[cfg(test)]
    pub(crate) failure: Cell<Option<TestPersistenceFailure>>,
}

impl PrivateJournal {
    pub(crate) fn create_new(
        path: &Path,
        format: PrivateJournalFormat,
    ) -> Result<Self, PrivateJournalError> {
        validate_format(format)?;
        let parent_path = path.parent().ok_or(PrivateJournalError::Corrupt)?;
        let name = path.file_name().ok_or(PrivateJournalError::Corrupt)?;
        let parent = open_directory(parent_path)?;
        validate_directory(&parent.metadata().map_err(storage_error)?, false)?;
        rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700))
            .map_err(|_| PrivateJournalError::StorageUnavailable)?;
        let directory = File::from(
            rustix::fs::openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| PrivateJournalError::StorageUnavailable)?,
        );
        validate_directory(&directory.metadata().map_err(storage_error)?, true)?;
        let journal = File::from(
            rustix::fs::openat(
                &directory,
                format.filename,
                OFlags::RDWR
                    | OFlags::APPEND
                    | OFlags::CREATE
                    | OFlags::EXCL
                    | OFlags::NOFOLLOW
                    | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| PrivateJournalError::StorageUnavailable)?,
        );
        let store = Self::locked(path, directory, journal, format)?;
        // Make the empty inode and directory durable before the owner appends Initialize.
        // A crash here leaves an invalid empty store, never an implicitly fresh wallet.
        if store
            .journal
            .sync_all()
            .and_then(|()| store.directory.sync_all())
            .and_then(|()| parent.sync_all())
            .is_err()
        {
            return Err(PrivateJournalError::Uncertain);
        }
        store.check_owned()?;
        Ok(store)
    }

    pub(crate) fn open_existing(
        path: &Path,
        format: PrivateJournalFormat,
    ) -> Result<Self, PrivateJournalError> {
        validate_format(format)?;
        let directory = open_directory(path)?;
        validate_directory(&directory.metadata().map_err(storage_error)?, true)?;
        let journal = File::from(
            rustix::fs::openat(
                &directory,
                format.filename,
                OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| PrivateJournalError::StorageUnavailable)?,
        );
        Self::locked(path, directory, journal, format)
    }

    fn locked(
        path: &Path,
        directory: File,
        mut journal: File,
        format: PrivateJournalFormat,
    ) -> Result<Self, PrivateJournalError> {
        let metadata = journal.metadata().map_err(storage_error)?;
        validate_journal(&metadata)?;
        match journal.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(PrivateJournalError::AlreadyOpen),
            Err(TryLockError::Error(_)) => return Err(PrivateJournalError::StorageUnavailable),
        }
        journal.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let store = Self {
            format,
            directory_path: path.to_path_buf(),
            directory,
            journal,
            file_identity: identity(&metadata),
            observed_version: JournalFileVersion::from_metadata(&metadata),
            acknowledged_bytes: metadata.len(),
            read_bytes: 0,
            next_sequence: 0,
            previous_frame_hash: [0; 32],
            poisoned: Cell::new(false),
            #[cfg(test)]
            failure: Cell::new(None),
        };
        store.check_owned()?;
        Ok(store)
    }

    /// Return the next exact payload and its sequence while replaying the acknowledged prefix.
    /// The caller validates its schema and semantics; no append is allowed before full replay.
    pub(crate) fn replay_next(&mut self) -> Result<Option<(u64, Vec<u8>)>, PrivateJournalError> {
        if self.read_bytes == self.acknowledged_bytes {
            self.check_owned()?;
            if self.next_sequence == 0 {
                return Err(PrivateJournalError::Corrupt);
            }
            return Ok(None);
        }
        let mut header = [0_u8; FRAME_HEADER_BYTES];
        self.journal
            .read_exact(&mut header)
            .map_err(|_| PrivateJournalError::Corrupt)?;
        let length = u64::from_le_bytes(
            header[8..16]
                .try_into()
                .map_err(|_| PrivateJournalError::Corrupt)?,
        );
        let sequence = u64::from_le_bytes(
            header[16..24]
                .try_into()
                .map_err(|_| PrivateJournalError::Corrupt)?,
        );
        let remaining = self
            .acknowledged_bytes
            .saturating_sub(self.read_bytes)
            .saturating_sub(FRAME_HEADER_BYTES as u64);
        if &header[..8] != self.format.magic
            || sequence != self.next_sequence
            || header[24..56] != self.previous_frame_hash
            || length == 0
            || length > self.format.maximum_payload_bytes
            || length > remaining
        {
            return Err(PrivateJournalError::Corrupt);
        }
        let mut payload =
            vec![0; usize::try_from(length).map_err(|_| PrivateJournalError::Corrupt)?];
        self.journal
            .read_exact(&mut payload)
            .map_err(|_| PrivateJournalError::Corrupt)?;
        let hash = self.frame_hash(&header[..56], &payload);
        if header[56..] != hash {
            return Err(PrivateJournalError::Corrupt);
        }
        self.read_bytes = self
            .read_bytes
            .checked_add(FRAME_HEADER_BYTES as u64 + length)
            .ok_or(PrivateJournalError::Corrupt)?;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(PrivateJournalError::Corrupt)?;
        self.previous_frame_hash = hash;
        Ok(Some((sequence, payload)))
    }

    fn frame_hash(&self, header: &[u8], payload: &[u8]) -> DigestV1 {
        let mut hash = Sha256::new();
        hash.update(self.format.hash_domain);
        hash.update(header);
        hash.update(payload);
        hash.finalize().into()
    }

    #[cfg(test)]
    pub(crate) fn observed_version(&self) -> JournalFileVersion {
        self.observed_version
    }
    pub(crate) fn check_owned(&self) -> Result<(), PrivateJournalError> {
        if self.poisoned.get() {
            return Err(PrivateJournalError::Uncertain);
        }
        let result = self.inspect_owned(self.acknowledged_bytes, self.observed_version);
        if result.is_err() {
            self.poisoned.set(true);
        }
        result
    }

    fn inspect_owned(
        &self,
        expected_bytes: u64,
        expected_version: JournalFileVersion,
    ) -> Result<(), PrivateJournalError> {
        let directory = open_directory(&self.directory_path)?;
        let current_directory = directory.metadata().map_err(storage_error)?;
        validate_directory(&current_directory, true)?;
        if identity(&current_directory)
            != identity(&self.directory.metadata().map_err(storage_error)?)
        {
            return Err(PrivateJournalError::Corrupt);
        }
        let named = File::from(
            rustix::fs::openat(
                &self.directory,
                self.format.filename,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| PrivateJournalError::Corrupt)?,
        );
        let metadata = self.journal.metadata().map_err(storage_error)?;
        let named_metadata = named.metadata().map_err(storage_error)?;
        validate_journal(&metadata)?;
        validate_journal(&named_metadata)?;
        if identity(&metadata) != self.file_identity
            || identity(&named_metadata) != self.file_identity
            || metadata.len() != expected_bytes
            || named_metadata.len() != expected_bytes
            || JournalFileVersion::from_metadata(&metadata) != expected_version
            || JournalFileVersion::from_metadata(&named_metadata) != expected_version
        {
            return Err(PrivateJournalError::Corrupt);
        }
        Ok(())
    }

    pub(crate) fn append(&mut self, payload: &[u8]) -> Result<(), PrivateJournalError> {
        self.check_owned()?;
        if self.read_bytes != self.acknowledged_bytes {
            return Err(PrivateJournalError::Corrupt);
        }
        let length = u64::try_from(payload.len()).map_err(|_| PrivateJournalError::Corrupt)?;
        if length == 0 || length > self.format.maximum_payload_bytes {
            return Err(PrivateJournalError::Corrupt);
        }
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(PrivateJournalError::Corrupt)?;
        let mut header = [0_u8; FRAME_HEADER_BYTES];
        header[..8].copy_from_slice(self.format.magic);
        header[8..16].copy_from_slice(&length.to_le_bytes());
        header[16..24].copy_from_slice(&self.next_sequence.to_le_bytes());
        header[24..56].copy_from_slice(&self.previous_frame_hash);
        let hash = self.frame_hash(&header[..56], payload);
        header[56..].copy_from_slice(&hash);
        let next_bytes = self
            .acknowledged_bytes
            .checked_add(FRAME_HEADER_BYTES as u64)
            .and_then(|bytes| bytes.checked_add(length))
            .ok_or(PrivateJournalError::Corrupt)?;
        let written_version = match self.write_frame(&header, payload) {
            Ok(version) => version,
            Err(_) => {
                self.poisoned.set(true);
                return Err(PrivateJournalError::Uncertain);
            }
        };
        // A renamed/truncated/replaced path or another writer's edit during sync must not
        // turn a successful descriptor fsync into acknowledgment of a different named journal.
        if self.inspect_owned(next_bytes, written_version).is_err() {
            self.poisoned.set(true);
            return Err(PrivateJournalError::Uncertain);
        }
        self.observed_version = written_version;
        self.acknowledged_bytes = next_bytes;
        self.read_bytes = next_bytes;
        self.previous_frame_hash = hash;
        self.next_sequence = next_sequence;
        Ok(())
    }

    fn write_frame(
        &mut self,
        header: &[u8],
        payload: &[u8],
    ) -> std::io::Result<JournalFileVersion> {
        #[cfg(test)]
        if self.failure.get() == Some(TestPersistenceFailure::PartialWrite) {
            self.journal.write_all(&header[..11])?;
            return Err(std::io::Error::other("injected partial journal write"));
        }
        self.journal.write_all(header)?;
        self.journal.write_all(payload)?;
        #[cfg(test)]
        if self.failure.get() == Some(TestPersistenceFailure::BeforeSync) {
            return Err(std::io::Error::other("injected journal sync failure"));
        }
        let written_version = JournalFileVersion::from_metadata(&self.journal.metadata()?);
        self.journal.sync_all()?;
        #[cfg(test)]
        if self.failure.get() == Some(TestPersistenceFailure::AfterSync) {
            return Err(std::io::Error::other(
                "injected failure after durable journal sync",
            ));
        }
        #[cfg(test)]
        match self.failure.get() {
            Some(TestPersistenceFailure::ReplaceAfterSync) => {
                let path = self.directory_path.join(self.format.filename);
                let displaced = self.directory_path.join("displaced-test.wal");
                std::fs::rename(&path, &displaced)?;
                std::fs::copy(&displaced, &path)?;
            }
            Some(TestPersistenceFailure::TruncateAfterSync) => self.journal.set_len(0)?,
            _ => {}
        }
        Ok(written_version)
    }
}

fn validate_format(format: PrivateJournalFormat) -> Result<(), PrivateJournalError> {
    let mut components = Path::new(format.filename).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || format.maximum_payload_bytes == 0
        || format.hash_domain.is_empty()
    {
        return Err(PrivateJournalError::Corrupt);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct JournalFileVersion {
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl JournalFileVersion {
    pub(crate) fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

fn storage_error(_: std::io::Error) -> PrivateJournalError {
    PrivateJournalError::StorageUnavailable
}

fn identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn validate_directory(metadata: &Metadata, private: bool) -> Result<(), PrivateJournalError> {
    if !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || (if private {
            metadata.mode() & 0o777 != 0o700
        } else {
            metadata.mode() & 0o022 != 0
        })
    {
        return Err(PrivateJournalError::Corrupt);
    }
    Ok(())
}

fn validate_journal(metadata: &Metadata) -> Result<(), PrivateJournalError> {
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(PrivateJournalError::Corrupt);
    }
    Ok(())
}

fn open_directory(path: &Path) -> Result<File, PrivateJournalError> {
    if !path.is_absolute() {
        return Err(PrivateJournalError::Corrupt);
    }
    let mut directory = File::from(
        rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| PrivateJournalError::StorageUnavailable)?,
    );
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = File::from(
                    rustix::fs::openat(
                        &directory,
                        name,
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|_| PrivateJournalError::Corrupt)?,
                );
            }
            _ => return Err(PrivateJournalError::Corrupt),
        }
    }
    Ok(directory)
}

#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TestPersistenceFailure {
    PartialWrite,
    BeforeSync,
    AfterSync,
    ReplaceAfterSync,
    TruncateAfterSync,
}
