//! Crash-safe time-floor persistence for the private Musubi publication service.
#[cfg(unix)]
use super::publication_filesystem_owner_probe;
use super::{
    MusubiPublicationServiceBackendErrorV1, MusubiPublicationServiceClockV1,
    MusubiPublicationSystemClockV1,
};
#[cfg(unix)]
use crate::musubi_archive_fetch::{
    secure_directory_open_flags, secure_no_follow_nonblocking_flags,
};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
};
const CLOCK_STATE_FILE: &str = "clock-floor-v1.norito";
const CLOCK_LOCK_FILE: &str = "clock-floor-v1.lock";
const CLOCK_NEXT_FILE: &str = "clock-floor-v1.next";
const CLOCK_STATE_DOMAIN_V1: [u8; 32] = *b"musubi-pub-clock-floor-v1\0\0\0\0\0\0\0";
const CLOCK_STATE_SCHEMA_V1: u8 = 1;
const MAX_CLOCK_STATE_BYTES: usize = 4 * 1024;
/// Stable failure opening a durable private-publication clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableMusubiPublicationServiceClockOpenErrorV1 {
    /// V1 cannot provide its required filesystem guarantees on this platform.
    UnsupportedPlatform,
    /// The configured directory is missing, shared, linked, or otherwise unsafe.
    UnsafeRoot,
    /// Another process already owns the clock state.
    Locked,
    /// Ordinary startup found no previously initialized durable clock state.
    Uninitialized,
    /// One-time initialization was requested for a directory that was not empty.
    AlreadyInitialized,
    /// The persisted state is malformed, noncanonical, inconsistent, or corrupt.
    InvalidState,
    /// The injected trusted clock could not be sampled during startup.
    SourceUnavailable,
    /// The injected clock is behind the durably committed high-water mark.
    ClockRollback,
    /// The private state could not be read or durably replaced.
    StorageUnavailable,
}
impl DurableMusubiPublicationServiceClockOpenErrorV1 {
    /// Return the stable operator-facing error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "MUSUBI_PUBLICATION_CLOCK_UNSUPPORTED_PLATFORM",
            Self::UnsafeRoot => "MUSUBI_PUBLICATION_CLOCK_UNSAFE_ROOT",
            Self::Locked => "MUSUBI_PUBLICATION_CLOCK_LOCKED",
            Self::Uninitialized => "MUSUBI_PUBLICATION_CLOCK_UNINITIALIZED",
            Self::AlreadyInitialized => "MUSUBI_PUBLICATION_CLOCK_ALREADY_INITIALIZED",
            Self::InvalidState => "MUSUBI_PUBLICATION_CLOCK_INVALID_STATE",
            Self::SourceUnavailable => "MUSUBI_PUBLICATION_CLOCK_SOURCE_UNAVAILABLE",
            Self::ClockRollback => "MUSUBI_PUBLICATION_CLOCK_ROLLBACK",
            Self::StorageUnavailable => "MUSUBI_PUBLICATION_CLOCK_STORAGE_UNAVAILABLE",
        }
    }
}
impl fmt::Display for DurableMusubiPublicationServiceClockOpenErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
impl std::error::Error for DurableMusubiPublicationServiceClockOpenErrorV1 {}
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurableClockStateV1 {
    domain: [u8; 32],
    schema: u8,
    revision: u64,
    floor_ms: u64,
}
impl DurableClockStateV1 {
    fn new(floor_ms: u64) -> Self {
        Self {
            domain: CLOCK_STATE_DOMAIN_V1,
            schema: CLOCK_STATE_SCHEMA_V1,
            revision: 1,
            floor_ms,
        }
    }
    fn validate(&self) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
        if self.domain != CLOCK_STATE_DOMAIN_V1
            || self.schema != CLOCK_STATE_SCHEMA_V1
            || self.revision == 0
            || self.floor_ms == 0
        {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
        }
        Ok(())
    }
    fn digest(&self) -> Result<[u8; 32], DurableMusubiPublicationServiceClockOpenErrorV1> {
        let encoded = norito::encode_canonical(self)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
        let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:publication-clock-floor:v1");
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurableClockEnvelopeV1 {
    state: DurableClockStateV1,
    state_digest: [u8; 32],
}
impl DurableClockEnvelopeV1 {
    fn new(
        state: DurableClockStateV1,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        let state_digest = state.digest()?;
        Ok(Self {
            state,
            state_digest,
        })
    }
    fn validate(&self) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
        self.state.validate()?;
        if self.state.digest()? != self.state_digest {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
        }
        Ok(())
    }
}
/// Restart-persistent non-regressing clock for a private Musubi publication service.
///
/// The caller supplies one existing dedicated directory. On Unix it must be a real directory
/// owned consistently with all state files and have mode `0700`. The clock holds an exclusive
/// process lock, commits a canonical Norito high-water record with file and directory `fsync`,
/// and returns a newly observed time only after that floor is durable. State contains no signing
/// material, credentials, request bodies, or provider tokens.
///
/// V1 fails closed on non-Unix platforms until an equivalently race-safe replacement primitive
/// is qualified there.
// TODO: Bind the floor/revision to a deployment-sealed monotonic CAS and replace pathname child
// mutation with qualified directory-relative primitives before production rollout.
pub struct DurableMusubiPublicationServiceClockV1 {
    source: Box<dyn MusubiPublicationServiceClockV1>,
    root: PathBuf,
    root_identity: PrivateFileIdentity,
    root_owner: u32,
    root_handle: File,
    lock_handle: File,
    lock_identity: PrivateFileIdentity,
    state_identity: PrivateFileIdentity,
    state: DurableClockStateV1,
    poisoned: bool,
}
#[derive(Clone, Copy)]
struct ClockStorageContext<'a> {
    root: &'a Path,
    root_handle: &'a File,
    root_identity: PrivateFileIdentity,
    root_owner: u32,
    lock_handle: &'a File,
    lock_identity: PrivateFileIdentity,
}
impl fmt::Debug for DurableMusubiPublicationServiceClockV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableMusubiPublicationServiceClockV1")
            .field("revision", &self.state.revision)
            .field("floor_ms", &self.state.floor_ms)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}
