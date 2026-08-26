//! Crash-safe write-ahead log for Sumeragi v2 safety decisions.
//!
//! The log stores opaque, Norito-encoded records behind a small framed envelope.  A frame is
//! acknowledged only after the file has been flushed and synchronised, which lets the consensus
//! reducer order signing and view-change effects after durable state transitions.  The envelope is
//! hash chained so corruption before the final, incomplete crash tail fails closed. Recovery
//! verifies frames incrementally and fixed height-local record/payload ceilings are enforced both
//! while opening and before append I/O, keeping valid replay memory bounded.
//! Platforms without descriptor-relative, no-follow directory and file operations reject safety
//! WAL storage as unsupported; lexical path checks are not an authenticated storage substitute.
use super::v2_core::{
    SAFETY_WAL_FILE_HEADER_LEN as FILE_HEADER_LEN, SAFETY_WAL_FRAME_HEADER_LEN as FRAME_HEADER_LEN,
    SAFETY_WAL_FRAME_MAGIC as FRAME_MAGIC, SAFETY_WAL_HASH_LEN as HASH_LEN,
    SAFETY_WAL_MAX_RECORD_BYTES as MAX_RECORD_BYTES, WalAppendError, WalAppendIo, WalAppendState,
    WalCodecError, WalFileIdentity, WalFrameCorruption, WalHeaderCorruption, WalIdentityField,
    WalIoStage, WalRetirementAuthorization, encode_wal_file_header, recover_wal_file,
};
#[cfg(all(test, unix, not(target_os = "espidf")))]
use super::v2_core::{
    SAFETY_WAL_FILE_MAGIC as FILE_MAGIC, SAFETY_WAL_FORMAT_VERSION as FORMAT_VERSION,
};
#[cfg(all(test, unix, not(target_os = "espidf")))]
use std::fs::OpenOptions;
#[cfg(all(unix, not(target_os = "espidf")))]
use std::path::Component;
#[cfg(all(unix, not(target_os = "espidf")))]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
#[cfg(all(test, unix, not(target_os = "espidf")))]
const FILE_HEADER_PREFIX_LEN: usize = FILE_MAGIC.len() + 2 + 2 + HASH_LEN + HASH_LEN;
/// Maximum complete frames retained by one height-local safety WAL.
pub(crate) const SAFETY_WAL_MAX_RECORDS: usize = 8 * 1024;
/// Maximum combined payload bytes retained by one height-local safety WAL.
pub(crate) const SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
const SAFETY_WAL_RECOVERY_SCRATCH_BYTES: usize = 64 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WalRetentionLimits {
    max_records: usize,
    max_payload_bytes: usize,
}
const WAL_RETENTION_LIMITS: WalRetentionLimits = WalRetentionLimits {
    max_records: SAFETY_WAL_MAX_RECORDS,
    max_payload_bytes: SAFETY_WAL_MAX_TOTAL_PAYLOAD_BYTES,
};
/// A record recovered from the safety WAL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredRecord {
    /// Monotonic record sequence starting at zero.
    sequence: u64,
    /// Hash of the exact complete frame accepted by WAL recovery.
    frame_hash: [u8; HASH_LEN],
    /// Opaque Norito payload bytes supplied by the consensus adapter.
    payload: Vec<u8>,
}
impl RecoveredRecord {
    /// Return the physical frame sequence starting at zero.
    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }
    /// Return the verified hash of this exact complete frame.
    pub(crate) const fn frame_hash(&self) -> [u8; HASH_LEN] {
        self.frame_hash
    }
    /// Borrow the opaque canonical payload carried by this frame.
    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }
    /// Match an append acknowledgement to this exact retained frame.
    pub(crate) fn exactly_matches_receipt(&self, receipt: SafetyWalAppendReceipt) -> bool {
        self.sequence == receipt.sequence && self.frame_hash == receipt.frame_hash
    }
}
/// Exact frame identity acknowledged only after the safety WAL is synchronized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a synchronized WAL append receipt must be checked against its reducer intent"]
pub(crate) struct SafetyWalAppendReceipt {
    sequence: u64,
    frame_hash: [u8; HASH_LEN],
}
impl SafetyWalAppendReceipt {
    /// Return the acknowledged physical frame sequence.
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }
    /// Return the acknowledged hash of the exact complete frame.
    pub(crate) const fn frame_hash(self) -> [u8; HASH_LEN] {
        self.frame_hash
    }
}
/// Errors raised while opening, replaying, or appending the safety WAL.
#[derive(Debug, Error)]
pub(crate) enum SafetyWalError {
    /// A filesystem operation failed.
    #[error("sumeragi safety WAL I/O failed at {path}: {source}")]
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The file header is missing or malformed.
    #[error("invalid sumeragi safety WAL header at {path}: {reason}")]
    InvalidHeader {
        /// WAL path.
        path: PathBuf,
        /// Validation failure.
        reason: &'static str,
    },
    /// The WAL belongs to another network, protocol, or consensus key.
    #[error("sumeragi safety WAL identity mismatch at {path}: {field}")]
    IdentityMismatch {
        /// WAL path.
        path: PathBuf,
        /// Mismatching header field.
        field: &'static str,
    },
    /// A complete frame is corrupt. Only an incomplete final frame may be discarded.
    #[error("corrupt sumeragi safety WAL frame {sequence} at {path}: {reason}")]
    CorruptFrame {
        /// WAL path.
        path: PathBuf,
        /// Expected frame sequence.
        sequence: u64,
        /// Validation failure.
        reason: &'static str,
    },
    /// A record exceeds the bounded safety-record size.
    #[error("sumeragi safety WAL record is too large: {actual} bytes (maximum {maximum})")]
    RecordTooLarge {
        /// Supplied record size.
        actual: usize,
        /// Maximum accepted record size.
        maximum: usize,
    },
    /// The height-local WAL exceeds its fixed record or aggregate-payload retention bound.
    #[error(
        "sumeragi safety WAL retention bound exceeded at {path}: {records} records/{payload_bytes} payload bytes (maximum {maximum_records} records/{maximum_payload_bytes} bytes)"
    )]
    RetentionLimitExceeded {
        /// WAL path.
        path: PathBuf,
        /// Record count that would cross the bound.
        records: usize,
        /// Aggregate payload bytes that would cross the bound.
        payload_bytes: usize,
        /// Maximum retained record count.
        maximum_records: usize,
        /// Maximum retained aggregate payload bytes.
        maximum_payload_bytes: usize,
    },
    /// The monotonic frame sequence is exhausted.
    #[error("sumeragi safety WAL frame sequence overflow at {path}")]
    SequenceOverflow {
        /// WAL path.
        path: PathBuf,
    },
    /// An ordered append I/O stage failed and poisoned this WAL instance.
    #[error("sumeragi safety WAL append {stage:?} failed at {path}: {source}")]
    AppendIo {
        /// WAL path.
        path: PathBuf,
        /// Exact failed stage.
        stage: WalIoStage,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// A failed append was retried without verified reopen and recovery.
    #[error("sumeragi safety WAL at {path} is failed closed and must be reopened")]
    FailedClosed {
        /// WAL path.
        path: PathBuf,
    },
    /// This platform cannot provide descriptor-relative adjacent-store ownership.
    #[cfg_attr(
        all(unix, not(target_os = "espidf")),
        allow(
            dead_code,
            reason = "unsupported-platform branch is compiled only for parity"
        )
    )]
    #[error("sumeragi safety WAL storage binding is unsupported at {path}: {reason}")]
    UnsupportedStorageBinding {
        /// WAL path.
        path: PathBuf,
        /// Fixed unsupported-platform diagnostic.
        reason: &'static str,
    },
}
/// Opened, no-follow owner of the post-open directory containing one safety WAL.
///
/// The raw directory handle and canonical path never cross this module. Fixed
/// sibling capabilities below expose only bounded read, publication, and
/// retirement operations for their statically derived entry names.
#[derive(Debug)]
struct BoundSafetyWalDirectory {
    expected_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    canonical_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    directory: File,
    #[cfg(all(unix, not(target_os = "espidf")))]
    identity: (u64, u64),
}
/// Private descriptor-relative owner of one fixed safety-WAL-adjacent entry.
#[derive(Debug)]
struct BoundSafetyWalAdjacentEntry {
    #[cfg(all(unix, not(target_os = "espidf")))]
    directory: Arc<BoundSafetyWalDirectory>,
    #[cfg(all(unix, not(target_os = "espidf")))]
    entry_name: OsString,
    display_path: PathBuf,
}
/// Move-only authority for the fixed serviced-candidate sibling snapshot.
#[derive(Debug)]
#[must_use = "serviced-candidate storage authority must open its fixed adjacent store"]
pub(crate) struct SafetyWalServicedCandidateStoreAuthority {
    entry: BoundSafetyWalAdjacentEntry,
}
/// Move-only authority for the fixed leader-wire lifecycle sibling snapshot.
#[derive(Debug)]
#[must_use = "leader-wire storage authority must open its fixed adjacent store"]
pub(crate) struct SafetyWalLeaderWireStoreAuthority {
    entry: BoundSafetyWalAdjacentEntry,
}
impl SafetyWalServicedCandidateStoreAuthority {
    /// Read the complete fixed snapshot through its retained directory owner.
    pub(crate) fn read_bounded(&self, maximum: u64) -> Result<Option<Vec<u8>>, String> {
        self.entry.read_bounded(maximum, "serviced-candidate")
    }
    /// Atomically replace and directory-sync the fixed snapshot.
    pub(crate) fn publish_atomic(&self, frame: &[u8], maximum: u64) -> Result<(), String> {
        self.entry
            .publish_atomic(frame, maximum, "serviced-candidate")
    }
    /// Remove and directory-sync the exact fixed snapshot, when present.
    pub(crate) fn retire(self, maximum: u64) -> Result<(), String> {
        self.entry.retire(maximum, "serviced-candidate")
    }
    /// Return the diagnostic path only to in-module and extracted test fixtures.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &Path {
        &self.entry.display_path
    }
    #[cfg(test)]
    pub(crate) fn for_test_path(safety_wal_path: &Path) -> Result<Self, String> {
        BoundSafetyWalAdjacentEntry::for_test_path(safety_wal_path, ".serviced-candidates")
            .map(|entry| Self { entry })
    }
}
impl SafetyWalLeaderWireStoreAuthority {
    /// Read the complete fixed snapshot through its retained directory owner.
    pub(crate) fn read_bounded(&self, maximum: u64) -> Result<Option<Vec<u8>>, String> {
        self.entry.read_bounded(maximum, "leader-wire lifecycle")
    }
    /// Atomically replace and directory-sync the fixed snapshot.
    pub(crate) fn publish_atomic(&self, frame: &[u8], maximum: u64) -> Result<(), String> {
        self.entry
            .publish_atomic(frame, maximum, "leader-wire lifecycle")
    }
    /// Remove and directory-sync the exact fixed snapshot, when present.
    #[cfg(test)]
    pub(crate) fn retire(self, maximum: u64) -> Result<(), String> {
        self.entry.retire(maximum, "leader-wire lifecycle")
    }
    /// Return the diagnostic path only to in-module and extracted test fixtures.
    #[cfg(test)]
    pub(crate) fn path_for_test(&self) -> &Path {
        &self.entry.display_path
    }
    #[cfg(test)]
    pub(crate) fn for_test_path(safety_wal_path: &Path) -> Result<Self, String> {
        BoundSafetyWalAdjacentEntry::for_test_path(safety_wal_path, ".leader-wire-lifecycles")
            .map(|entry| Self { entry })
    }
}
impl BoundSafetyWalDirectory {
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn from_kura_authority(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraSafetyWalDirectoryAuthority,
    ) -> io::Result<Self> {
        if !authority.matches_kura(kura) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "safety-WAL authority belongs to a different Kura instance",
            ));
        }
        let (expected_path, directory) =
            authority.into_opened_directory_for(kura).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "consumed safety-WAL authority changed its Kura identity",
                )
            })?;
        let canonical_path = fs::canonicalize(&expected_path)?;
        let lexical_metadata = direct_lexical_directory_metadata(&expected_path)?;
        let metadata = directory.metadata()?;
        let identity = unix_file_identity(&metadata);
        if !metadata.is_dir() || unix_file_identity(&lexical_metadata) != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Kura-bound safety-WAL directory changed before WAL open",
            ));
        }
        Ok(Self {
            expected_path,
            canonical_path,
            directory,
            identity,
        })
    }
    #[cfg(all(test, unix, not(target_os = "espidf")))]
    fn bind(expected_path: &Path) -> io::Result<Self> {
        let lexical_metadata = direct_lexical_directory_metadata(expected_path)?;
        let canonical_path = fs::canonicalize(expected_path)?;
        let directory = open_canonical_directory_nofollow(&canonical_path)?;
        let metadata = directory.metadata()?;
        let identity = unix_file_identity(&metadata);
        if !metadata.is_dir() || unix_file_identity(&lexical_metadata) != identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "safety WAL parent is not a directory",
            ));
        }
        Ok(Self {
            expected_path: expected_path.to_path_buf(),
            canonical_path,
            directory,
            identity,
        })
    }
    fn verify_linked(&self) -> io::Result<()> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let lexical_metadata = direct_lexical_directory_metadata(&self.expected_path)?;
            let canonical_path = fs::canonicalize(&self.expected_path)?;
            if canonical_path != self.canonical_path {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "safety WAL parent resolves to a different canonical directory",
                ));
            }
            let linked = open_canonical_directory_nofollow(&canonical_path)?;
            let retained_metadata = self.directory.metadata()?;
            let linked_metadata = linked.metadata()?;
            if !retained_metadata.is_dir()
                || !linked_metadata.is_dir()
                || unix_file_identity(&lexical_metadata) != self.identity
                || unix_file_identity(&retained_metadata) != self.identity
                || unix_file_identity(&linked_metadata) != self.identity
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "safety WAL parent changed its opened directory identity",
                ));
            }
            Ok(())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(unsupported_storage_binding_io())
        }
    }
    fn open_wal_leaf(&self, name: &OsStr) -> io::Result<(File, bool)> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_linked()?;
            let (created, flags, existing_identity) = match rustix::fs::statat(
                &self.directory,
                name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(stat) => {
                    ensure_unix_regular_single_link_stat(&stat)?;
                    (
                        false,
                        rustix::fs::OFlags::RDWR
                            | rustix::fs::OFlags::NOFOLLOW
                            | rustix::fs::OFlags::CLOEXEC,
                        Some((stat.st_dev as u64, stat.st_ino as u64)),
                    )
                }
                Err(rustix::io::Errno::NOENT) => (
                    true,
                    rustix::fs::OFlags::RDWR
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::EXCL
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    None,
                ),
                Err(error) => return Err(io::Error::from(error)),
            };
            let file = File::from(
                rustix::fs::openat(
                    &self.directory,
                    name,
                    flags,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .map_err(io::Error::from)?,
            );
            if let Some(expected_identity) = existing_identity {
                let opened = file.metadata()?;
                if unix_file_identity(&opened) != expected_identity {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "safety WAL leaf changed between inspection and open",
                    ));
                }
            }
            self.verify_leaf(&file, name)?;
            self.verify_linked()?;
            Ok((file, created))
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = name;
            Err(unsupported_storage_binding_io())
        }
    }
    fn verify_leaf(&self, file: &File, name: &OsStr) -> io::Result<()> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::MetadataExt as _;
            self.verify_linked()?;
            let opened = file.metadata()?;
            let linked =
                rustix::fs::statat(&self.directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                    .map_err(io::Error::from)?;
            ensure_unix_regular_single_link_stat(&linked)?;
            if !opened.is_file()
                || opened.nlink() != 1
                || opened.dev() != linked.st_dev as u64
                || opened.ino() != linked.st_ino as u64
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "safety WAL leaf changed its opened file identity",
                ));
            }
            Ok(())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (file, name);
            Err(unsupported_storage_binding_io())
        }
    }
    fn sync(&self) -> io::Result<()> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_linked()?;
            self.directory.sync_all()?;
            self.verify_linked()
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(unsupported_storage_binding_io())
        }
    }
    fn unlink_exact_leaf(&self, name: &OsStr, file: &File) -> io::Result<()> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_leaf(file, name)?;
            rustix::fs::unlinkat(&self.directory, name, rustix::fs::AtFlags::empty())
                .map_err(io::Error::from)?;
            self.sync()
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (name, file);
            Err(unsupported_storage_binding_io())
        }
    }
}
impl BoundSafetyWalAdjacentEntry {
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn from_wal(
        directory: Arc<BoundSafetyWalDirectory>,
        wal_path: &Path,
        suffix: &str,
    ) -> io::Result<Self> {
        let wal_name = wal_path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "safety WAL path has no file name",
            )
        })?;
        let mut entry_name = wal_name.to_os_string();
        entry_name.push(suffix);
        let display_path = wal_path.with_file_name(&entry_name);
        Ok(Self {
            directory,
            entry_name,
            display_path,
        })
    }
    #[cfg(test)]
    fn for_test_path(safety_wal_path: &Path, suffix: &str) -> Result<Self, String> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let parent = safety_wal_parent(safety_wal_path).map_err(|error| error.to_string())?;
            fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
            let directory = Arc::new(
                BoundSafetyWalDirectory::bind(&parent).map_err(|error| error.to_string())?,
            );
            Self::from_wal(directory, safety_wal_path, suffix).map_err(|error| error.to_string())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = suffix;
            Err(format!(
                "descriptor-relative no-follow safety-WAL storage is unsupported on this platform: {}",
                safety_wal_path.display()
            ))
        }
    }
    fn read_bounded(&self, maximum: u64, label: &str) -> Result<Option<Vec<u8>>, String> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::MetadataExt as _;
            self.directory.verify_linked().map_err(|error| {
                self.error(label, "verify adjacent directory before read", error)
            })?;
            let linked_before = match rustix::fs::statat(
                &self.directory.directory,
                &self.entry_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => {
                    self.directory.verify_linked().map_err(|error| {
                        self.error(label, "verify absent adjacent snapshot", error)
                    })?;
                    return Ok(None);
                }
                Err(error) => {
                    return Err(self.error(
                        label,
                        "inspect adjacent snapshot",
                        io::Error::from(error),
                    ));
                }
            };
            ensure_unix_regular_single_link_stat(&linked_before)
                .map_err(|error| self.error(label, "validate adjacent snapshot", error))?;
            if linked_before.st_size < 0
                || u64::try_from(linked_before.st_size).unwrap_or(u64::MAX) > maximum
            {
                return Err(format!(
                    "{label} snapshot {} exceeds its bounded frame size",
                    self.display_path.display()
                ));
            }
            let mut file = File::from(
                rustix::fs::openat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(io::Error::from)
                .map_err(|error| self.error(label, "open adjacent snapshot", error))?,
            );
            let opened_before = file
                .metadata()
                .map_err(|error| self.error(label, "inspect opened adjacent snapshot", error))?;
            if !opened_before.is_file()
                || opened_before.nlink() != 1
                || opened_before.dev() != linked_before.st_dev as u64
                || opened_before.ino() != linked_before.st_ino as u64
                || opened_before.len() > maximum
            {
                return Err(format!(
                    "{label} snapshot {} changed identity while opening",
                    self.display_path.display()
                ));
            }
            let read_limit = maximum
                .checked_add(1)
                .ok_or_else(|| format!("{label} snapshot read bound overflowed"))?;
            let mut bytes =
                Vec::with_capacity(usize::try_from(opened_before.len()).unwrap_or_default());
            Read::by_ref(&mut file)
                .take(read_limit)
                .read_to_end(&mut bytes)
                .map_err(|error| self.error(label, "read adjacent snapshot", error))?;
            let opened_after = file
                .metadata()
                .map_err(|error| self.error(label, "reinspect opened adjacent snapshot", error))?;
            let linked_after = rustix::fs::statat(
                &self.directory.directory,
                &self.entry_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(io::Error::from)
            .map_err(|error| self.error(label, "reinspect adjacent snapshot", error))?;
            self.directory.verify_linked().map_err(|error| {
                self.error(label, "verify adjacent directory after read", error)
            })?;
            let bytes_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if bytes_len > maximum
                || !unix_metadata_revision_unchanged(&opened_before, &opened_after)
                || opened_after.dev() != linked_after.st_dev as u64
                || opened_after.ino() != linked_after.st_ino as u64
                || linked_after.st_nlink as u64 != 1
                || opened_after.len() != bytes_len
            {
                return Err(format!(
                    "{label} snapshot {} changed while reading",
                    self.display_path.display()
                ));
            }
            Ok(Some(bytes))
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = maximum;
            Err(format!(
                "{label} snapshot storage is unsupported on this platform: {}",
                self.display_path.display()
            ))
        }
    }
    fn publish_atomic(&self, frame: &[u8], maximum: u64, label: &str) -> Result<(), String> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::MetadataExt as _;
            let frame_len = u64::try_from(frame.len())
                .map_err(|_| format!("{label} snapshot frame length is not representable"))?;
            if frame_len > maximum {
                return Err(format!(
                    "{label} snapshot frame exceeds its bounded publication size"
                ));
            }
            self.directory.verify_linked().map_err(|error| {
                self.error(label, "verify adjacent directory before publication", error)
            })?;
            let temporary = self.temporary_name();
            self.remove_stale_temporary(&temporary, label)?;
            self.ensure_replaceable_target(label)?;
            let mut file = File::from(
                rustix::fs::openat(
                    &self.directory.directory,
                    &temporary,
                    rustix::fs::OFlags::WRONLY
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::EXCL
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .map_err(io::Error::from)
                .map_err(|error| self.error(label, "create adjacent temporary", error))?,
            );
            let publication = (|| -> io::Result<()> {
                let created = file.metadata()?;
                let linked = rustix::fs::statat(
                    &self.directory.directory,
                    &temporary,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(io::Error::from)?;
                ensure_unix_regular_single_link_stat(&linked)?;
                if !created.is_file()
                    || created.nlink() != 1
                    || created.dev() != linked.st_dev as u64
                    || created.ino() != linked.st_ino as u64
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "adjacent temporary changed during exclusive creation",
                    ));
                }
                file.write_all(frame)?;
                file.flush()?;
                file.sync_all()?;
                self.directory.verify_linked()?;
                let synced = file.metadata()?;
                let before_promotion = rustix::fs::statat(
                    &self.directory.directory,
                    &temporary,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(io::Error::from)?;
                if synced.nlink() != 1
                    || synced.dev() != before_promotion.st_dev as u64
                    || synced.ino() != before_promotion.st_ino as u64
                    || synced.len() != frame_len
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "adjacent temporary changed before promotion",
                    ));
                }
                rustix::fs::renameat(
                    &self.directory.directory,
                    &temporary,
                    &self.directory.directory,
                    &self.entry_name,
                )
                .map_err(io::Error::from)?;
                let promoted = rustix::fs::statat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(io::Error::from)?;
                if promoted.st_dev as u64 != synced.dev()
                    || promoted.st_ino as u64 != synced.ino()
                    || promoted.st_nlink as u64 != 1
                    || promoted.st_size < 0
                    || u64::try_from(promoted.st_size).unwrap_or(u64::MAX) != frame_len
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "promoted adjacent snapshot has the wrong identity",
                    ));
                }
                self.directory.sync()?;
                let durable = rustix::fs::statat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(io::Error::from)?;
                if durable.st_dev as u64 != synced.dev()
                    || durable.st_ino as u64 != synced.ino()
                    || durable.st_nlink as u64 != 1
                    || durable.st_size < 0
                    || u64::try_from(durable.st_size).unwrap_or(u64::MAX) != frame_len
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "promoted adjacent snapshot changed across directory sync",
                    ));
                }
                self.directory.verify_linked()
            })();
            if let Err(error) = publication {
                drop(file);
                let _ = self.remove_stale_temporary(&temporary, label);
                return Err(self.error(label, "publish adjacent snapshot", error));
            }
            Ok(())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (frame, maximum);
            Err(format!(
                "{label} snapshot storage is unsupported on this platform: {}",
                self.display_path.display()
            ))
        }
    }
    fn retire(self, maximum: u64, label: &str) -> Result<(), String> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::MetadataExt as _;
            self.directory.verify_linked().map_err(|error| {
                self.error(label, "verify adjacent directory before retirement", error)
            })?;
            let linked = match rustix::fs::statat(
                &self.directory.directory,
                &self.entry_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            ) {
                Ok(stat) => stat,
                Err(rustix::io::Errno::NOENT) => {
                    self.directory.verify_linked().map_err(|error| {
                        self.error(label, "verify absent adjacent retirement", error)
                    })?;
                    return Ok(());
                }
                Err(error) => {
                    return Err(self.error(
                        label,
                        "inspect adjacent snapshot for retirement",
                        io::Error::from(error),
                    ));
                }
            };
            ensure_unix_regular_single_link_stat(&linked)
                .map_err(|error| self.error(label, "validate adjacent retirement", error))?;
            if linked.st_size < 0 || u64::try_from(linked.st_size).unwrap_or(u64::MAX) > maximum {
                return Err(format!(
                    "{label} snapshot {} exceeds its retirement bound",
                    self.display_path.display()
                ));
            }
            let file = File::from(
                rustix::fs::openat(
                    &self.directory.directory,
                    &self.entry_name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(io::Error::from)
                .map_err(|error| self.error(label, "open adjacent retirement", error))?,
            );
            let opened = file
                .metadata()
                .map_err(|error| self.error(label, "inspect adjacent retirement", error))?;
            if opened.nlink() != 1
                || opened.dev() != linked.st_dev as u64
                || opened.ino() != linked.st_ino as u64
            {
                return Err(format!(
                    "{label} snapshot {} changed before retirement",
                    self.display_path.display()
                ));
            }
            file.sync_all()
                .map_err(|error| self.error(label, "sync adjacent retirement", error))?;
            self.directory.verify_linked().map_err(|error| {
                self.error(label, "verify adjacent directory during retirement", error)
            })?;
            let linked_after = rustix::fs::statat(
                &self.directory.directory,
                &self.entry_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(io::Error::from)
            .map_err(|error| self.error(label, "reinspect adjacent retirement", error))?;
            if linked_after.st_dev as u64 != opened.dev()
                || linked_after.st_ino as u64 != opened.ino()
                || linked_after.st_nlink as u64 != 1
            {
                return Err(format!(
                    "{label} snapshot {} changed before unlink",
                    self.display_path.display()
                ));
            }
            drop(file);
            rustix::fs::unlinkat(
                &self.directory.directory,
                &self.entry_name,
                rustix::fs::AtFlags::empty(),
            )
            .map_err(io::Error::from)
            .map_err(|error| self.error(label, "unlink adjacent snapshot", error))?;
            self.directory
                .sync()
                .map_err(|error| self.error(label, "sync adjacent retirement", error))?;
            Ok(())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = maximum;
            Err(format!(
                "{label} snapshot storage is unsupported on this platform: {}",
                self.display_path.display()
            ))
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn temporary_name(&self) -> OsString {
        let mut name = self.entry_name.clone();
        name.push(".tmp");
        name
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn remove_stale_temporary(&self, name: &OsStr, label: &str) -> Result<(), String> {
        match rustix::fs::statat(
            &self.directory.directory,
            name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => {
                Err(self.error(label, "inspect adjacent temporary", io::Error::from(error)))
            }
            Ok(stat) => {
                ensure_unix_regular_single_link_stat(&stat)
                    .map_err(|error| self.error(label, "validate adjacent temporary", error))?;
                rustix::fs::unlinkat(
                    &self.directory.directory,
                    name,
                    rustix::fs::AtFlags::empty(),
                )
                .map_err(io::Error::from)
                .map_err(|error| self.error(label, "remove adjacent temporary", error))
            }
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn ensure_replaceable_target(&self, label: &str) -> Result<(), String> {
        match rustix::fs::statat(
            &self.directory.directory,
            &self.entry_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(self.error(
                label,
                "inspect adjacent publication target",
                io::Error::from(error),
            )),
            Ok(stat) => ensure_unix_regular_single_link_stat(&stat)
                .map_err(|error| self.error(label, "validate adjacent publication target", error)),
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn error(&self, label: &str, operation: &str, source: io::Error) -> String {
        format!(
            "failed to {operation} for {label} snapshot {}: {source}",
            self.display_path.display()
        )
    }
}
#[cfg(test)]
fn safety_wal_parent(path: &Path) -> io::Result<PathBuf> {
    if path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "safety WAL path has no file name",
        ));
    }
    Ok(path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf())
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn direct_lexical_directory_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "safety WAL immediate parent must be a direct directory",
        ));
    }
    Ok(metadata)
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn open_canonical_directory_nofollow(path: &Path) -> io::Result<File> {
    use std::os::unix::fs::MetadataExt as _;
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "canonical safety WAL directory path is not absolute",
        ));
    }
    let mut current = File::from(
        rustix::fs::open(
            "/",
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(io::Error::from)?,
    );
    for component in path.components() {
        let name = match component {
            Component::RootDir => continue,
            Component::Normal(name) => name,
            Component::CurDir => continue,
            Component::ParentDir | Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "canonical safety WAL directory has a non-normal component",
                ));
            }
        };
        let before = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(io::Error::from)?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "canonical safety WAL ancestry contains a non-directory component",
            ));
        }
        let child = File::from(
            rustix::fs::openat(
                &current,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        let opened = child.metadata()?;
        let after = rustix::fs::statat(&current, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(io::Error::from)?;
        if !opened.is_dir()
            || before.st_dev as u64 != opened.dev()
            || before.st_ino as u64 != opened.ino()
            || after.st_dev as u64 != opened.dev()
            || after.st_ino as u64 != opened.ino()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "canonical safety WAL ancestry changed during no-follow traversal",
            ));
        }
        current = child;
    }
    Ok(current)
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn unix_file_identity(metadata: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt as _;
    (metadata.dev(), metadata.ino())
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn unix_metadata_revision_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
        && left.nlink() == right.nlink()
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn wal_metadata_revision_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    unix_metadata_revision_unchanged(left, right)
}
#[cfg(not(all(unix, not(target_os = "espidf"))))]
fn wal_metadata_revision_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}
#[cfg(not(all(unix, not(target_os = "espidf"))))]
fn unsupported_storage_binding_io() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative no-follow safety-WAL storage is unavailable on this platform",
    )
}
#[cfg(all(unix, not(target_os = "espidf")))]
fn ensure_unix_regular_single_link_stat(stat: &rustix::fs::Stat) -> io::Result<()> {
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
        || stat.st_nlink as u64 != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bound safety-WAL-adjacent entry is not a direct single-link regular file",
        ));
    }
    Ok(())
}
/// Append-only, hash-chained Sumeragi safety WAL.
#[derive(Debug)]
pub(crate) struct SafetyWal {
    path: PathBuf,
    directory: Arc<BoundSafetyWalDirectory>,
    wal_name: OsString,
    file: File,
    records: Vec<RecoveredRecord>,
    payload_bytes: usize,
    append_state: WalAppendState,
    #[cfg(all(unix, not(target_os = "espidf")))]
    serviced_candidate_authority_minted: AtomicBool,
    #[cfg(all(unix, not(target_os = "espidf")))]
    leader_wire_authority_minted: AtomicBool,
}
impl SafetyWal {
    /// Open the production WAL through one descriptor-relative Kura authority.
    #[cfg(all(unix, not(target_os = "espidf")))]
    pub(crate) fn open_with_kura_authority(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraSafetyWalDirectoryAuthority,
        wal_name: impl Into<OsString>,
        protocol_version: u16,
        network_id: [u8; HASH_LEN],
        key_hash: [u8; HASH_LEN],
    ) -> Result<Self, SafetyWalError> {
        let wal_name = wal_name.into();
        let mut components = Path::new(&wal_name).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(SafetyWalError::Io {
                path: PathBuf::from(wal_name),
                source: io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Kura-bound safety WAL name must be one nonempty path component",
                ),
            });
        }
        let directory = Arc::new(
            BoundSafetyWalDirectory::from_kura_authority(kura, authority).map_err(|source| {
                SafetyWalError::Io {
                    path: kura.sumeragi_v2_storage_root().join("wal"),
                    source,
                }
            })?,
        );
        let path = directory.expected_path.join(&wal_name);
        Self::open_bound(
            path,
            directory,
            wal_name,
            protocol_version,
            network_id,
            key_hash,
        )
    }
    /// Reject production opening where descriptor-relative ancestry is unavailable.
    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    pub(crate) fn open_with_kura_authority(
        kura: &crate::kura::Kura,
        _authority: crate::kura::KuraSafetyWalDirectoryAuthority,
        wal_name: impl Into<OsString>,
        _protocol_version: u16,
        _network_id: [u8; HASH_LEN],
        _key_hash: [u8; HASH_LEN],
    ) -> Result<Self, SafetyWalError> {
        let path = kura
            .sumeragi_v2_storage_root()
            .join("wal")
            .join(wal_name.into());
        Err(SafetyWalError::UnsupportedStorageBinding {
            path,
            reason: "descriptor-relative Kura-root storage is unavailable",
        })
    }
    /// Open or create a WAL bound to the supplied network, protocol, and consensus-key hashes.
    ///
    /// An incomplete final frame is treated as an unacknowledged crash tail and truncated. Any
    /// earlier structural or hash-chain failure, or a complete prefix beyond the fixed retention
    /// bounds, is returned as an error.
    #[cfg(test)]
    pub(crate) fn open(
        path: impl Into<PathBuf>,
        protocol_version: u16,
        network_id: [u8; HASH_LEN],
        key_hash: [u8; HASH_LEN],
    ) -> Result<Self, SafetyWalError> {
        let path = path.into();
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let parent = safety_wal_parent(&path).map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
            fs::create_dir_all(&parent).map_err(|source| SafetyWalError::Io {
                path: parent.clone(),
                source,
            })?;
            let directory = Arc::new(BoundSafetyWalDirectory::bind(&parent).map_err(|source| {
                SafetyWalError::Io {
                    path: parent.clone(),
                    source,
                }
            })?);
            let wal_name = path
                .file_name()
                .expect("safety_wal_parent rejected a missing file name")
                .to_os_string();
            Self::open_bound(
                path,
                directory,
                wal_name,
                protocol_version,
                network_id,
                key_hash,
            )
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (protocol_version, network_id, key_hash);
            Err(SafetyWalError::UnsupportedStorageBinding {
                path,
                reason: "descriptor-relative storage is unavailable",
            })
        }
    }
    #[cfg_attr(
        not(all(unix, not(target_os = "espidf"))),
        allow(
            dead_code,
            reason = "unsupported platforms reject before constructing a bound WAL"
        )
    )]
    fn open_bound(
        path: PathBuf,
        directory: Arc<BoundSafetyWalDirectory>,
        wal_name: OsString,
        protocol_version: u16,
        network_id: [u8; HASH_LEN],
        key_hash: [u8; HASH_LEN],
    ) -> Result<Self, SafetyWalError> {
        let parent = directory.expected_path.clone();
        let identity = WalFileIdentity::new(protocol_version, network_id, key_hash);
        let (mut file, created) =
            directory
                .open_wal_leaf(&wal_name)
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
        if created
            || file
                .metadata()
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?
                .len()
                == 0
        {
            let header = encode_wal_file_header(identity, &frame_hash);
            file.write_all(&header)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_data())
                .and_then(|()| directory.verify_leaf(&file, &wal_name))
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
            directory.sync().map_err(|source| SafetyWalError::Io {
                path: parent.clone(),
                source,
            })?;
        }
        directory
            .verify_leaf(&file, &wal_name)
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
        let read_metadata_before = file.metadata().map_err(|source| SafetyWalError::Io {
            path: path.clone(),
            source,
        })?;
        let recovery = recover_wal_stream(&mut file, &path, identity, WAL_RETENTION_LIMITS)?;
        let read_metadata_after = file.metadata().map_err(|source| SafetyWalError::Io {
            path: path.clone(),
            source,
        })?;
        directory
            .verify_leaf(&file, &wal_name)
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
        if !wal_metadata_revision_unchanged(&read_metadata_before, &read_metadata_after) {
            return Err(SafetyWalError::Io {
                path: path.clone(),
                source: io::Error::new(
                    io::ErrorKind::InvalidData,
                    "safety WAL changed while recovering its durable bytes",
                ),
            });
        }
        if recovery.incomplete_tail {
            let valid_prefix_len = recovery.valid_prefix_len;
            file.set_len(valid_prefix_len)
                .and_then(|()| file.sync_data())
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
            let truncated_before = file.metadata().map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
            directory
                .verify_leaf(&file, &wal_name)
                .map_err(|source| SafetyWalError::Io {
                    path: path.clone(),
                    source,
                })?;
            let truncated_after = file.metadata().map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
            if truncated_after.len() != valid_prefix_len
                || !wal_metadata_revision_unchanged(&truncated_before, &truncated_after)
            {
                return Err(SafetyWalError::Io {
                    path: path.clone(),
                    source: io::Error::new(
                        io::ErrorKind::InvalidData,
                        "safety WAL changed after crash-tail truncation",
                    ),
                });
            }
        }
        file.seek(SeekFrom::End(0))
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
        directory
            .verify_leaf(&file, &wal_name)
            .map_err(|source| SafetyWalError::Io {
                path: path.clone(),
                source,
            })?;
        let append_state = WalAppendState::from_verified_stream_recovery(
            recovery.next_sequence,
            recovery.last_frame_hash,
        );
        Ok(Self {
            path,
            directory,
            wal_name,
            file,
            records: recovery.records,
            payload_bytes: recovery.payload_bytes,
            append_state,
            #[cfg(all(unix, not(target_os = "espidf")))]
            serviced_candidate_authority_minted: AtomicBool::new(false),
            #[cfg(all(unix, not(target_os = "espidf")))]
            leader_wire_authority_minted: AtomicBool::new(false),
        })
    }
    /// Return all records recovered during open.
    pub(crate) fn recovered_records(&self) -> &[RecoveredRecord] {
        &self.records
    }
    /// Compare this open WAL with one recovery-sealed canonical path.
    ///
    /// The path itself stays private so lifecycle startup can validate storage
    /// ownership without exposing a caller-selectable WAL target.
    pub(crate) fn matches_path(&self, expected: &Path) -> bool {
        self.path == expected
            && self
                .directory
                .verify_leaf(&self.file, &self.wal_name)
                .is_ok()
    }
    /// Mint the sole fixed serviced-candidate sibling authority.
    pub(crate) fn mint_serviced_candidate_store_authority(
        &self,
        expected: &Path,
    ) -> Result<SafetyWalServicedCandidateStoreAuthority, SafetyWalError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_expected_binding(expected)?;
            if self
                .serviced_candidate_authority_minted
                .swap(true, Ordering::AcqRel)
            {
                return Err(SafetyWalError::FailedClosed {
                    path: self.path.clone(),
                });
            }
            let entry = BoundSafetyWalAdjacentEntry::from_wal(
                Arc::clone(&self.directory),
                &self.path,
                ".serviced-candidates",
            )
            .map_err(|source| SafetyWalError::Io {
                path: self.path.clone(),
                source,
            })?;
            Ok(SafetyWalServicedCandidateStoreAuthority { entry })
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = expected;
            Err(SafetyWalError::UnsupportedStorageBinding {
                path: self.path.clone(),
                reason: "descriptor-relative adjacent storage is unavailable",
            })
        }
    }
    /// Mint the sole fixed leader-wire lifecycle sibling authority.
    pub(crate) fn mint_leader_wire_store_authority(
        &self,
        expected: &Path,
    ) -> Result<SafetyWalLeaderWireStoreAuthority, SafetyWalError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_expected_binding(expected)?;
            if self
                .leader_wire_authority_minted
                .swap(true, Ordering::AcqRel)
            {
                return Err(SafetyWalError::FailedClosed {
                    path: self.path.clone(),
                });
            }
            let entry = BoundSafetyWalAdjacentEntry::from_wal(
                Arc::clone(&self.directory),
                &self.path,
                ".leader-wire-lifecycles",
            )
            .map_err(|source| SafetyWalError::Io {
                path: self.path.clone(),
                source,
            })?;
            Ok(SafetyWalLeaderWireStoreAuthority { entry })
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = expected;
            Err(SafetyWalError::UnsupportedStorageBinding {
                path: self.path.clone(),
                reason: "descriptor-relative adjacent storage is unavailable",
            })
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_expected_binding(&self, expected: &Path) -> Result<(), SafetyWalError> {
        if self.path != expected {
            return Err(SafetyWalError::Io {
                path: expected.to_path_buf(),
                source: io::Error::new(
                    io::ErrorKind::InvalidData,
                    "expected path differs from the opened safety WAL",
                ),
            });
        }
        self.directory
            .verify_leaf(&self.file, &self.wal_name)
            .map_err(|source| SafetyWalError::Io {
                path: self.path.clone(),
                source,
            })
    }
    /// Append and synchronise an opaque Norito record.
    ///
    /// A successful return is the durability acknowledgement used by the reducer. On any error,
    /// callers must fail stop and reopen the WAL before attempting another consensus action.
    pub(crate) fn append(
        &mut self,
        payload: &[u8],
    ) -> Result<SafetyWalAppendReceipt, SafetyWalError> {
        self.append_with_limits(payload, WAL_RETENTION_LIMITS)
    }
    fn append_with_limits(
        &mut self,
        payload: &[u8],
        limits: WalRetentionLimits,
    ) -> Result<SafetyWalAppendReceipt, SafetyWalError> {
        if payload.len() > MAX_RECORD_BYTES {
            return Err(SafetyWalError::RecordTooLarge {
                actual: payload.len(),
                maximum: MAX_RECORD_BYTES,
            });
        }
        let (_, next_payload_bytes) = enforce_retention_limits(
            &self.path,
            self.records.len(),
            self.payload_bytes,
            payload.len(),
            limits,
        )?;
        let mut io = FileAppendIo {
            file: &mut self.file,
            directory: &self.directory,
            wal_name: &self.wal_name,
        };
        let receipt = self
            .append_state
            .append(payload, &frame_hash, &mut io)
            .map_err(|error| map_append_error(&self.path, error))?;
        let receipt = SafetyWalAppendReceipt {
            sequence: receipt.sequence(),
            frame_hash: receipt.frame_hash(),
        };
        let record = RecoveredRecord {
            sequence: receipt.sequence,
            frame_hash: receipt.frame_hash,
            payload: payload.to_vec(),
        };
        debug_assert!(record.exactly_matches_receipt(receipt));
        self.records.push(record);
        self.payload_bytes = next_payload_bytes;
        Ok(receipt)
    }
    /// Retire a closed height's WAL after the caller has validated Kura's
    /// durable block-and-finality receipt.
    ///
    /// Consuming the log prevents any safety intent from being appended after
    /// retirement. `authorization` can only be derived after the reducer has
    /// compared the exact typed Kura receipt and consumed the finalized height.
    pub(crate) fn retire(
        self,
        _authorization: WalRetirementAuthorization,
    ) -> Result<(), SafetyWalError> {
        let Self {
            path,
            directory,
            wal_name,
            file,
            ..
        } = self;
        remove_wal_file(path, &directory, &wal_name, file)
    }
}
fn remove_wal_file(
    path: PathBuf,
    directory: &BoundSafetyWalDirectory,
    wal_name: &OsStr,
    file: File,
) -> Result<(), SafetyWalError> {
    file.sync_all().map_err(|source| SafetyWalError::Io {
        path: path.clone(),
        source,
    })?;
    directory
        .verify_leaf(&file, wal_name)
        .map_err(|source| SafetyWalError::Io {
            path: path.clone(),
            source,
        })?;
    directory
        .unlink_exact_leaf(wal_name, &file)
        .map_err(|source| SafetyWalError::Io {
            path: path.clone(),
            source,
        })?;
    drop(file);
    Ok(())
}
struct StreamingWalRecovery {
    records: Vec<RecoveredRecord>,
    payload_bytes: usize,
    valid_prefix_len: u64,
    incomplete_tail: bool,
    next_sequence: u64,
    last_frame_hash: [u8; HASH_LEN],
}
fn enforce_retention_limits(
    path: &Path,
    current_records: usize,
    current_payload_bytes: usize,
    next_payload_bytes: usize,
    limits: WalRetentionLimits,
) -> Result<(usize, usize), SafetyWalError> {
    let records = current_records.checked_add(1).unwrap_or(usize::MAX);
    let payload_bytes = current_payload_bytes
        .checked_add(next_payload_bytes)
        .unwrap_or(usize::MAX);
    if records > limits.max_records || payload_bytes > limits.max_payload_bytes {
        return Err(SafetyWalError::RetentionLimitExceeded {
            path: path.to_path_buf(),
            records,
            payload_bytes,
            maximum_records: limits.max_records,
            maximum_payload_bytes: limits.max_payload_bytes,
        });
    }
    Ok((records, payload_bytes))
}
#[allow(clippy::too_many_lines)]
fn recover_wal_stream(
    file: &mut File,
    path: &Path,
    identity: WalFileIdentity,
    limits: WalRetentionLimits,
) -> Result<StreamingWalRecovery, SafetyWalError> {
    let file_len = file
        .metadata()
        .map_err(|source| SafetyWalError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    file.seek(SeekFrom::Start(0))
        .map_err(|source| SafetyWalError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut file_header = [0_u8; FILE_HEADER_LEN];
    let header_len = read_up_to(file, &mut file_header).map_err(|source| SafetyWalError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if header_len < FILE_HEADER_LEN {
        return Err(SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: "truncated header",
        });
    }
    recover_wal_file(&file_header, identity, &frame_hash)
        .map_err(|error| map_codec_error(path, error))?;
    let mut records = Vec::new();
    let mut payload_bytes = 0_usize;
    let mut valid_prefix_len = u64::try_from(FILE_HEADER_LEN).expect("WAL header length fits u64");
    let mut incomplete_tail = false;
    let mut expected_sequence = 0_u64;
    let mut previous_hash = [0_u8; HASH_LEN];
    while valid_prefix_len < file_len {
        let frame_start = valid_prefix_len;
        let mut frame_header = [0_u8; FRAME_HEADER_LEN];
        let frame_header_len =
            read_up_to(file, &mut frame_header).map_err(|source| SafetyWalError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        if frame_header_len < FRAME_HEADER_LEN {
            incomplete_tail = true;
            break;
        }
        let mut offset = 0_usize;
        if frame_header[offset..offset + FRAME_MAGIC.len()] != FRAME_MAGIC {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence: expected_sequence,
                    reason: WalFrameCorruption::Magic,
                },
            ));
        }
        offset += FRAME_MAGIC.len();
        let sequence = u64::from_le_bytes(
            frame_header[offset..offset + 8]
                .try_into()
                .expect("fixed-width WAL sequence"),
        );
        offset += 8;
        let payload_len = usize::try_from(u32::from_le_bytes(
            frame_header[offset..offset + 4]
                .try_into()
                .expect("fixed-width WAL payload length"),
        ))
        .unwrap_or(usize::MAX);
        offset += 4;
        let mut encoded_previous = [0_u8; HASH_LEN];
        encoded_previous.copy_from_slice(&frame_header[offset..offset + HASH_LEN]);
        if sequence != expected_sequence {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence: expected_sequence,
                    reason: WalFrameCorruption::Sequence,
                },
            ));
        }
        if payload_len > MAX_RECORD_BYTES {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::RecordLength,
                },
            ));
        }
        if encoded_previous != previous_hash {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::PreviousHash,
                },
            ));
        }
        let frame_len = FRAME_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|length| length.checked_add(HASH_LEN))
            .and_then(|length| u64::try_from(length).ok())
            .ok_or_else(|| {
                map_codec_error(
                    path,
                    WalCodecError::CorruptFrame {
                        sequence,
                        reason: WalFrameCorruption::RecordLength,
                    },
                )
            })?;
        if file_len.saturating_sub(frame_start) < frame_len {
            incomplete_tail = true;
            break;
        }
        let retention =
            enforce_retention_limits(path, records.len(), payload_bytes, payload_len, limits);
        let mut hasher = blake3::Hasher::new();
        hasher.update(&frame_header);
        let payload = if retention.is_ok() {
            let mut payload = vec![0_u8; payload_len];
            file.read_exact(&mut payload)
                .map_err(|source| SafetyWalError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            hasher.update(&payload);
            Some(payload)
        } else {
            let mut scratch = vec![0_u8; SAFETY_WAL_RECOVERY_SCRATCH_BYTES];
            let mut remaining = payload_len;
            while remaining > 0 {
                let chunk_len = remaining.min(scratch.len());
                let chunk = &mut scratch[..chunk_len];
                file.read_exact(chunk)
                    .map_err(|source| SafetyWalError::Io {
                        path: path.to_path_buf(),
                        source,
                    })?;
                hasher.update(chunk);
                remaining -= chunk_len;
            }
            None
        };
        let mut encoded_hash = [0_u8; HASH_LEN];
        file.read_exact(&mut encoded_hash)
            .map_err(|source| SafetyWalError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        let calculated_hash = *hasher.finalize().as_bytes();
        if encoded_hash != calculated_hash {
            return Err(map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::Checksum,
                },
            ));
        }
        if expected_sequence == u64::MAX {
            return Err(map_codec_error(path, WalCodecError::SequenceOverflow));
        }
        let (_, next_payload_total) = retention?;
        records.push(RecoveredRecord {
            sequence,
            frame_hash: encoded_hash,
            payload: payload.expect("payload is retained when its retention check succeeds"),
        });
        payload_bytes = next_payload_total;
        previous_hash = encoded_hash;
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| map_codec_error(path, WalCodecError::SequenceOverflow))?;
        valid_prefix_len = frame_start.checked_add(frame_len).ok_or_else(|| {
            map_codec_error(
                path,
                WalCodecError::CorruptFrame {
                    sequence,
                    reason: WalFrameCorruption::RecordLength,
                },
            )
        })?;
    }
    Ok(StreamingWalRecovery {
        records,
        payload_bytes,
        valid_prefix_len,
        incomplete_tail,
        next_sequence: expected_sequence,
        last_frame_hash: previous_hash,
    })
}
fn read_up_to(reader: &mut impl Read, buffer: &mut [u8]) -> io::Result<usize> {
    let mut read = 0_usize;
    while read < buffer.len() {
        match reader.read(&mut buffer[read..]) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(read)
}
struct FileAppendIo<'a> {
    file: &'a mut File,
    directory: &'a BoundSafetyWalDirectory,
    wal_name: &'a OsStr,
}
impl WalAppendIo for FileAppendIo<'_> {
    type Error = io::Error;
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.directory.verify_leaf(self.file, self.wal_name)?;
        self.file.write_all(bytes)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        self.file.flush()
    }
    fn sync_data(&mut self) -> Result<(), Self::Error> {
        self.file.sync_data()?;
        self.directory.verify_leaf(self.file, self.wal_name)
    }
}
fn map_append_error(path: &Path, error: WalAppendError<io::Error>) -> SafetyWalError {
    match error {
        WalAppendError::Codec(error) => map_codec_error(path, error),
        WalAppendError::Io { stage, source } => SafetyWalError::AppendIo {
            path: path.to_path_buf(),
            stage,
            source,
        },
        WalAppendError::FailedClosed => SafetyWalError::FailedClosed {
            path: path.to_path_buf(),
        },
    }
}
fn map_codec_error(path: &Path, error: WalCodecError) -> SafetyWalError {
    match error {
        WalCodecError::InvalidHeader(reason) => SafetyWalError::InvalidHeader {
            path: path.to_path_buf(),
            reason: match reason {
                WalHeaderCorruption::Truncated => "truncated header",
                WalHeaderCorruption::Magic => "magic mismatch",
                WalHeaderCorruption::FormatVersion => "unsupported format version",
                WalHeaderCorruption::Checksum => "checksum mismatch",
            },
        },
        WalCodecError::IdentityMismatch(field) => SafetyWalError::IdentityMismatch {
            path: path.to_path_buf(),
            field: match field {
                WalIdentityField::ProtocolVersion => "protocol version",
                WalIdentityField::NetworkId => "network id",
                WalIdentityField::ConsensusKeyHash => "consensus key hash",
            },
        },
        WalCodecError::CorruptFrame { sequence, reason } => SafetyWalError::CorruptFrame {
            path: path.to_path_buf(),
            sequence,
            reason: match reason {
                WalFrameCorruption::Magic => "frame magic mismatch",
                WalFrameCorruption::Sequence => "non-monotonic sequence",
                WalFrameCorruption::RecordLength => "record length exceeds safety bound",
                WalFrameCorruption::PreviousHash => "previous-frame hash mismatch",
                WalFrameCorruption::Checksum => "frame checksum mismatch",
            },
        },
        WalCodecError::RecordTooLarge { actual, maximum } => {
            SafetyWalError::RecordTooLarge { actual, maximum }
        }
        WalCodecError::SequenceOverflow => SafetyWalError::SequenceOverflow {
            path: path.to_path_buf(),
        },
    }
}
fn frame_hash(bytes: &[u8]) -> [u8; HASH_LEN] {
    *blake3::hash(bytes).as_bytes()
}
#[cfg(all(test, unix, not(target_os = "espidf")))]
mod tests {
    use super::*;
    const NETWORK_ID: [u8; HASH_LEN] = [0x11; HASH_LEN];
    const KEY: [u8; HASH_LEN] = [0x22; HASH_LEN];
    const PROTOCOL: u16 = iroha_data_model::block::consensus_v2::PROTOCOL_VERSION;
    fn read_test_u16(bytes: &[u8]) -> u16 {
        u16::from_le_bytes(bytes.try_into().expect("two-byte fixture field"))
    }
    #[test]
    fn file_header_uses_the_declared_canonical_layout() {
        let header =
            encode_wal_file_header(WalFileIdentity::new(PROTOCOL, NETWORK_ID, KEY), &frame_hash);
        let format_offset = FILE_MAGIC.len();
        let protocol_offset = format_offset + 2;
        let network_id_offset = protocol_offset + 2;
        let key_offset = network_id_offset + HASH_LEN;
        assert_eq!(&header[..FILE_MAGIC.len()], &FILE_MAGIC);
        assert_eq!(
            read_test_u16(&header[format_offset..protocol_offset]),
            FORMAT_VERSION
        );
        assert_eq!(
            read_test_u16(&header[protocol_offset..network_id_offset]),
            PROTOCOL
        );
        assert_eq!(&header[network_id_offset..key_offset], &NETWORK_ID);
        assert_eq!(&header[key_offset..FILE_HEADER_PREFIX_LEN], &KEY);
        assert_eq!(
            &header[FILE_HEADER_PREFIX_LEN..],
            &frame_hash(&header[..FILE_HEADER_PREFIX_LEN])
        );
    }
    #[test]
    fn append_reopens_and_replays_hash_chained_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let (prepare_receipt, commit_receipt) = {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            let prepare = wal.append(b"prepare").expect("append Prepare");
            let commit = wal.append(b"commit").expect("append Commit");
            assert_eq!(prepare.sequence(), 0);
            assert_eq!(commit.sequence(), 1);
            (prepare, commit)
        };
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("reopen WAL");
        assert_eq!(
            wal.recovered_records(),
            [
                RecoveredRecord {
                    sequence: 0,
                    frame_hash: prepare_receipt.frame_hash(),
                    payload: b"prepare".to_vec(),
                },
                RecoveredRecord {
                    sequence: 1,
                    frame_hash: commit_receipt.frame_hash(),
                    payload: b"commit".to_vec(),
                },
            ]
        );
        assert_eq!(wal.payload_bytes, b"prepare".len() + b"commit".len());
    }
    #[test]
    fn append_retention_limits_fail_before_file_or_hash_state_changes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let limits = WalRetentionLimits {
            max_records: 2,
            max_payload_bytes: 5,
        };
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let _first_receipt = wal
            .append_with_limits(b"ab", limits)
            .expect("append first bounded record");
        let _second_receipt = wal
            .append_with_limits(b"cde", limits)
            .expect("append exact payload boundary");
        let file_len = wal.file.metadata().expect("WAL metadata").len();
        let append_state = wal.append_state;
        assert!(matches!(
            wal.append_with_limits(b"", limits),
            Err(SafetyWalError::RetentionLimitExceeded {
                records: 3,
                payload_bytes: 5,
                maximum_records: 2,
                maximum_payload_bytes: 5,
                ..
            })
        ));
        assert_eq!(wal.file.metadata().expect("WAL metadata").len(), file_len);
        assert_eq!(wal.append_state, append_state);
        assert_eq!(wal.recovered_records().len(), 2);
        assert_eq!(wal.payload_bytes, 5);
        let payload_path = dir.path().join("sumeragi-v2-payload.wal");
        let mut payload_wal =
            SafetyWal::open(&payload_path, PROTOCOL, NETWORK_ID, KEY).expect("open payload WAL");
        let _payload_receipt = payload_wal
            .append_with_limits(b"12345", limits)
            .expect("append exact aggregate payload boundary");
        let payload_file_len = payload_wal
            .file
            .metadata()
            .expect("payload WAL metadata")
            .len();
        let payload_append_state = payload_wal.append_state;
        assert!(matches!(
            payload_wal.append_with_limits(b"x", limits),
            Err(SafetyWalError::RetentionLimitExceeded {
                records: 2,
                payload_bytes: 6,
                maximum_records: 2,
                maximum_payload_bytes: 5,
                ..
            })
        ));
        assert_eq!(
            payload_wal
                .file
                .metadata()
                .expect("payload WAL metadata")
                .len(),
            payload_file_len
        );
        assert_eq!(payload_wal.append_state, payload_append_state);
        assert_eq!(payload_wal.recovered_records().len(), 1);
        assert_eq!(payload_wal.payload_bytes, 5);
    }
    #[test]
    fn streaming_recovery_accepts_the_boundary_and_rejects_the_next_frame() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            let _one = wal.append(b"one").expect("append one");
            let _two = wal.append(b"two").expect("append two");
            let _three = wal.append(b"three").expect("append three");
        }
        let identity = WalFileIdentity::new(PROTOCOL, NETWORK_ID, KEY);
        let mut file = File::open(&path).expect("open WAL for streaming recovery");
        let exact = recover_wal_stream(
            &mut file,
            &path,
            identity,
            WalRetentionLimits {
                max_records: 3,
                max_payload_bytes: 11,
            },
        )
        .expect("recover exact record and payload boundary");
        assert_eq!(exact.records.len(), 3);
        assert_eq!(exact.payload_bytes, 11);
        assert_eq!(exact.next_sequence, 3);
        let error = match recover_wal_stream(
            &mut file,
            &path,
            identity,
            WalRetentionLimits {
                max_records: 2,
                max_payload_bytes: usize::MAX,
            },
        ) {
            Ok(_) => panic!("the first complete frame above the retention bound must fail closed"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            SafetyWalError::RetentionLimitExceeded {
                records: 3,
                payload_bytes: 11,
                maximum_records: 2,
                maximum_payload_bytes: usize::MAX,
                ..
            }
        ));
    }
    #[test]
    fn open_wal_matches_only_its_exact_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        assert!(wal.matches_path(&path));
        assert!(!wal.matches_path(&dir.path().join("foreign.wal")));
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let kura = crate::kura::Kura::blank_kura_for_testing();
            let foreign_kura = crate::kura::Kura::blank_kura_for_testing();
            let rejected = SafetyWal::open_with_kura_authority(
                foreign_kura.as_ref(),
                kura.mint_safety_wal_directory_authority()
                    .expect("mint foreign-bound authority"),
                "00000000000000000001.wal",
                PROTOCOL,
                NETWORK_ID,
                KEY,
            );
            assert!(matches!(rejected, Err(SafetyWalError::Io { .. })));
            let expected = kura
                .sumeragi_v2_storage_root()
                .join("wal")
                .join("00000000000000000001.wal");
            let bound = SafetyWal::open_with_kura_authority(
                kura.as_ref(),
                kura.mint_safety_wal_directory_authority()
                    .expect("mint exact Kura authority"),
                "00000000000000000001.wal",
                PROTOCOL,
                NETWORK_ID,
                KEY,
            )
            .expect("open Kura-bound WAL");
            assert!(bound.matches_path(&expected));
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn open_rejects_a_preexisting_symlink_for_the_owned_wal_directory() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("tempdir");
        let foreign = root.path().join("foreign-wal-directory");
        fs::create_dir(&foreign).expect("create foreign WAL directory");
        let parent = root.path().join("wal");
        symlink(&foreign, &parent).expect("substitute the WAL directory with a symlink");
        let path = parent.join("sumeragi-v2.wal");
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY),
            Err(SafetyWalError::Io { .. })
        ));
        assert!(!foreign.join("sumeragi-v2.wal").exists());
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    fn substitute_wal_parent(root: &Path, parent: &Path) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::symlink;
        let retained = root.join("retained-wal-directory");
        let foreign = root.join("foreign-wal-directory");
        fs::rename(parent, &retained).expect("move the opened WAL directory");
        fs::create_dir(&foreign).expect("create foreign WAL directory");
        symlink(&foreign, parent).expect("substitute the canonical WAL directory name");
        (retained, foreign)
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn parent_substitution_poisoning_prevents_wal_append_acknowledgement() {
        let root = tempfile::tempdir().expect("tempdir");
        let parent = root.path().join("wal");
        let path = parent.join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let (retained, foreign) = substitute_wal_parent(root.path(), &parent);
        assert!(!wal.matches_path(&path));
        assert!(matches!(
            wal.append(b"must not receive a durability receipt"),
            Err(SafetyWalError::AppendIo {
                stage: WalIoStage::Write,
                ..
            })
        ));
        assert!(wal.append_state.is_failed_closed());
        assert!(wal.recovered_records().is_empty());
        assert!(retained.join("sumeragi-v2.wal").is_file());
        assert!(!foreign.join("sumeragi-v2.wal").exists());
        assert!(matches!(
            wal.mint_leader_wire_store_authority(&path),
            Err(SafetyWalError::Io { .. })
        ));
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn adjacent_authorities_reject_parent_substitution_without_path_fallback() {
        let root = tempfile::tempdir().expect("tempdir");
        let parent = root.path().join("wal");
        let path = parent.join("sumeragi-v2.wal");
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let serviced = wal
            .mint_serviced_candidate_store_authority(&path)
            .expect("mint serviced-candidate authority");
        let leader = wal
            .mint_leader_wire_store_authority(&path)
            .expect("mint leader-wire authority");
        assert!(matches!(
            wal.mint_serviced_candidate_store_authority(&path),
            Err(SafetyWalError::FailedClosed { .. })
        ));
        assert!(matches!(
            wal.mint_leader_wire_store_authority(&path),
            Err(SafetyWalError::FailedClosed { .. })
        ));
        let (retained, foreign) = substitute_wal_parent(root.path(), &parent);
        assert!(serviced.read_bounded(1024).is_err());
        assert!(leader.publish_atomic(b"must not publish", 1024).is_err());
        for directory in [&retained, &foreign] {
            assert!(
                !directory
                    .join("sumeragi-v2.wal.serviced-candidates")
                    .exists()
            );
            assert!(
                !directory
                    .join("sumeragi-v2.wal.leader-wire-lifecycles")
                    .exists()
            );
        }
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn adjacent_authority_bounds_publish_read_and_retirement() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join("wal").join("sumeragi-v2.wal");
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let leader = wal
            .mint_leader_wire_store_authority(&path)
            .expect("mint leader-wire authority");
        let adjacent = path.with_file_name("sumeragi-v2.wal.leader-wire-lifecycles");
        assert!(leader.publish_atomic(b"oversized", 4).is_err());
        leader
            .publish_atomic(b"bounded", 7)
            .expect("publish bounded adjacent bytes");
        assert_eq!(
            leader.read_bounded(7).expect("read bounded bytes"),
            Some(b"bounded".to_vec())
        );
        leader.retire(7).expect("retire exact adjacent entry");
        assert!(!adjacent.exists());
    }
    #[test]
    fn incomplete_final_frame_is_truncated_as_unacknowledged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            let _receipt = wal.append(b"durable").expect("append durable record");
        }
        let good_len = fs::metadata(&path).expect("metadata").len();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open append")
            .write_all(b"S2FR\x01\x00")
            .expect("write crash tail");
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("recover WAL");
        assert_eq!(wal.recovered_records().len(), 1);
        assert_eq!(fs::metadata(path).expect("metadata").len(), good_len);
    }
    #[test]
    fn incomplete_final_payload_and_checksum_are_discarded_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let good_len;
        let durable_receipt;
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            durable_receipt = wal
                .append(b"durable decision")
                .expect("append acknowledged decision");
            good_len = wal.file.metadata().expect("metadata").len();
            let _unacknowledged_receipt = wal
                .append(b"unacknowledged next intent")
                .expect("append frame used to model partial write");
        }
        let partial_len = good_len
            .checked_add(u64::try_from(FRAME_HEADER_LEN + 3).expect("partial length"))
            .expect("test file length");
        OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open WAL for truncation")
            .set_len(partial_len)
            .expect("truncate in final payload");
        let wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("recover WAL");
        assert_eq!(
            wal.recovered_records(),
            [RecoveredRecord {
                sequence: 0,
                frame_hash: durable_receipt.frame_hash(),
                payload: b"durable decision".to_vec(),
            }]
        );
        assert_eq!(fs::metadata(path).expect("metadata").len(), good_len);
    }
    #[test]
    fn complete_corrupt_frame_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            let _receipt = wal.append(b"prepare").expect("append record");
        }
        let mut bytes = fs::read(&path).expect("read WAL");
        let payload_offset = FILE_HEADER_LEN + FRAME_HEADER_LEN;
        bytes[payload_offset] ^= 0x80;
        fs::write(&path, bytes).expect("corrupt WAL");
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY),
            Err(SafetyWalError::CorruptFrame { .. })
        ));
    }
    #[test]
    fn complete_hash_chain_break_after_valid_record_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let first_payload_len = b"prepare".len();
        {
            let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
            let _first_receipt = wal.append(b"prepare").expect("append first record");
            let _second_receipt = wal.append(b"decision").expect("append second record");
        }
        let second_frame = FILE_HEADER_LEN + FRAME_HEADER_LEN + first_payload_len + HASH_LEN;
        let previous_hash_offset = second_frame + FRAME_MAGIC.len() + 8 + 4;
        let mut bytes = fs::read(&path).expect("read WAL");
        bytes[previous_hash_offset] ^= 0x80;
        fs::write(&path, bytes).expect("break hash chain");
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY),
            Err(SafetyWalError::CorruptFrame {
                sequence: 1,
                reason: "previous-frame hash mismatch",
                ..
            })
        ));
    }
    #[test]
    fn identity_mismatch_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        drop(SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL"));
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, NETWORK_ID, [0x33; HASH_LEN]),
            Err(SafetyWalError::IdentityMismatch {
                field: "consensus key hash",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL.saturating_add(1), NETWORK_ID, KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "protocol version",
                ..
            })
        ));
        assert!(matches!(
            SafetyWal::open(&path, PROTOCOL, [0x44; HASH_LEN], KEY),
            Err(SafetyWalError::IdentityMismatch {
                field: "network id",
                ..
            })
        ));
    }
    #[test]
    fn append_io_failure_poisoning_requires_verified_reopen() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let read_only = File::open(&path).expect("open read-only WAL handle");
        let writable = std::mem::replace(&mut wal.file, read_only);
        drop(writable);
        assert!(matches!(
            wal.append(b"must fail before acknowledgement"),
            Err(SafetyWalError::AppendIo {
                stage: WalIoStage::Write,
                ..
            })
        ));
        assert!(wal.append_state.is_failed_closed());
        assert!(matches!(
            wal.append(b"retry is forbidden"),
            Err(SafetyWalError::FailedClosed { .. })
        ));
        assert!(wal.recovered_records().is_empty());
        drop(wal);
        let reopened = SafetyWal::open(path, PROTOCOL, NETWORK_ID, KEY).expect("verified reopen");
        assert!(reopened.recovered_records().is_empty());
        assert!(!reopened.append_state.is_failed_closed());
    }
    #[test]
    fn physical_retirement_removes_and_directory_syncs_a_closed_height_log() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sumeragi-v2.wal");
        let mut wal = SafetyWal::open(&path, PROTOCOL, NETWORK_ID, KEY).expect("open WAL");
        let _receipt = wal.append(b"decision").expect("append decision");
        let SafetyWal {
            path,
            directory,
            wal_name,
            file,
            ..
        } = wal;
        let retired_path = path.clone();
        remove_wal_file(path, &directory, &wal_name, file).expect("retire finalized WAL bytes");
        assert!(!retired_path.exists());
    }
}

#[cfg(all(test, not(all(unix, not(target_os = "espidf")))))]
mod unsupported_platform_tests {
    use super::*;

    #[test]
    fn lexical_test_open_fails_closed_without_descriptor_relative_storage() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let parent = directory.path().join("must-not-be-created");
        let path = parent.join("sumeragi-v2.wal");
        let result = SafetyWal::open(
            path,
            iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
            [0x11; HASH_LEN],
            [0x22; HASH_LEN],
        );
        assert!(matches!(
            result,
            Err(SafetyWalError::UnsupportedStorageBinding { .. })
        ));
        assert!(
            !parent.exists(),
            "unsupported test opening must fail before filesystem mutation"
        );
    }
}
