//! Private descriptor-owned append-only journals shared by offline Core stores.
//!
//! This layer owns durable bytes, not monetary authority. Record owners supply their own exact
//! Norito schema and authenticate recovered state against current hardware before using it.

use iroha_fs::{FileIdentity, FileSnapshot, OwnerDirectory, PrivateDirectory};
use sha2::{Digest as _, Sha256};

use std::{
    cell::Cell,
    fs::{File, TryLockError},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};
use zeroize::Zeroizing;

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
    directory: PrivateDirectory,
    journal: File,
    file_identity: FileIdentity,
    observed_version: JournalFileVersion,
    acknowledged_bytes: u64,
    read_bytes: u64,
    next_sequence: u64,
    previous_frame_hash: DigestV1,
    // One verified immutable prefix, scoped to this held descriptor and invalidated by poison.
    verified_recovery_prefix: Cell<Option<super::KagemushaRecoveryJournalPrefixV1>>,
    poisoned: Cell<bool>,
    // A consumer cannot recursively materialize another record through this same owner.
    scanning: Cell<bool>,
    #[cfg(test)]
    pub(crate) failure: Cell<Option<TestPersistenceFailure>>,
}

/// Bounded positional data replay under one already authenticated complete owned prefix.
/// Its fields and constructor are private; it grants byte custody, never financial authority.
pub(crate) struct PrivateJournalReplayCursor {
    prefix: super::KagemushaRecoveryJournalPrefixV1,
    file_identity: FileIdentity,
    offset: u64,
    sequence: u64,
    previous: DigestV1,
    failed: bool,
}

impl PrivateJournalReplayCursor {
    /// Actual frame boundary consumed by this cursor, absent before the first complete frame.
    pub(crate) fn consumed_prefix(&self) -> Option<super::KagemushaRecoveryJournalPrefixV1> {
        (self.sequence != 0 && !self.failed).then_some(super::KagemushaRecoveryJournalPrefixV1 {
            sequence: self.sequence,
            head: self.previous,
            byte_len: self.offset,
        })
    }
}

impl PrivateJournal {
    /// Start bounded semantic replay only after every physical frame has been authenticated.
    /// Exact offset reads retain one payload at a time; each write reanchors the acknowledged tail.
    pub(crate) fn replay_cursor(&self) -> Result<PrivateJournalReplayCursor, PrivateJournalError> {
        let prefix = self.recovery_prefix()?;
        Ok(PrivateJournalReplayCursor {
            prefix,
            file_identity: self.file_identity,
            offset: 0,
            sequence: 0,
            previous: [0; 32],
            failed: false,
        })
    }

    /// Read one exact frame under the cursor's same inode and complete immutable prefix.
    /// Valid appended suffixes retire this cursor; replacement/corruption still poisons storage.
    /// No caller callback or semantic decoder can change this cursor's framing or own its bytes.
    pub(crate) fn read_cursor_next(
        &self,
        cursor: &mut PrivateJournalReplayCursor,
    ) -> Result<Option<(u64, Zeroizing<Vec<u8>>)>, PrivateJournalError> {
        if cursor.failed || cursor.file_identity != self.file_identity {
            return Err(PrivateJournalError::Corrupt);
        }
        let result = (|| {
            if self.recovery_prefix()? != cursor.prefix {
                return Err(PrivateJournalError::Corrupt);
            }
            if cursor.sequence == cursor.prefix.sequence {
                if cursor.offset != cursor.prefix.byte_len || cursor.previous != cursor.prefix.head
                {
                    return Err(PrivateJournalError::Corrupt);
                }
                return Ok(None);
            }
            if cursor.sequence > cursor.prefix.sequence
                || cursor.prefix.byte_len.saturating_sub(cursor.offset) < FRAME_HEADER_BYTES as u64
            {
                return Err(PrivateJournalError::Corrupt);
            }
            let mut header = [0; FRAME_HEADER_BYTES];
            iroha_fs::read_exact_at(&self.journal, &mut header, cursor.offset)
                .map_err(storage_error)?;
            let parsed =
                validate_frame_header(&header, self.format, cursor.sequence, cursor.previous)?;
            let payload_offset = cursor
                .offset
                .checked_add(FRAME_HEADER_BYTES as u64)
                .ok_or(PrivateJournalError::Corrupt)?;
            if parsed.length > cursor.prefix.byte_len.saturating_sub(payload_offset) {
                return Err(PrivateJournalError::Corrupt);
            }
            let length =
                usize::try_from(parsed.length).map_err(|_| PrivateJournalError::Corrupt)?;
            let mut payload = Zeroizing::new(Vec::new());
            payload
                .try_reserve_exact(length)
                .map_err(|_| PrivateJournalError::StorageUnavailable)?;
            payload.resize(length, 0);
            iroha_fs::read_exact_at(&self.journal, &mut payload, payload_offset)
                .map_err(storage_error)?;
            let actual = self.frame_hash(&header[..56], &payload);
            if actual != parsed.hash || self.recovery_prefix()? != cursor.prefix {
                return Err(PrivateJournalError::Corrupt);
            }
            let sequence = cursor.sequence;
            cursor.offset = payload_offset
                .checked_add(parsed.length)
                .ok_or(PrivateJournalError::Corrupt)?;
            cursor.sequence = cursor
                .sequence
                .checked_add(1)
                .ok_or(PrivateJournalError::Corrupt)?;
            cursor.previous = actual;
            Ok(Some((sequence, payload)))
        })();
        if result.is_err() {
            cursor.failed = true;
            if self.recovery_prefix().ok() == Some(cursor.prefix) {
                self.poisoned.set(true);
                self.verified_recovery_prefix.set(None);
            }
        }
        result
    }