impl DurableMusubiPublicationServiceClockV1 {
    /// Explicitly initialize one empty private state directory.
    ///
    /// Initialization is deliberately separate from [`Self::open`]. This method refuses a
    /// nonempty directory, so ordinary restart can never mistake deleted rollback state for first
    /// boot. If a process crashes after installing the owner lock but before installing the first
    /// floor, the directory remains fail-closed and requires explicit operator recovery.
    ///
    /// # Errors
    ///
    /// Returns a stable, path-free category when the directory is not empty or filesystem safety,
    /// exclusivity, source availability, or durable initialization cannot be established.
    pub fn initialize(
        root: &Path,
        source: Box<dyn MusubiPublicationServiceClockV1>,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        Self::open_inner(root, source, true)
    }
    /// Open existing private state and durably advance it to the source's current time.
    ///
    /// Only one process may hold a state directory at a time. Existing state is decoded under a
    /// fixed resource bound and must be canonical, integrity-bound, private, singly linked, and
    /// stable across opening. A source time below the stored floor is rejected before traffic can
    /// reach the publication service.
    ///
    /// # Errors
    ///
    /// Returns a stable, path-free category when filesystem safety, exclusivity, state integrity,
    /// source availability, or monotonicity cannot be established.
    pub fn open(
        root: &Path,
        source: Box<dyn MusubiPublicationServiceClockV1>,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        Self::open_inner(root, source, false)
    }
    fn open_inner(
        root: &Path,
        mut source: Box<dyn MusubiPublicationServiceClockV1>,
        initialize: bool,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        if !cfg!(unix) {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsupportedPlatform);
        }
        let (root, root_handle, root_identity, root_owner) = open_private_root(root)?;
        let initialization_sample = if initialize {
            ensure_empty_initialization_root(&root)?;
            let sampled = sample_startup_source(source.as_mut())?;
            ensure_empty_initialization_root(&root)?;
            Some(sampled)
        } else {
            None
        };
        let lock_mode = if initialize {
            ClockLockOpenMode::CreateNew
        } else {
            ClockLockOpenMode::Existing
        };
        let (lock_handle, lock_identity) = open_and_lock(&root, root_owner, lock_mode)?;
        let storage = ClockStorageContext {
            root: &root,
            root_handle: &root_handle,
            root_identity,
            root_owner,
            lock_handle: &lock_handle,
            lock_identity,
        };
        reconcile_directory(
            &root,
            &root_handle,
            root_identity,
            root_owner,
            &lock_handle,
            lock_identity,
        )?;
        let state_path = root.join(CLOCK_STATE_FILE);
        let loaded = read_state(&state_path, root_owner)?;
        if initialize && loaded.is_some() {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized);
        }
        if !initialize && loaded.is_none() {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized);
        }
        let sampled = match initialization_sample {
            Some(sampled) => sampled,
            None => sample_startup_source(source.as_mut())?,
        };
        let (state, state_identity) = match loaded {
            Some((state, _)) if sampled < state.floor_ms => {
                return Err(DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback);
            }
            Some((mut state, state_identity)) => {
                validate_live_state(storage, state_identity, &state)?;
                if sampled > state.floor_ms {
                    let previous_state = state.clone();
                    state.revision = state
                        .revision
                        .checked_add(1)
                        .ok_or(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
                    state.floor_ms = sampled;
                    let state_identity =
                        write_state(storage, Some(state_identity), Some(&previous_state), &state)?;
                    (state, state_identity)
                } else {
                    (state, state_identity)
                }
            }
            None => {
                debug_assert!(initialize);
                let state = DurableClockStateV1::new(sampled);
                let state_identity = write_state(storage, None, None, &state)?;
                (state, state_identity)
            }
        };
        state.validate()?;
        Ok(Self {
            source,
            root,
            root_identity,
            root_owner,
            root_handle,
            lock_handle,
            lock_identity,
            state_identity,
            state,
            poisoned: false,
        })
    }
    /// Open a durable wrapper around the raw system wall clock.
    ///
    /// This is suitable only when the supplied directory is on storage whose durability
    /// guarantees have been qualified for the deployment.
    ///
    /// # Errors
    ///
    /// Returns the same stable startup categories as [`Self::open`].
    pub fn open_system(
        root: &Path,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        Self::open(root, Box::new(MusubiPublicationSystemClockV1))
    }
    /// Explicitly initialize a durable wrapper around the raw system wall clock.
    ///
    /// # Errors
    ///
    /// Returns the same stable initialization categories as [`Self::initialize`].
    pub fn initialize_system(
        root: &Path,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        Self::initialize(root, Box::new(MusubiPublicationSystemClockV1))
    }
    /// Return the last durably committed Unix-millisecond floor.
    #[must_use]
    pub const fn durable_floor_ms(&self) -> u64 {
        self.state.floor_ms
    }
    fn storage_context(&self) -> ClockStorageContext<'_> {
        ClockStorageContext {
            root: &self.root,
            root_handle: &self.root_handle,
            root_identity: self.root_identity,
            root_owner: self.root_owner,
            lock_handle: &self.lock_handle,
            lock_identity: self.lock_identity,
        }
    }
}
impl MusubiPublicationServiceClockV1 for DurableMusubiPublicationServiceClockV1 {
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
        if self.poisoned {
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        if validate_live_state(self.storage_context(), self.state_identity, &self.state).is_err() {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        let sampled = self.source.current_time_ms()?;
        if validate_live_state(self.storage_context(), self.state_identity, &self.state).is_err() {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        if sampled == 0 || sampled < self.state.floor_ms {
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        if sampled == self.state.floor_ms {
            return Ok(sampled);
        }
        let Some(revision) = self.state.revision.checked_add(1) else {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Permanent);
        };
        let next = DurableClockStateV1 {
            revision,
            floor_ms: sampled,
            ..self.state.clone()
        };
        let Ok(state_identity) = write_state(
            self.storage_context(),
            Some(self.state_identity),
            Some(&self.state),
            &next,
        ) else {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        };
        self.state_identity = state_identity;
        self.state = next;
        Ok(sampled)
    }
}
fn open_private_root(
    root: &Path,
) -> Result<
    (PathBuf, File, PrivateFileIdentity, u32),
    DurableMusubiPublicationServiceClockOpenErrorV1,
> {
    let linked = fs::symlink_metadata(root)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot)?;
    validate_private_root(&linked)?;
    let canonical = fs::canonicalize(root)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot)?;
    let canonical_metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot)?;
    validate_private_root(&canonical_metadata)?;
    if !same_file(&linked, &canonical_metadata) {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    let filesystem_owner = publication_filesystem_owner_probe(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    #[cfg(unix)]
    if metadata_owner(&linked) != filesystem_owner
        || metadata_owner(&canonical_metadata) != filesystem_owner
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_directory_open_flags());
    let handle = options
        .open(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let opened = handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_root(&opened)?;
    if !same_file(&canonical_metadata, &opened) {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    if metadata_owner(&opened) != filesystem_owner {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    Ok((
        canonical,
        handle,
        PrivateFileIdentity::from_metadata(&opened),
        metadata_owner(&opened),
    ))
}
fn sample_startup_source(
    source: &mut dyn MusubiPublicationServiceClockV1,
) -> Result<u64, DurableMusubiPublicationServiceClockOpenErrorV1> {
    let sampled = source
        .current_time_ms()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::SourceUnavailable)?;
    if sampled == 0 {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::SourceUnavailable);
    }
    Ok(sampled)
}
#[derive(Clone, Copy)]
enum ClockLockOpenMode {
    Existing,
    CreateNew,
}
fn ensure_empty_initialization_root(
    root: &Path,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let mut entries = fs::read_dir(root)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    if entries
        .next()
        .transpose()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?
        .is_some()
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized);
    }
    Ok(())
}
fn open_and_lock(
    root: &Path,
    root_owner: u32,
    mode: ClockLockOpenMode,
) -> Result<(File, PrivateFileIdentity), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let path = root.join(CLOCK_LOCK_FILE);
    let before = optional_metadata(&path)?;
    match (mode, before.is_some()) {
        (ClockLockOpenMode::Existing, false) => {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized);
        }
        (ClockLockOpenMode::CreateNew, true) => {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized);
        }
        _ => {}
    }
    if let Some(metadata) = &before {
        validate_private_file(metadata, root_owner)?;
        if metadata.len() != 0 {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).truncate(false);
    match mode {
        ClockLockOpenMode::Existing => {}
        ClockLockOpenMode::CreateNew => {
            options.create_new(true);
        }
    }
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(secure_no_follow_nonblocking_flags());
    let file = options
        .open(&path)
        .map_err(|error| match (mode, error.kind()) {
            (ClockLockOpenMode::Existing, io::ErrorKind::NotFound) => {
                DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
            }
            (ClockLockOpenMode::CreateNew, io::ErrorKind::AlreadyExists) => {
                DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized
            }
            _ => DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable,
        })?;
    if before.is_none() {
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    }
    let opened = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened, root_owner)?;
    if opened.len() != 0 {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    if before
        .as_ref()
        .is_some_and(|metadata| !same_file(metadata, &opened))
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    let named = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&named, root_owner)?;
    if named.len() != 0 || !same_file(&opened, &named) {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => DurableMusubiPublicationServiceClockOpenErrorV1::Locked,
        fs::TryLockError::Error(_) => {
            DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable
        }
    })?;
    let after = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&after, root_owner)?;
    if after.len() != 0 || !same_file(&opened, &after) {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    Ok((file, PrivateFileIdentity::from_metadata(&opened)))
}
fn reconcile_directory(
    root: &Path,
    root_handle: &File,
    root_identity: PrivateFileIdentity,
    root_owner: u32,
    lock_handle: &File,
    lock_identity: PrivateFileIdentity,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    let mut remove_next = false;
    for entry in fs::read_dir(root)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?
    {
        let entry = entry
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        let name = entry.file_name();
        if name == CLOCK_LOCK_FILE || name == CLOCK_STATE_FILE {
            continue;
        }
        if name == CLOCK_NEXT_FILE && !remove_next {
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
            validate_private_file(&metadata, root_owner)?;
            if usize::try_from(metadata.len())
                .ok()
                .is_none_or(|length| length > MAX_CLOCK_STATE_BYTES)
            {
                return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
            }
            remove_next = true;
            continue;
        }
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    if remove_next {
        let path = root.join(CLOCK_NEXT_FILE);
        let before = fs::symlink_metadata(&path)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        validate_private_file(&before, root_owner)?;
        fs::remove_file(&path)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        root_handle
            .sync_all()
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    }
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)
}
fn read_state(
    path: &Path,
    root_owner: u32,
) -> Result<
    Option<(DurableClockStateV1, PrivateFileIdentity)>,
    DurableMusubiPublicationServiceClockOpenErrorV1,
