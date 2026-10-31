//! Exact journal-file custody across one locked geometry transition.
//!
//! The raw geometry caller still owns all publication locks. Each new phase
//! acquires its temporary before writing; this is not a pre-vote reservation or
//! permission to publish State. Retries retain that temporary, and a completed
//! rename is never retried as a new publication.

use std::io::{Seek, SeekFrom};

use super::*;
use crate::kura::{KuraInstanceIdentity, SecureMetadata, secure_file_metadata};

struct JournalFile {
    file: File,
    metadata: SecureMetadata,
}

struct PendingWrite {
    #[cfg(not(windows))]
    file: File,
    #[cfg(windows)]
    file: Option<File>,
    phase: LaneGeometryPhase,
    // Only a temporary created by this owner may be overwritten after a short
    // write. A preexisting recovery temporary must already have exact bytes.
    writable: bool,
}

/// Original parent/predecessor and the active phase's one physical write owner.
pub(super) struct RetainedGeometryJournal {
    kura: KuraInstanceIdentity,
    namespace: BoundProgressNamespace,
    current: Option<JournalFile>,
    current_bytes: Box<[u8]>,
    current_len: usize,
    pending: Option<PendingWrite>,
    // Set immediately on rename, before any fallible verification or sync.
    renamed: bool,
    completed: Option<LaneGeometryPhase>,
}

impl RetainedGeometryJournal {
    /// Start the next exact write under the same operation's retained descriptors.
    /// Raw operation preparation may grow this buffer; this is not pre-vote admission.
    pub(super) fn prepare_next_write(&mut self, next_len: usize) -> Result<()> {
        if self.pending.is_some() || self.renamed {
            return Err(self.error("unfinished geometry journal write cannot be replaced"));
        }
        self.verify_current()?;
        if next_len > self.current_bytes.len() {
            if u64::try_from(next_len).unwrap_or(u64::MAX) > MAX_GEOMETRY_JOURNAL_BYTES {
                return Err(self.error("retained geometry write exceeds the journal limit"));
            }
            let mut replacement = Vec::new();
            replacement.try_reserve_exact(next_len)?;
            replacement.resize(next_len, 0);
            replacement[..self.current_len]
                .copy_from_slice(&self.current_bytes[..self.current_len]);
            self.current_bytes = replacement.into_boxed_slice();
        }
        self.completed = None;
        Ok(())
    }

    pub(super) fn capture(kura: &Kura, next_len: usize) -> Result<Self> {
        let path = kura.lane_geometry_journal_path();
        let temporary = kura.store_root.join(JOURNAL_TEMP_FILE_NAME);
        let parent = Kura::open_bound_progress_directory(&kura.store_root, &kura.store_root)?;
        let namespace = BoundProgressNamespace {
            data_path: path,
            index_path: temporary,
            directories: vec![parent],
        };
        let current = Self::open_journal_file(kura, &namespace, &namespace.data_path)?
            .map(|file| -> Result<JournalFile> {
                let metadata = secure_file_metadata::from_file(&file)
                    .map_err(|error| Error::IO(error, namespace.data_path.clone()))?;
                Ok(JournalFile { file, metadata })
            })
            .transpose()?;
        let current_len = current.as_ref().map_or(0, |current| current.metadata.len());
        if current_len > MAX_GEOMETRY_JOURNAL_BYTES {
            return Err(kura.geometry_error(
                ErrorKind::InvalidData,
                "retained geometry predecessor exceeds the journal limit",
            ));
        }
        let current_len = usize::try_from(current_len)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(current_len.max(next_len))?;
        bytes.resize(current_len.max(next_len), 0);
        let mut owner = Self {
            kura: kura.instance_identity(),
            namespace,
            current,
            current_bytes: bytes.into_boxed_slice(),
            current_len,
            pending: None,
            renamed: false,
            completed: None,
        };
        if let Some(current) = &mut owner.current {
            current
                .file
                .read_exact(&mut owner.current_bytes[..current_len])
                .map_err(|error| Error::IO(error, owner.namespace.data_path.clone()))?;
        }
        owner.verify_current()?;
        Ok(owner)
    }