    pub(crate) fn create_new(
        path: &Path,
        format: PrivateJournalFormat,
    ) -> Result<Self, PrivateJournalError> {
        validate_format(format)?;
        let parent_path = path.parent().ok_or(PrivateJournalError::Corrupt)?;
        let name = path.file_name().ok_or(PrivateJournalError::Corrupt)?;
        if !parent_path.is_absolute() {
            return Err(PrivateJournalError::Corrupt);
        }
        #[cfg(unix)]
        if parent_path.canonicalize().map_err(storage_error)? != parent_path {
            return Err(PrivateJournalError::Corrupt);
        }
        let parent = OwnerDirectory::open(parent_path).map_err(storage_error)?;
        let directory = parent.create_private_child(name).map_err(storage_error)?;
        let journal = directory
            .create_lock(format.filename)
            .map_err(storage_error)?;
        let store = Self::locked(path, directory, journal, format)?;
        // Make the empty inode and directory durable before the owner appends Initialize.
        // A crash here leaves an invalid empty store, never an implicitly fresh wallet.
        if store
            .journal
            .sync_all()
            .and_then(|()| store.directory.sync())
            .and_then(|()| parent.sync())
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
        if !path.is_absolute() {
            return Err(PrivateJournalError::Corrupt);
        }
        #[cfg(unix)]
        if path.canonicalize().map_err(storage_error)? != path {
            return Err(PrivateJournalError::Corrupt);
        }
        let parent = OwnerDirectory::open(path.parent().ok_or(PrivateJournalError::Corrupt)?)
            .map_err(storage_error)?;
        let directory = PrivateDirectory::open(path).map_err(storage_error)?;
        let journal = directory
            .open_existing_lock(format.filename)
            .map_err(storage_error)?;
        let store = Self::locked(path, directory, journal, format)?;
        // Adopt surviving complete frames durably before replay may expose a recovery prefix.
        // A previous writer may have exited after a write but before its fsync acknowledgement.
        if store
            .journal
            .sync_all()
            .and_then(|()| store.directory.sync())
            .and_then(|()| parent.sync())
            .is_err()
        {
            return Err(PrivateJournalError::Uncertain);
        }
        store.check_owned()?;
        Ok(store)
    }