> {
    let Some(named_before) = optional_metadata(path)? else {
        return Ok(None);
    };
    validate_private_file(&named_before, root_owner)?;
    if named_before.len() == 0
        || usize::try_from(named_before.len())
            .ok()
            .is_none_or(|length| length > MAX_CLOCK_STATE_BYTES)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_no_follow_nonblocking_flags());
    let mut file = options
        .open(path)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let opened_before = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_before, root_owner)?;
    if opened_before.len() == 0
        || usize::try_from(opened_before.len())
            .ok()
            .is_none_or(|length| length > MAX_CLOCK_STATE_BYTES)
        || !same_file_version(&named_before, &opened_before)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(opened_before.len())
            .unwrap_or(MAX_CLOCK_STATE_BYTES)
            .min(MAX_CLOCK_STATE_BYTES),
    );
    std::io::Read::by_ref(&mut file)
        .take(u64::try_from(MAX_CLOCK_STATE_BYTES).expect("clock-state bound fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let opened_after = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let named_after = fs::symlink_metadata(path)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_after, root_owner)?;
    validate_private_file(&named_after, root_owner)?;
    if bytes.len() > MAX_CLOCK_STATE_BYTES
        || u64::try_from(bytes.len()).ok() != Some(opened_before.len())
        || !same_file_version(&opened_before, &opened_after)
        || !same_file_version(&opened_after, &named_after)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let envelope: DurableClockEnvelopeV1 = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(64, MAX_CLOCK_STATE_BYTES, 128, 64 * 1024, 16),
    )
    .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    envelope.validate()?;
    Ok(Some((
        envelope.state,
        PrivateFileIdentity::from_metadata(&opened_after),
    )))
}
fn write_state(
    storage: ClockStorageContext<'_>,
    expected_state_identity: Option<PrivateFileIdentity>,
    expected_state: Option<&DurableClockStateV1>,
    state: &DurableClockStateV1,
) -> Result<PrivateFileIdentity, DurableMusubiPublicationServiceClockOpenErrorV1> {
    let ClockStorageContext {
        root,
        root_handle,
        root_identity,
        root_owner,
        lock_handle,
        lock_identity,
    } = storage;
    state.validate()?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    let envelope = DurableClockEnvelopeV1::new(state.clone())?;
    let bytes = norito::encode_canonical(&envelope)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    if bytes.is_empty() || bytes.len() > MAX_CLOCK_STATE_BYTES {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let target = root.join(CLOCK_STATE_FILE);
    validate_persisted_state(&target, expected_state_identity, expected_state, root_owner)?;
    let mut pending = PrivateTemporaryFile::create(root, root_owner)?;
    pending
        .file
        .write_all(&bytes)
        .and_then(|()| pending.file.flush())
        .and_then(|()| pending.file.sync_all())
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_pending_state(&pending, state, root_owner)?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    validate_persisted_state(&target, expected_state_identity, expected_state, root_owner)?;
    validate_pending_state(&pending, state, root_owner)?;
    fs::rename(&pending.path, &target)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    pending.disarm();
    let installed = fs::symlink_metadata(&target)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&installed, root_owner)?;
    if !pending.identity.matches(&installed)
        || installed.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    root_handle
        .sync_all()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    let Some((final_state, final_identity)) = read_state(&target, root_owner)? else {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    };
    if final_state != *state || final_identity != pending.identity {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    Ok(pending.identity)
}
fn validate_pending_state(
    pending: &PrivateTemporaryFile,
    expected_state: &DurableClockStateV1,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    pending.validate(root_owner)?;
    let Some((actual_state, actual_identity)) = read_state(&pending.path, root_owner)? else {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    };
    if actual_identity != pending.identity || actual_state != *expected_state {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}
fn validate_persisted_state(
    path: &Path,
    expected: Option<PrivateFileIdentity>,
    expected_state: Option<&DurableClockStateV1>,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    match (expected, expected_state) {
        (None, None) => match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable),
        },
        (Some(expected), Some(expected_state)) => {
            let Some((actual_state, actual_identity)) = read_state(path, root_owner)? else {
                return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
            };
            if actual_identity == expected && actual_state == *expected_state {
                Ok(())
            } else {
                Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)
            }
        }
        _ => Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable),
    }
}
fn validate_lock_identity(
    root: &Path,
    lock_handle: &File,
    identity: PrivateFileIdentity,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let path = root.join(CLOCK_LOCK_FILE);
    let named = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let opened = lock_handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&named, root_owner)?;
    validate_private_file(&opened, root_owner)?;
    if named.len() != 0
        || opened.len() != 0
        || !identity.matches(&named)
        || !identity.matches(&opened)
        || !same_file(&named, &opened)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}