    /// Exact predecessor value captured from the retained descriptor.
    pub(super) fn predecessor(&self) -> Option<&[u8]> {
        self.current
            .as_ref()
            .map(|_| &self.current_bytes[..self.current_len])
    }

    pub(super) fn retained_allocation_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>().checked_add(self.current_bytes.len())?;
        bytes = bytes.checked_add(self.namespace.data_path.capacity())?;
        bytes = bytes.checked_add(self.namespace.index_path.capacity())?;
        bytes = bytes.checked_add(
            self.namespace
                .directories
                .capacity()
                .checked_mul(std::mem::size_of::<BoundProgressDirectory>())?,
        )?;
        for directory in &self.namespace.directories {
            bytes = bytes.checked_add(directory.expected_path.capacity())?;
            bytes = bytes.checked_add(directory.canonical_path.capacity())?;
            if let Some(name) = &directory.entry_name {
                bytes = bytes.checked_add(name.capacity())?;
            }
        }
        Some(bytes)
    }

    fn error(&self, message: &'static str) -> Error {
        Error::IO(
            std::io::Error::new(ErrorKind::InvalidData, message),
            self.namespace.data_path.clone(),
        )
    }

    fn verify_namespace(&self) -> Result<()> {
        if !Kura::progress_mutation_namespace_unchanged(&self.namespace) {
            return Err(self.error("retained geometry journal parent changed"));
        }
        Ok(())
    }

    fn open_journal_file(
        kura: &Kura,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<File>> {
        #[cfg(not(windows))]
        {
            kura.open_optional_bound_progress_file(namespace, path)
        }
        #[cfg(windows)]
        {
            let _ = kura;
            let owner = Kura::native_evidence_parent(namespace, path)?;
            let name = path.file_name().ok_or_else(|| {
                Error::IO(
                    std::io::Error::new(ErrorKind::InvalidInput, "journal name missing"),
                    path.to_path_buf(),
                )
            })?;
            owner
                .open_private_journal_optional(name)
                .map_err(|error| Error::IO(error, path.to_path_buf()))
        }
    }

    // Validate through the original parent descriptor. No reopening through a
    // substituted path, and no following symlinks, FIFOs or extra hard links.
    fn verify_name(&self, path: &Path, file: Option<&File>) -> Result<()> {
        self.verify_namespace()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let named = rustix::fs::statat(
                &self.namespace.directories[0].file,
                path.file_name()
                    .ok_or_else(|| self.error("journal name missing"))?,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            );
            match (file, named) {
                (None, Err(rustix::io::Errno::NOENT)) => return Ok(()),
                (Some(file), Ok(named)) => {
                    let opened = file
                        .metadata()
                        .map_err(|error| Error::IO(error, path.to_path_buf()))?;
                    if opened.is_file()
                        && opened.nlink() == 1
                        && rustix::fs::FileType::from_raw_mode(named.st_mode)
                            == rustix::fs::FileType::RegularFile
                        && crate::kura::unix_device_identity(named.st_dev) == opened.dev()
                        && named.st_ino == opened.ino()
                        && named.st_nlink == 1
                    {
                        return Ok(());
                    }
                }
                (_, Err(error)) if error != rustix::io::Errno::NOENT => {
                    return Err(Error::IO(error.into(), path.to_path_buf()));
                }
                _ => {}
            }
            Err(self.error("retained geometry journal file or absence changed"))
        }
        #[cfg(windows)]
        {
            let owner = Kura::native_evidence_parent(&self.namespace, path)?;
            let name = path
                .file_name()
                .ok_or_else(|| self.error("journal name missing"))?;
            match file {
                Some(file) => {
                    Kura::native_evidence_original(&self.namespace, path, file)?;
                }
                None => {
                    if owner
                        .open_regular_optional(name)
                        .map_err(|error| Error::IO(error, path.to_path_buf()))?
                        .is_some()
                    {
                        return Err(self.error("retained geometry journal absence changed"));
                    }
                }
            }
            self.verify_namespace()
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (path, file);
            Err(self
                .error("retained journal publication requires descriptor-relative file identity"))
        }
    }

    fn verify_bytes(file: &File, expected: &[u8], path: &Path) -> Result<()> {
        let mut reader = file;
        reader
            .seek(SeekFrom::Start(0))
            .map_err(|error| Error::IO(error, path.to_path_buf()))?;
        let mut scratch = [0_u8; 8192];
        for chunk in expected.chunks(scratch.len()) {
            reader
                .read_exact(&mut scratch[..chunk.len()])
                .map_err(|error| Error::IO(error, path.to_path_buf()))?;
            if &scratch[..chunk.len()] != chunk {
                return Err(Error::IO(
                    std::io::Error::new(ErrorKind::InvalidData, "retained journal bytes changed"),
                    path.to_path_buf(),
                ));
            }
        }
        if reader
            .read(&mut scratch[..1])
            .map_err(|error| Error::IO(error, path.to_path_buf()))?
            != 0
        {
            return Err(Error::IO(
                std::io::Error::new(ErrorKind::InvalidData, "retained journal grew"),
                path.to_path_buf(),
            ));
        }
        Ok(())
    }

    fn verify_current(&self) -> Result<()> {
        self.verify_name(
            &self.namespace.data_path,
            self.current.as_ref().map(|current| &current.file),
        )?;
        if let Some(current) = &self.current {
            let opened = secure_file_metadata::from_file(&current.file)
                .map_err(|error| Error::IO(error, self.namespace.data_path.clone()))?;
            if !Kura::sidecar_file_metadata_unchanged(&current.metadata, &opened) {
                return Err(self.error("retained geometry journal predecessor changed"));
            }
            Self::verify_bytes(
                &current.file,
                &self.current_bytes[..self.current_len],
                &self.namespace.data_path,
            )?;
            self.verify_name(&self.namespace.data_path, Some(&current.file))?;
        }
        Ok(())
    }

    /// Recheck this original completed file/absence without another write or reopen.
    pub(super) fn reauthenticate_current(&self, kura: &Kura) -> Result<()> {
        if !self.kura.matches(kura) || self.pending.is_some() || self.renamed {
            return Err(self.error("retained geometry journal is foreign or still pending"));
        }
        self.verify_current()
    }

    /// Authenticate the exact bytes last made durable by this retained phase owner.
    pub(super) fn reauthenticate_completed(
        &self,
        kura: &Kura,
        phase: LaneGeometryPhase,
        bytes: &[u8],
    ) -> Result<()> {
        if self.completed != Some(phase) || &self.current_bytes[..self.current_len] != bytes {
            return Err(self.error("retained geometry completion differs from its original phase"));
        }
        self.reauthenticate_current(kura)
    }

    fn sync_file(file: &File, path: &Path) -> Result<()> {
        #[cfg(test)]
        if crate::kura::FAIL_NEXT_BOUND_PROGRESS_INTENT_FILE_SYNC.with(|fault| fault.replace(false))
        {
            return Err(Error::IO(
                std::io::Error::other("injected retained geometry journal file sync failure"),
                path.to_path_buf(),
            ));
        }
        file.sync_all()
            .map_err(|error| Error::IO(error, path.to_path_buf()))
    }

    pub(super) fn persist(
        &mut self,
        kura: &Kura,
        phase: LaneGeometryPhase,
        bytes: &[u8],
    ) -> Result<()> {
        if !self.kura.matches(kura) {
            return Err(self.error("retained geometry journal belongs to another Kura"));
        }
        if bytes.len() > self.current_bytes.len() {
            return Err(self.error("geometry phase exceeds its retained readback buffer"));
        }
        if let Some(pending) = &self.pending {
            if pending.phase != phase {
                return Err(self.error(
                    "unfinished geometry journal phase must complete before another phase",
                ));
            }
        } else if self.completed == Some(phase) {
            self.verify_current()?;
            if &self.current_bytes[..self.current_len] != bytes {
                return Err(self.error("completed geometry journal phase changed its bytes"));
            }
            return Ok(());
        }
        if !self.renamed {
            self.verify_current()?;
        } else {
            self.verify_namespace()?;
        }
        if self.pending.is_none()
            && self.current.is_some()
            && &self.current_bytes[..self.current_len] == bytes
        {
            // A newly reconstructed owner may already have this phase. Prove
            // durability on that original object; a second same-bytes temp
            // would be an ambiguous competing inode during startup recovery.
            let current = self
                .current
                .as_ref()
                .ok_or_else(|| self.error("geometry journal lost its current phase"))?;
            Self::sync_file(&current.file, &self.namespace.data_path)?;
            Kura::sync_bound_progress_intent_directories(&self.namespace)
                .map_err(|error| Error::IO(error, self.namespace.data_path.clone()))?;
            self.verify_current()?;
            self.completed = Some(phase);
            return Ok(());
        }
        let before = kura
            .accounted_atomic_geometry_file_len(&self.namespace.data_path)?
            .saturating_add(kura.accounted_atomic_geometry_file_len(&self.namespace.index_path)?);
        let accounting = kura
            .begin_total_disk_usage_mutation()
            .with_resource_paths(vec![
                self.namespace.data_path.clone(),
                self.namespace.index_path.clone(),
            ]);
        if self.pending.is_none() {
            let existing =
                Self::open_journal_file(kura, &self.namespace, &self.namespace.index_path)?;
            #[cfg(windows)]
            {
                let writable = existing.is_none();
                // The phase and empty original slot exist before native CREATE_NEW. An
                // error after creation retains that exact handle for this same phase.
                self.pending = Some(PendingWrite {
                    file: existing,
                    phase,
                    writable,
                });
                if let Some(file) = self
                    .pending
                    .as_ref()
                    .and_then(|pending| pending.file.as_ref())
                {
                    Self::verify_bytes(file, bytes, &self.namespace.index_path)?;
                }
            }
            #[cfg(not(windows))]
            {
                let (file, writable) = match existing {
                    Some(file) => {
                        Self::verify_bytes(&file, bytes, &self.namespace.index_path)?;
                        (file, false)
                    }
                    None => {
                        #[cfg(unix)]
                        let file = File::from(
                            rustix::fs::openat(
                                &self.namespace.directories[0].file,
                                JOURNAL_TEMP_FILE_NAME,
                                rustix::fs::OFlags::RDWR
                                    | rustix::fs::OFlags::CREATE
                                    | rustix::fs::OFlags::EXCL
                                    | rustix::fs::OFlags::NOFOLLOW
                                    | rustix::fs::OFlags::CLOEXEC,
                                rustix::fs::Mode::from_raw_mode(0o600),
                            )
                            .map_err(|error| {
                                Error::IO(error.into(), self.namespace.index_path.clone())
                            })?,
                        );
                        #[cfg(not(unix))]
                        return Err(self.error(
                            "retained journal publication requires descriptor-relative creation",
                        ));
                        #[cfg(unix)]
                        (file, true)
                    }
                };
                self.pending = Some(PendingWrite {
                    file,
                    phase,
                    writable,
                });
            }
        }
        #[cfg(windows)]
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.file.is_none())
        {
            self.verify_name(&self.namespace.index_path, None)?;
            let owner = Kura::native_evidence_parent(&self.namespace, &self.namespace.index_path)?;
            let pending = self.pending.as_mut().ok_or_else(|| {
                Error::IO(
                    std::io::Error::other("geometry phase lost its pending creator"),
                    self.namespace.index_path.clone(),
                )
            })?;
            owner
                .create_private_journal_into(JOURNAL_TEMP_FILE_NAME, &mut pending.file)
                .map_err(|error| Error::IO(error, self.namespace.index_path.clone()))?;
        }
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| self.error("geometry phase lost its write owner"))?;
        #[cfg(not(windows))]
        let original = &pending.file;
        #[cfg(windows)]
        let original = pending
            .file
            .as_ref()
            .ok_or_else(|| self.error("geometry journal creation has no original yet"))?;
        if !self.renamed {
            self.verify_name(&self.namespace.index_path, Some(original))?;
            if pending.writable {
                let mut writer = original;
                writer
                    .seek(SeekFrom::Start(0))
                    .and_then(|_| writer.write_all(bytes))
                    .and_then(|_| writer.set_len(bytes.len() as u64))
                    .map_err(|error| Error::IO(error, self.namespace.index_path.clone()))?;
            }
            Self::verify_bytes(original, bytes, &self.namespace.index_path)?;
            Self::sync_file(original, &self.namespace.index_path)?;
            self.verify_current()?;
            #[cfg(windows)]
            {
                let owner =
                    Kura::native_evidence_parent(&self.namespace, &self.namespace.data_path)?;
                owner
                    .publish_private_journal_observed(
                        JOURNAL_TEMP_FILE_NAME,
                        JOURNAL_FILE_NAME,
                        original,
                        self.current.as_ref().map(|current| &current.file),
                        &mut self.renamed,
                        |_| Ok(()),
                    )
                    .map_err(|error| Error::IO(error, self.namespace.data_path.clone()))?;
            }
            #[cfg(not(windows))]
            {
                let promotion = if self.current.is_some() {
                    Kura::promote_bound_progress_temp(
                        &self.namespace,
                        &self.namespace.index_path,
                        &self.namespace.data_path,
                        original,
                    )
                } else {
                    Kura::promote_bound_progress_temp_noreplace(
                        &self.namespace,
                        &self.namespace.index_path,
                        &self.namespace.data_path,
                        original,
                    )
                };
                match promotion {
                    Ok(()) => self.renamed = true,
                    Err(error) => {
                        self.renamed = error.published;
                        return Err(Error::IO(error.source, self.namespace.data_path.clone()));
                    }
                }
            }
        }
        self.verify_name(&self.namespace.data_path, Some(original))?;
        Self::verify_bytes(original, bytes, &self.namespace.data_path)?;
        Self::sync_file(original, &self.namespace.data_path)?;
        Kura::sync_bound_progress_intent_directories(&self.namespace)
            .map_err(|error| Error::IO(error, self.namespace.data_path.clone()))?;
        self.verify_name(&self.namespace.data_path, Some(original))?;
        let metadata = secure_file_metadata::from_file(original)
            .map_err(|error| Error::IO(error, self.namespace.data_path.clone()))?;
        let after = kura
            .accounted_atomic_geometry_file_len(&self.namespace.data_path)?
            .saturating_add(kura.accounted_atomic_geometry_file_len(&self.namespace.index_path)?);
        let pending = self
            .pending
            .take()
            .ok_or_else(|| self.error("geometry phase lost its completed write owner"))?;
        #[cfg(not(windows))]
        let file = pending.file;
        #[cfg(windows)]
        let file = pending
            .file
            .ok_or_else(|| self.error("completed geometry journal lost its original"))?;
        self.current = Some(JournalFile { file, metadata });
        self.current_len = bytes.len();
        self.current_bytes[..bytes.len()].copy_from_slice(bytes);
        self.renamed = false;
        self.completed = Some(phase);
        kura.update_disk_usage_delta(before, after);
        accounting.finish();
        Ok(())
    }
}