    fn locked(
        path: &Path,
        directory: PrivateDirectory,
        mut journal: File,
        format: PrivateJournalFormat,
    ) -> Result<Self, PrivateJournalError> {
        let metadata = journal.metadata().map_err(storage_error)?;
        FileSnapshot::private_journal(&journal).map_err(storage_error)?;
        match journal.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err(PrivateJournalError::AlreadyOpen),
            Err(TryLockError::Error(_)) => return Err(PrivateJournalError::StorageUnavailable),
        }
        journal.seek(SeekFrom::Start(0)).map_err(storage_error)?;
        let file_identity = FileIdentity::of(&journal).map_err(storage_error)?;
        let observed_version = JournalFileVersion::from_file(&journal).map_err(storage_error)?;
        let store = Self {
            format,
            directory_path: path.to_path_buf(),
            directory,
            journal,
            file_identity,
            observed_version,
            acknowledged_bytes: metadata.len(),
            read_bytes: 0,
            next_sequence: 0,
            previous_frame_hash: [0; 32],
            verified_recovery_prefix: Cell::new(None),
            poisoned: Cell::new(false),
            scanning: Cell::new(false),
            #[cfg(test)]
            failure: Cell::new(None),
        };
        store.check_owned()?;
        Ok(store)
    }

    /// Return the original native path only while this descriptor still owns that exact inode.
    /// A path projection is a locator, not authenticated state or monetary authority.
    pub(crate) fn original_directory(&self) -> Result<PathBuf, PrivateJournalError> {
        self.check_owned()?;
        Ok(self.directory_path.clone())
    }

    /// Return the next exact payload and its sequence while replaying the acknowledged prefix.
    /// The caller validates its schema and semantics; no append is allowed before full replay.
    /// Error paths erase the partial payload. A successful caller owns its disposal and must
    /// retain secret-bearing payloads in its zeroizing record/custody type.
    pub(crate) fn replay_next(&mut self) -> Result<Option<(u64, Vec<u8>)>, PrivateJournalError> {
        if self.read_bytes == self.acknowledged_bytes {
            self.check_owned()?;
            if self.next_sequence == 0 {
                return Err(PrivateJournalError::Corrupt);
            }
            return Ok(None);
        }
        // Other genuine exact-offset reads may move Windows' physical cursor. Replay is
        // selected only by this owner's acknowledged logical offset, never that cursor.
        self.journal
            .seek(SeekFrom::Start(self.read_bytes))
            .map_err(|_| PrivateJournalError::Corrupt)?;
        let mut header = [0_u8; FRAME_HEADER_BYTES];
        self.journal
            .read_exact(&mut header)
            .map_err(|_| PrivateJournalError::Corrupt)?;
        let sequence = self.next_sequence;
        let parsed =
            validate_frame_header(&header, self.format, sequence, self.previous_frame_hash)?;
        let length = parsed.length;
        let remaining = self
            .acknowledged_bytes
            .saturating_sub(self.read_bytes)
            .saturating_sub(FRAME_HEADER_BYTES as u64);
        if length > remaining {
            return Err(PrivateJournalError::Corrupt);
        }
        let mut payload = Zeroizing::new(vec![
            0;
            usize::try_from(length)
                .map_err(|_| PrivateJournalError::Corrupt)?
        ]);
        self.journal
            .read_exact(&mut payload)
            .map_err(|_| PrivateJournalError::Corrupt)?;
        let hash = self.frame_hash(&header[..56], &payload);
        if parsed.hash != hash {
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
        Ok(Some((sequence, std::mem::take(&mut *payload))))
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
    /// The fully replayed durable prefix of the held production journal.
    pub(crate) fn recovery_prefix(
        &self,
    ) -> Result<super::KagemushaRecoveryJournalPrefixV1, PrivateJournalError> {
        self.check_owned()?;
        if self.read_bytes != self.acknowledged_bytes || self.next_sequence == 0 {
            return Err(PrivateJournalError::Corrupt);
        }
        Ok(super::KagemushaRecoveryJournalPrefixV1 {
            sequence: self.next_sequence,
            head: self.previous_frame_hash,
            byte_len: self.acknowledged_bytes,
        })
    }

    /// Require the selected frame boundary to occur in this actual owned, fully replayed WAL.
    /// A validated append-only suffix is permitted; this does not authenticate hardware selection.
    /// Exact-offset reads leave the logical replay/append selection unchanged. At most one verified prefix is
    /// retained, and even a cached match requires the existing descriptor/generation checks.
    pub(crate) fn contains_recovery_prefix(
        &self,
        expected: super::KagemushaRecoveryJournalPrefixV1,
    ) -> Result<bool, PrivateJournalError> {
        let current = self.recovery_prefix()?;
        if expected.sequence == 0
            || expected.head == [0; 32]
            || expected.byte_len == 0
            || expected.sequence > current.sequence
            || expected.byte_len > current.byte_len
        {
            return Ok(false);
        }
        if expected == current || self.verified_recovery_prefix.get() == Some(expected) {
            self.verified_recovery_prefix.set(Some(expected));
            return Ok(true);
        }
        let result = self.scan_recovery_prefix(expected);
        // Never retain a successful scan across file replacement, edits, or storage failure.
        self.check_owned()?;
        match result {
            Ok(true) => {
                self.verified_recovery_prefix.set(Some(expected));
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(error) => {
                self.poisoned.set(true);
                Err(error)
            }
        }
    }

    fn scan_recovery_prefix(
        &self,
        expected: super::KagemushaRecoveryJournalPrefixV1,
    ) -> Result<bool, PrivateJournalError> {
        let mut offset = 0_u64;
        let mut previous = [0; 32];
        let mut buffer = Zeroizing::new([0_u8; 8192]);
        for sequence in 0..expected.sequence {
            if expected.byte_len.saturating_sub(offset) < FRAME_HEADER_BYTES as u64 {
                return Ok(false);
            }
            let mut header = [0_u8; FRAME_HEADER_BYTES];
            iroha_fs::read_exact_at(&self.journal, &mut header, offset).map_err(storage_error)?;
            let parsed = validate_frame_header(&header, self.format, sequence, previous)?;
            let length = parsed.length;
            offset = offset
                .checked_add(FRAME_HEADER_BYTES as u64)
                .ok_or(PrivateJournalError::Corrupt)?;
            if length > expected.byte_len.saturating_sub(offset) {
                return Ok(false);
            }
            let mut hash = Sha256::new();
            hash.update(self.format.hash_domain);
            hash.update(&header[..56]);
            let mut remaining = length;
            while remaining != 0 {
                let count = remaining.min(buffer.len() as u64) as usize;
                iroha_fs::read_exact_at(&self.journal, &mut buffer[..count], offset)
                    .map_err(storage_error)?;
                hash.update(&buffer[..count]);
                offset = offset
                    .checked_add(count as u64)
                    .ok_or(PrivateJournalError::Corrupt)?;
                remaining -= count as u64;
            }
            let actual: DigestV1 = hash.finalize().into();
            if parsed.hash != actual {
                return Err(PrivateJournalError::Corrupt);
            }
            previous = actual;
        }
        Ok(offset == expected.byte_len && previous == expected.head)
    }

    /// Recheck a single immutable snapshot record through the held descriptor.
    /// This bounded byte comparison grants no snapshot or hardware authority.
    pub(crate) fn require_single_record(&self, expected: &[u8]) -> Result<(), PrivateJournalError> {
        let result = (|| {
            let prefix = self.recovery_prefix()?;
            if prefix.sequence != 1
                || expected.is_empty()
                || expected.len() as u64 > self.format.maximum_payload_bytes
                || prefix.byte_len != FRAME_HEADER_BYTES as u64 + expected.len() as u64
            {
                return Err(PrivateJournalError::Corrupt);
            }
            let mut header = [0_u8; FRAME_HEADER_BYTES];
            iroha_fs::read_exact_at(&self.journal, &mut header, 0).map_err(storage_error)?;
            let parsed = validate_frame_header(&header, self.format, 0, [0; 32])?;
            if parsed.length != expected.len() as u64 || parsed.hash != prefix.head {
                return Err(PrivateJournalError::Corrupt);
            }
            let mut digest = Sha256::new();
            digest.update(self.format.hash_domain);
            digest.update(&header[..56]);
            let mut buffer = Zeroizing::new([0_u8; 8192]);
            let mut offset = 0_usize;
            while offset < expected.len() {
                let count = buffer.len().min(expected.len() - offset);
                iroha_fs::read_exact_at(
                    &self.journal,
                    &mut buffer[..count],
                    FRAME_HEADER_BYTES as u64 + offset as u64,
                )
                .map_err(storage_error)?;
                if buffer[..count] != expected[offset..offset + count] {
                    return Err(PrivateJournalError::Corrupt);
                }
                digest.update(&buffer[..count]);
                offset += count;
            }
            let actual: DigestV1 = digest.finalize().into();
            if actual != parsed.hash {
                return Err(PrivateJournalError::Corrupt);
            }
            self.check_owned()
        })();
        if result.is_err() {
            self.poisoned.set(true);
            self.verified_recovery_prefix.set(None);
        }
        result
    }

    /// Visit every complete record through this owner's original locked descriptor.
    ///
    /// The journal must already be fully replayed. The complete end/head are captured once;
    /// every original frame, including any unselected suffix, is checked again. Positional
    /// reads preserve the file offset and all replay/append counters. Existing named-path
    /// probes only verify ownership; they never supply replay bytes or acquire another lock.
    ///
    /// The borrowed callback is data plumbing, not an authentication decision or capability.
    /// Payloads remain owner-specific untrusted bytes; their canonical Norito schema and any
    /// signatures, selected hardware prefix or suffix semantics require a separate verifier.
    /// At most one payload of the format's maximum size is allocated by this scan; allocations
    /// performed by the callback are outside that bound. Recursive scans on this owner fail
    /// before allocating or reading another record.
    ///
    /// # Errors
    /// Incomplete replay or reentrancy refuses admission without poisoning. Once admitted,
    /// read/framing/ownership errors, callback errors and unwinding poison the owner and clear
    /// its cached prefix. Success leaves it usable. No path initializes, appends, fsyncs,
    /// truncates or retires journal records; the caller must discard partial callback results.
    // Concrete incoming/outgoing recovery owners authenticate each original schema and proof;
    // this shared scan grants byte custody only.
    pub(crate) fn scan_complete(
        &self,
        mut consume: impl FnMut(u64, &[u8]) -> Result<(), PrivateJournalError>,
    ) -> Result<super::KagemushaRecoveryJournalPrefixV1, PrivateJournalError> {
        if self.scanning.get() {
            return Err(PrivateJournalError::Corrupt);
        }
        let expected = self.recovery_prefix()?;
        self.scanning.set(true);
        let mut lease = CompleteScanLease {
            journal: self,
            complete: false,
        };
        let mut offset = 0_u64;
        let mut previous = [0; 32];
        for sequence in 0..expected.sequence {
            self.check_owned()?;
            if expected.byte_len.saturating_sub(offset) < FRAME_HEADER_BYTES as u64 {
                return Err(PrivateJournalError::Corrupt);
            }
            let mut header = [0_u8; FRAME_HEADER_BYTES];
            iroha_fs::read_exact_at(&self.journal, &mut header, offset).map_err(storage_error)?;
            let parsed = validate_frame_header(&header, self.format, sequence, previous)?;
            let payload_offset = offset
                .checked_add(FRAME_HEADER_BYTES as u64)
                .ok_or(PrivateJournalError::Corrupt)?;
            if parsed.length > expected.byte_len.saturating_sub(payload_offset) {
                return Err(PrivateJournalError::Corrupt);
            }
            let length =
                usize::try_from(parsed.length).map_err(|_| PrivateJournalError::Corrupt)?;
            // Ordinary preparation records include the Native financial seed. Erase this
            // temporary complete-scan copy on success, callback failure and unwinding alike.
            let mut payload = Zeroizing::new(Vec::new());
            payload
                .try_reserve_exact(length)
                .map_err(|_| PrivateJournalError::StorageUnavailable)?;
            payload.resize(length, 0);
            iroha_fs::read_exact_at(&self.journal, &mut payload, payload_offset)
                .map_err(storage_error)?;
            let actual = self.frame_hash(&header[..56], &payload);
            if actual != parsed.hash {
                return Err(PrivateJournalError::Corrupt);
            }
            // Never deliver bytes read across an externally changed file generation.
            self.check_owned()?;
            let result = consume(sequence, &payload);
            // This check runs even for an ordinary callback error. Unwinding drops the lease,
            // which poisons and invalidates the owner before any later method can acknowledge it.
            self.check_owned()?;
            result?;
            offset = payload_offset
                .checked_add(parsed.length)
                .ok_or(PrivateJournalError::Corrupt)?;
            previous = actual;
        }
        // A callback may own a destructor that mutates storage or unwinds. Complete it
        // while the scan lease is still armed, before the final ownership check.
        drop(consume);
        if offset != expected.byte_len
            || previous != expected.head
            || self.recovery_prefix()? != expected
        {
            return Err(PrivateJournalError::Corrupt);
        }
        lease.complete = true;
        Ok(expected)
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
        self.directory.revalidate().map_err(storage_error)?;
        let current_directory =
            PrivateDirectory::open(&self.directory_path).map_err(storage_error)?;
        if current_directory.identity().map_err(storage_error)?
            != self.directory.identity().map_err(storage_error)?
        {
            return Err(PrivateJournalError::Corrupt);
        }
        let named = self
            .directory
            .open_read(self.format.filename)
            .map_err(storage_error)?;
        let metadata = self.journal.metadata().map_err(storage_error)?;
        let named_metadata = named.metadata().map_err(storage_error)?;
        if FileIdentity::of(&self.journal).map_err(storage_error)? != self.file_identity
            || FileIdentity::of(&named).map_err(storage_error)? != self.file_identity
            || metadata.len() != expected_bytes
            || named_metadata.len() != expected_bytes
            || JournalFileVersion::from_file(&self.journal).map_err(storage_error)?
                != expected_version
            || JournalFileVersion::from_file(&named).map_err(storage_error)? != expected_version
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
        // The exact locked original cursor is positioned at the acknowledged tail. Portable
        // read/write handles cannot overwrite an earlier record or accept a foreign suffix.
        if self.journal.seek(SeekFrom::End(0))? != self.acknowledged_bytes {
            return Err(std::io::Error::other(
                "private journal tail changed before append",
            ));
        }
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
        let written_version = JournalFileVersion::from_file(&self.journal)?;
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

// Completion is deliberately local to one scan, not a retained authentication receipt.
struct CompleteScanLease<'a> {
    journal: &'a PrivateJournal,
    complete: bool,
}
impl Drop for CompleteScanLease<'_> {
    fn drop(&mut self) {
        if !self.complete {
            self.journal.poisoned.set(true);
            self.journal.verified_recovery_prefix.set(None);
        }
        self.journal.scanning.set(false);
    }
}

struct ValidatedFrameHeader {
    length: u64,
    hash: DigestV1,
}

// All replay paths share the same fixed frame grammar; owners still decode their own payloads.
fn validate_frame_header(
    header: &[u8; FRAME_HEADER_BYTES],
    format: PrivateJournalFormat,
    expected_sequence: u64,
    previous_hash: DigestV1,
) -> Result<ValidatedFrameHeader, PrivateJournalError> {
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
    if &header[..8] != format.magic
        || sequence != expected_sequence
        || header[24..56] != previous_hash
        || length == 0
        || length > format.maximum_payload_bytes
    {
        return Err(PrivateJournalError::Corrupt);
    }
    Ok(ValidatedFrameHeader {
        length,
        hash: header[56..88]
            .try_into()
            .map_err(|_| PrivateJournalError::Corrupt)?,
    })
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
pub(crate) struct JournalFileVersion(FileSnapshot);
impl JournalFileVersion {
    pub(crate) fn from_file(file: &File) -> std::io::Result<Self> {
        FileSnapshot::private_journal(file).map(Self)
    }
}

fn storage_error(_: std::io::Error) -> PrivateJournalError {
    PrivateJournalError::StorageUnavailable
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
        filename: "test.wal",
        magic: b"IKGTEST1",
        hash_domain: b"test-only:private-journal\0",
        maximum_payload_bytes: 1024,
    };

    #[test]
    fn fifo_replacement_cannot_block_owned_inspection_or_reopen() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("journal");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        journal.append(b"initialize").unwrap();
        std::fs::remove_file(path.join(FORMAT.filename)).unwrap();
        // Spawning a utility can transiently inherit a parallel test's flock before
        // close-on-exec, making that journal appear open after its owner has dropped.
        #[cfg(target_vendor = "apple")]
        #[allow(unsafe_code)]
        // rustix does not expose mkfifoat on Apple; this test owns the path.
        {
            use std::os::unix::ffi::OsStrExt as _;
            unsafe extern "C" {
                // Apple's SDK declares mode_t as __darwin_mode_t (u16).
                fn mkfifo(path: *const std::ffi::c_char, mode: u16) -> std::ffi::c_int;
            }
            let fifo =
                std::ffi::CString::new(path.join(FORMAT.filename).as_os_str().as_bytes()).unwrap();
            // SAFETY: the NUL-terminated path lives through the call; the exact Apple
            // POSIX signature creates only this task-owned test FIFO, without a child.
            assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
        }
        #[cfg(not(target_vendor = "apple"))]
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            path.join(FORMAT.filename),
            Mode::from_raw_mode(0o600),
        )
        .expect("create an actual FIFO without inheriting other journals");
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            assert!(journal.check_owned().is_err());
            drop(journal);
            assert!(PrivateJournal::open_existing(&path, FORMAT).is_err());
            send.send(()).unwrap();
        });
        receive
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("FIFO without a writer must be rejected without waiting for bytes");
    }

    #[test]
    fn reopen_adopts_complete_unacknowledged_frame_and_retains_exact_prefix() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("journal");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        journal.append(b"initialize").unwrap();
        let initial = journal.recovery_prefix().unwrap();
        journal
            .failure
            .set(Some(TestPersistenceFailure::BeforeSync));
        assert_eq!(
            journal.append(b"retained complete frame"),
            Err(PrivateJournalError::Uncertain)
        );
        assert_eq!(
            journal.recovery_prefix(),
            Err(PrivateJournalError::Uncertain)
        );
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(
            reopened.replay_next().unwrap(),
            Some((0, b"initialize".to_vec()))
        );
        assert_eq!(
            reopened.replay_next().unwrap(),
            Some((1, b"retained complete frame".to_vec()))
        );
        assert_eq!(reopened.replay_next().unwrap(), None);
        let adopted = reopened.recovery_prefix().unwrap();
        assert_eq!(adopted.sequence, initial.sequence + 1);
        assert!(adopted.byte_len > initial.byte_len);
        assert_ne!(adopted.head, initial.head);
        drop(reopened);
        let mut again = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        while again.replay_next().unwrap().is_some() {}
        assert_eq!(again.recovery_prefix().unwrap(), adopted);
    }

    #[test]
    fn selected_prefix_ancestry_preserves_exact_boundaries_and_appended_suffixes() {
        let format = PrivateJournalFormat {
            maximum_payload_bytes: 32 * 1024,
            ..FORMAT
        };
        let selected_payload = vec![73; 2 * 8192 + 17];
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("journal");
        let mut journal = PrivateJournal::create_new(&path, format).unwrap();
        journal.append(b"initialize").unwrap();
        let initial = journal.recovery_prefix().unwrap();
        journal.append(&selected_payload).unwrap();
        let selected = journal.recovery_prefix().unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        journal.append(b"valid unselected suffix").unwrap();
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        assert!(journal.contains_recovery_prefix(initial).unwrap());
        drop(journal);
        let mut journal = PrivateJournal::open_existing(&path, format).unwrap();
        assert_eq!(
            journal.contains_recovery_prefix(selected),
            Err(PrivateJournalError::Corrupt)
        );
        while journal.replay_next().unwrap().is_some() {}
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        assert!(journal.contains_recovery_prefix(initial).unwrap());
        for field in 0..6 {
            let mut wrong = selected;
            match field {
                0 => wrong.sequence = 0,
                1 => wrong.sequence += 1,
                2 => wrong.byte_len -= 1,
                3 => wrong.byte_len += 1,
                4 => wrong.head[0] ^= 1,
                _ => wrong.byte_len = u64::MAX,
            }
            assert!(!journal.contains_recovery_prefix(wrong).unwrap());
        }
        // A mismatched selection is not storage corruption. Exact ancestry stays usable,
        // and positional validation neither consumes replay nor changes the append cursor.
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        journal.append(b"another valid suffix").unwrap();
        let extended = journal.recovery_prefix().unwrap();
        assert_eq!(extended.sequence, selected.sequence + 2);
        assert!(journal.contains_recovery_prefix(selected).unwrap());
        drop(journal);
        let mut journal = PrivateJournal::open_existing(&path, format).unwrap();
        let mut payloads = Vec::new();
        while let Some((_, payload)) = journal.replay_next().unwrap() {
            payloads.push(payload);
        }
        assert_eq!(
            payloads,
            vec![
                b"initialize".to_vec(),
                selected_payload,
                b"valid unselected suffix".to_vec(),
                b"another valid suffix".to_vec(),
            ]
        );
        assert_eq!(journal.recovery_prefix().unwrap(), extended);
        assert!(journal.contains_recovery_prefix(selected).unwrap());
    }

    #[test]
    fn positional_replay_requires_complete_physical_replay_and_preserves_append_cursor() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("positional");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        journal.append(b"initialize").unwrap();
        let first = journal.recovery_prefix().unwrap();
        journal.append(b"selected operation").unwrap();
        let complete = journal.recovery_prefix().unwrap();
        drop(journal);
        let mut journal = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(journal.replay_cursor().is_err());
        assert_eq!(journal.replay_next().unwrap().unwrap().1, b"initialize");
        assert!(journal.replay_cursor().is_err());
        assert_eq!(
            journal.replay_next().unwrap().unwrap().1,
            b"selected operation"
        );
        assert!(journal.replay_next().unwrap().is_none());
        let mut cursor = journal.replay_cursor().unwrap();
        assert!(cursor.consumed_prefix().is_none());
        let (sequence, raw) = journal.read_cursor_next(&mut cursor).unwrap().unwrap();
        assert_eq!(sequence, 0);
        assert_eq!(raw.as_slice(), b"initialize");
        assert_eq!(cursor.consumed_prefix(), Some(first));
        assert!(journal.contains_recovery_prefix(first).unwrap());
        let (sequence, raw) = journal.read_cursor_next(&mut cursor).unwrap().unwrap();
        assert_eq!(sequence, 1);
        assert_eq!(raw.as_slice(), b"selected operation");
        assert_eq!(cursor.consumed_prefix(), Some(complete));
        assert!(journal.read_cursor_next(&mut cursor).unwrap().is_none());
        assert_eq!(journal.recovery_prefix().unwrap(), complete);
        journal.append(b"valid suffix after replay").unwrap();
        assert_eq!(
            journal.recovery_prefix().unwrap().sequence,
            complete.sequence + 1
        );
    }

    #[test]
    fn positional_replay_refuses_other_inode_and_retires_after_valid_append() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap();
        let mut first = PrivateJournal::create_new(&path.join("first"), FORMAT).unwrap();
        let mut other = PrivateJournal::create_new(&path.join("other"), FORMAT).unwrap();
        first.append(b"same original").unwrap();
        other.append(b"same original").unwrap();
        assert_eq!(
            first.recovery_prefix().unwrap(),
            other.recovery_prefix().unwrap()
        );
        let mut cursor = first.replay_cursor().unwrap();
        assert!(other.read_cursor_next(&mut cursor).is_err());
        assert_eq!(
            first
                .read_cursor_next(&mut cursor)
                .unwrap()
                .unwrap()
                .1
                .as_slice(),
            b"same original"
        );
        let mut retired = first.replay_cursor().unwrap();
        first.append(b"genuine appended suffix").unwrap();
        assert!(first.read_cursor_next(&mut retired).is_err());
        assert!(first.read_cursor_next(&mut retired).is_err());
        assert!(retired.consumed_prefix().is_none());
        assert!(first.recovery_prefix().is_ok());
        let mut fresh = first.replay_cursor().unwrap();
        assert!(first.read_cursor_next(&mut fresh).unwrap().is_some());
        assert_eq!(
            first
                .read_cursor_next(&mut fresh)
                .unwrap()
                .unwrap()
                .1
                .as_slice(),
            b"genuine appended suffix"
        );
        assert!(first.read_cursor_next(&mut fresh).unwrap().is_none());
    }

    #[test]
    fn cached_selected_prefix_never_bypasses_owned_file_or_durability_checks() {
        for change in 0..4 {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().canonicalize().unwrap().join("journal");
            let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            journal.append(b"initialize").unwrap();
            let selected = journal.recovery_prefix().unwrap();
            assert!(journal.contains_recovery_prefix(selected).unwrap());
            journal.append(b"retained suffix").unwrap();
            assert!(journal.contains_recovery_prefix(selected).unwrap());
            let file = path.join(FORMAT.filename);
            match change {
                0 => {
                    let mut bytes = std::fs::read(&file).unwrap();
                    *bytes.last_mut().unwrap() ^= 1;
                    std::fs::write(&file, bytes).unwrap();
                }
                1 => {
                    let moved = path.join("displaced.wal");
                    std::fs::rename(&file, &moved).unwrap();
                    std::fs::copy(&moved, &file).unwrap();
                }
                2 => {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(&file)
                        .unwrap()
                        .set_len(selected.byte_len)
                        .unwrap();
                }
                _ => {
                    journal.failure.set(Some(TestPersistenceFailure::AfterSync));
                    assert_eq!(
                        journal.append(b"uncertain result"),
                        Err(PrivateJournalError::Uncertain)
                    );
                }
            }
            assert!(journal.contains_recovery_prefix(selected).is_err());
            assert_eq!(
                journal.contains_recovery_prefix(selected),
                Err(PrivateJournalError::Uncertain)
            );
        }
    }
}