fn validate_live_state(
    storage: ClockStorageContext<'_>,
    state_identity: PrivateFileIdentity,
    expected_state: &DurableClockStateV1,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let ClockStorageContext {
        root,
        root_handle,
        root_identity,
        root_owner,
        lock_handle,
        lock_identity,
    } = storage;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    validate_persisted_state(
        &root.join(CLOCK_STATE_FILE),
        Some(state_identity),
        Some(expected_state),
        root_owner,
    )
}
fn validate_root_identity(
    root: &Path,
    root_handle: &File,
    identity: PrivateFileIdentity,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let named = fs::symlink_metadata(root)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    let opened = root_handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_private_root(&named)?;
    validate_private_root(&opened)?;
    if metadata_owner(&named) != root_owner
        || metadata_owner(&opened) != root_owner
        || !identity.matches(&named)
        || !identity.matches(&opened)
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}
fn optional_metadata(
    path: &Path,
) -> Result<Option<fs::Metadata>, DurableMusubiPublicationServiceClockOpenErrorV1> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable),
    }
}
fn validate_private_root(
    metadata: &fs::Metadata,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    if metadata.mode() & 0o7777 != 0o700 {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot);
    }
    Ok(())
}
fn validate_private_file(
    metadata: &fs::Metadata,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    #[cfg(not(unix))]
    let _ = root_owner;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    #[cfg(unix)]
    if metadata.mode() & 0o7777 != 0o600 || metadata.nlink() != 1 || metadata.uid() != root_owner {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    Ok(())
}
struct PrivateTemporaryFile {
    path: PathBuf,
    file: File,
    identity: PrivateFileIdentity,
    armed: bool,
}
impl PrivateTemporaryFile {
    fn create(
        root: &Path,
        root_owner: u32,
    ) -> Result<Self, DurableMusubiPublicationServiceClockOpenErrorV1> {
        let path = root.join(CLOCK_NEXT_FILE);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options
            .open(&path)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        let metadata = file
            .metadata()
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        let pending = Self {
            path,
            file,
            identity: PrivateFileIdentity::from_metadata(&metadata),
            armed: true,
        };
        pending.validate(root_owner)?;
        Ok(pending)
    }
    fn validate(
        &self,
        root_owner: u32,
    ) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
        let opened = self
            .file
            .metadata()
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        let named = fs::symlink_metadata(&self.path)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
        validate_private_file(&opened, root_owner)?;
        validate_private_file(&named, root_owner)?;
        if !self.identity.matches(&opened)
            || !self.identity.matches(&named)
            || !same_file(&opened, &named)
        {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
        }
        Ok(())
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for PrivateTemporaryFile {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_file()
            && !metadata.file_type().is_symlink()
            && self.identity.matches(&metadata)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}
#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct PrivateFileIdentity {
    device: u64,
    inode: u64,
}
#[cfg(unix)]
impl PrivateFileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
    fn matches(self, metadata: &fs::Metadata) -> bool {
        self.device == metadata.dev() && self.inode == metadata.ino()
    }
}
#[cfg(not(unix))]
#[derive(Clone, Copy, PartialEq, Eq)]
struct PrivateFileIdentity;
#[cfg(not(unix))]
impl PrivateFileIdentity {
    fn from_metadata(_metadata: &fs::Metadata) -> Self {
        Self
    }
    fn matches(self, _metadata: &fs::Metadata) -> bool {
        true
    }
}
#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(unix)]
fn same_file_version(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    same_file(left, right)
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.nlink() == right.nlink()
}
#[cfg(not(unix))]
fn same_file(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}
#[cfg(not(unix))]
fn same_file_version(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}
#[cfg(unix)]
fn metadata_owner(metadata: &fs::Metadata) -> u32 {
    metadata.uid()
}
#[cfg(not(unix))]
fn metadata_owner(_metadata: &fs::Metadata) -> u32 {
    0
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::symlink,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    #[derive(Clone)]
    struct TestClock {
        current: Arc<AtomicU64>,
    }
    impl TestClock {
        fn new(current: u64) -> (Self, Arc<AtomicU64>) {
            let current = Arc::new(AtomicU64::new(current));
            (
                Self {
                    current: Arc::clone(&current),
                },
                current,
            )
        }
    }
    fn private_tempdir() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("private state root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("set private state-root permissions");
        root
    }
    impl MusubiPublicationServiceClockV1 for TestClock {
        fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
            Ok(self.current.load(Ordering::SeqCst))
        }
    }
    struct FailingClock;
    impl MusubiPublicationServiceClockV1 for FailingClock {
        fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        }
    }
    struct SubstitutingClock {
        current: u64,
        calls_before_substitution: usize,
        state_path: PathBuf,
    }
    impl MusubiPublicationServiceClockV1 for SubstitutingClock {
        fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
            if self.calls_before_substitution == 0 {
                let displaced = self.state_path.with_extension("sampled-prior");
                fs::rename(&self.state_path, &displaced).expect("displace state while sampling");
                fs::copy(&displaced, &self.state_path).expect("substitute state while sampling");
                fs::set_permissions(&self.state_path, fs::Permissions::from_mode(0o600))
                    .expect("private substituted state");
            } else {
                self.calls_before_substitution -= 1;
            }
            Ok(self.current)
        }
    }
    #[test]
    fn open_error_codes_are_stable_and_path_free() {
        let cases = [
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::UnsupportedPlatform,
                "MUSUBI_PUBLICATION_CLOCK_UNSUPPORTED_PLATFORM",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot,
                "MUSUBI_PUBLICATION_CLOCK_UNSAFE_ROOT",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::Locked,
                "MUSUBI_PUBLICATION_CLOCK_LOCKED",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized,
                "MUSUBI_PUBLICATION_CLOCK_UNINITIALIZED",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized,
                "MUSUBI_PUBLICATION_CLOCK_ALREADY_INITIALIZED",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState,
                "MUSUBI_PUBLICATION_CLOCK_INVALID_STATE",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::SourceUnavailable,
                "MUSUBI_PUBLICATION_CLOCK_SOURCE_UNAVAILABLE",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback,
                "MUSUBI_PUBLICATION_CLOCK_ROLLBACK",
            ),
            (
                DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable,
                "MUSUBI_PUBLICATION_CLOCK_STORAGE_UNAVAILABLE",
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.as_str(), code);
            assert_eq!(error.to_string(), code);
        }
    }
    #[test]
    fn ordinary_open_never_initializes_missing_or_deleted_state() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("ordinary open must not initialize"),
            DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
        );
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("explicit initialization");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect_err("reinitialization rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized
        );
        drop(clock);
        fs::remove_file(root.path().join(CLOCK_STATE_FILE)).expect("delete durable floor");
        let (source, _) = TestClock::new(1);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("deleted floor fails closed"),
            DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
        );
        fs::remove_file(root.path().join(CLOCK_LOCK_FILE)).expect("delete owner marker");
        let (source, _) = TestClock::new(1);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("fully deleted state still fails closed"),
            DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
        );
    }
    #[test]
    fn failed_or_zero_initial_sample_leaves_the_root_uninitialized() {
        let unavailable_root = private_tempdir();
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(
                unavailable_root.path(),
                Box::new(FailingClock),
            )
            .expect_err("unavailable source rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::SourceUnavailable
        );
        assert_eq!(
            fs::read_dir(unavailable_root.path())
                .expect("read root")
                .count(),
            0
        );
        let zero_root = private_tempdir();
        let (source, _) = TestClock::new(0);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(zero_root.path(), Box::new(source))
                .expect_err("zero source rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::SourceUnavailable
        );
        assert_eq!(
            fs::read_dir(zero_root.path()).expect("read root").count(),
            0
        );
    }
    #[test]
    fn floor_is_durable_and_restart_rollback_fails_closed() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        assert_eq!(clock.durable_floor_ms(), 100);
        current.store(200, Ordering::SeqCst);
        assert_eq!(clock.current_time_ms(), Ok(200));
        assert_eq!(clock.durable_floor_ms(), 200);
        current.store(199, Ordering::SeqCst);
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
        assert_eq!(clock.durable_floor_ms(), 200);
        current.store(201, Ordering::SeqCst);
        assert_eq!(clock.current_time_ms(), Ok(201));
        drop(clock);
        current.store(200, Ordering::SeqCst);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(
                root.path(),
                Box::new(TestClock {
                    current: Arc::clone(&current),
                }),
            )
            .expect_err("rollback rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback
        );
        current.store(201, Ordering::SeqCst);
        let reopened = DurableMusubiPublicationServiceClockV1::open(
            root.path(),
            Box::new(TestClock {
                current: Arc::clone(&current),
            }),
        )
        .expect("equal floor accepted");
        assert_eq!(reopened.durable_floor_ms(), 201);
        drop(reopened);
        current.store(250, Ordering::SeqCst);
        let advanced = DurableMusubiPublicationServiceClockV1::open(
            root.path(),
            Box::new(TestClock {
                current: Arc::clone(&current),
            }),
        )
        .expect("startup advances the durable floor");
        assert_eq!(advanced.durable_floor_ms(), 250);
        drop(advanced);
        current.store(249, Ordering::SeqCst);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(
                root.path(),
                Box::new(TestClock { current }),
            )
            .expect_err("advanced floor survives restart"),
            DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback
        );
    }
    #[test]
    fn exclusive_lock_prevents_two_clock_writers() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let first =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("first writer");
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(
                root.path(),
                Box::new(TestClock { current }),
            )
            .expect_err("second writer rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::Locked
        );
        drop(first);
        let (source, _) = TestClock::new(100);
        let reopened = DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
            .expect("lock released after drop");
        assert_eq!(reopened.durable_floor_ms(), 100);
    }
    #[test]
    fn root_mode_rejects_special_permission_bits() {
        let root = private_tempdir();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o1700))
            .expect("set sticky private mode");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect_err("special mode bits rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
        );
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("restore tempdir mode");
    }
    #[test]
    fn persisted_corruption_and_unsafe_paths_are_rejected() {
        let corrupt_root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock = DurableMusubiPublicationServiceClockV1::initialize(
            corrupt_root.path(),
            Box::new(source),
        )
        .expect("initialize state before corruption");
        drop(clock);
        fs::write(corrupt_root.path().join(CLOCK_STATE_FILE), b"not norito")
            .expect("write corrupt state");
        fs::set_permissions(
            corrupt_root.path().join(CLOCK_STATE_FILE),
            fs::Permissions::from_mode(0o600),
        )
        .expect("private corrupt state");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(corrupt_root.path(), Box::new(source),)
                .expect_err("corrupt state rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
        let public_root = private_tempdir();
        fs::set_permissions(public_root.path(), fs::Permissions::from_mode(0o755))
            .expect("make root public");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(
                public_root.path(),
                Box::new(source)
            )
            .expect_err("public root rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
        );
        let target = private_tempdir();
        let parent = private_tempdir();
        let linked = parent.path().join("clock-root");
        symlink(target.path(), &linked).expect("root symlink");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::initialize(&linked, Box::new(source))
                .expect_err("linked root rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
        );
    }
    #[test]
    fn startup_reconciles_only_the_fixed_private_next_file() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable state");
        drop(clock);
        let next = root.path().join(CLOCK_NEXT_FILE);
        let staged = DurableClockEnvelopeV1::new(DurableClockStateV1 {
            domain: CLOCK_STATE_DOMAIN_V1,
            schema: CLOCK_STATE_SCHEMA_V1,
            revision: 2,
            floor_ms: 500,
        })
        .expect("staged envelope");
        fs::write(
            &next,
            norito::encode_canonical(&staged).expect("encode staged envelope"),
        )
        .expect("write interrupted state");
        fs::set_permissions(&next, fs::Permissions::from_mode(0o600))
            .expect("make interrupted state private");
        let (source, _) = TestClock::new(100);
        let clock = DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
            .expect("known next file reconciled");
        assert_eq!(clock.durable_floor_ms(), 100);
        assert!(!next.exists());
        drop(clock);
        fs::write(root.path().join("unexpected"), b"foreign state")
            .expect("write unexpected state");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("unexpected directory entry rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
        );
    }
    #[test]
    fn live_state_substitution_poisoning_is_sticky() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        let state = root.path().join(CLOCK_STATE_FILE);
        let displaced = root.path().join("displaced-state");
        fs::rename(&state, &displaced).expect("displace live state");
        current.store(101, Ordering::SeqCst);
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
        fs::rename(&displaced, &state).expect("restore displaced state");
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
    }
    #[test]
    fn sampling_revalidates_state_before_returning_time() {
        let startup_root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock = DurableMusubiPublicationServiceClockV1::initialize(
            startup_root.path(),
            Box::new(source),
        )
        .expect("initialize startup fixture");
        drop(clock);
        let state_path = startup_root.path().join(CLOCK_STATE_FILE);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(
                startup_root.path(),
                Box::new(SubstitutingClock {
                    current: 100,
                    calls_before_substitution: 0,
                    state_path,
                }),
            )
            .expect_err("startup substitution rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable
        );
        let live_root = private_tempdir();
        let state_path = live_root.path().join(CLOCK_STATE_FILE);
        let mut clock = DurableMusubiPublicationServiceClockV1::initialize(
            live_root.path(),
            Box::new(SubstitutingClock {
                current: 100,
                calls_before_substitution: 1,
                state_path,
            }),
        )
        .expect("initialize live fixture");
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
    }
    #[test]
    fn live_next_file_collision_poisoning_is_sticky() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        let next = root.path().join(CLOCK_NEXT_FILE);
        fs::write(&next, b"occupied").expect("occupy next path");
        fs::set_permissions(&next, fs::Permissions::from_mode(0o600)).expect("private next path");
        current.store(101, Ordering::SeqCst);
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
        fs::remove_file(next).expect("remove collision");
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
    }
    #[test]
    fn state_digest_and_single_link_invariants_are_enforced() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        drop(clock);
        let state_path = root.path().join(CLOCK_STATE_FILE);
        let bytes = fs::read(&state_path).expect("read state");
        let mut envelope: DurableClockEnvelopeV1 =
            norito::decode_canonical(&bytes).expect("decode state written by implementation");
        envelope.state.floor_ms = 101;
        let tampered = norito::encode_canonical(&envelope).expect("encode tampered state");
        fs::write(&state_path, tampered).expect("replace state bytes");
        fs::set_permissions(&state_path, fs::Permissions::from_mode(0o600))
            .expect("retain private state mode");
        let (source, _) = TestClock::new(101);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("digest mismatch rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
        let clean_root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(clean_root.path(), Box::new(source))
                .expect("open clean durable clock");
        drop(clock);
        let state_path = clean_root.path().join(CLOCK_STATE_FILE);
        let external_link = clean_root.path().with_extension("clock-hardlink");
        fs::hard_link(&state_path, &external_link).expect("create hostile state hard link");
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(clean_root.path(), Box::new(source))
                .expect_err("hard-linked state rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
        fs::remove_file(external_link).expect("remove hostile hard link");
    }
    #[test]
    fn nonempty_lifetime_lock_fails_closed() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        drop(clock);
        fs::write(root.path().join(CLOCK_LOCK_FILE), b"substituted lock state")
            .expect("mutate lock file");
        fs::set_permissions(
            root.path().join(CLOCK_LOCK_FILE),
            fs::Permissions::from_mode(0o600),
        )
        .expect("retain private lock mode");
        let (source, _) = TestClock::new(101);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .expect_err("nonempty lock rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
    }
    #[test]
    fn error_codes_are_stable_and_system_constructor_uses_the_same_state() {
        assert_eq!(
            DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback.as_str(),
            "MUSUBI_PUBLICATION_CLOCK_ROLLBACK"
        );
        let root = private_tempdir();
        let clock = DurableMusubiPublicationServiceClockV1::initialize_system(root.path())
            .expect("system clock state");
        assert!(clock.durable_floor_ms() > 0);
    }
}