#[cfg(all(test, unix))]
#[path = "private_journal_held_scan_tests.rs"]
mod held_scan_tests;

#[cfg(all(test, unix))]
#[path = "private_journal_single_record_tests.rs"]
mod single_record_tests;

#[cfg(test)]
mod portable_custody_tests {
    use super::*;
    const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
        filename: "portable.wal",
        magic: b"IKGTEST1",
        hash_domain: b"test-only:portable-journal\0",
        maximum_payload_bytes: 1024,
    };
    #[test]
    fn portable_journal_replays_same_durable_original_and_excludes_second_owner() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("original");
        let mut owner = PrivateJournal::create_new(&path, FORMAT).unwrap();
        owner.append(b"original initialization").unwrap();
        assert!(matches!(
            PrivateJournal::open_existing(&path, FORMAT),
            Err(PrivateJournalError::AlreadyOpen)
        ));
        let prefix = owner.recovery_prefix().unwrap();
        drop(owner);
        let mut recovered = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(
            recovered.replay_next().unwrap(),
            Some((0, b"original initialization".to_vec()))
        );
        assert_eq!(recovered.replay_next().unwrap(), None);
        assert_eq!(recovered.recovery_prefix().unwrap(), prefix);
        recovered.append(b"same original successor").unwrap();
        assert_eq!(recovered.recovery_prefix().unwrap().sequence, 2);
    }
    #[test]
    fn portable_recovery_never_creates_missing_original_or_resets_existing_store() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("original");
        assert!(PrivateJournal::open_existing(&path, FORMAT).is_err());
        assert!(!path.exists());
        let mut owner = PrivateJournal::create_new(&path, FORMAT).unwrap();
        owner.append(b"unchanged original").unwrap();
        let prefix = owner.recovery_prefix().unwrap();
        assert!(PrivateJournal::create_new(&path, FORMAT).is_err());
        assert_eq!(owner.recovery_prefix().unwrap(), prefix);
    }
    #[test]
    fn portable_replay_reanchors_after_other_exact_offset_reads() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("original");
        let mut owner = PrivateJournal::create_new(&path, FORMAT).unwrap();
        owner.append(b"first").unwrap();
        owner.append(b"second").unwrap();
        drop(owner);
        let mut recovered = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(recovered.replay_next().unwrap().unwrap().1, b"first");
        let logical = recovered.read_bytes;
        let mut header = [0; FRAME_HEADER_BYTES];
        iroha_fs::read_exact_at(&recovered.journal, &mut header, 0).unwrap();
        assert_eq!(recovered.read_bytes, logical);
        assert_eq!(recovered.replay_next().unwrap().unwrap().1, b"second");
        assert_eq!(recovered.replay_next().unwrap(), None);
    }

    #[test]
    fn portable_positioned_scan_does_not_overwrite_original_tail() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().canonicalize().unwrap().join("original");
        let mut owner = PrivateJournal::create_new(&path, FORMAT).unwrap();
        owner.append(b"first").unwrap();
        let mut cursor = owner.replay_cursor().unwrap();
        assert_eq!(
            owner
                .read_cursor_next(&mut cursor)
                .unwrap()
                .unwrap()
                .1
                .as_slice(),
            b"first"
        );
        owner.append(b"second").unwrap();
        drop(owner);
        let mut recovered = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(recovered.replay_next().unwrap().unwrap().1, b"first");
        assert_eq!(recovered.replay_next().unwrap().unwrap().1, b"second");
        assert_eq!(recovered.replay_next().unwrap(), None);
    }
}
