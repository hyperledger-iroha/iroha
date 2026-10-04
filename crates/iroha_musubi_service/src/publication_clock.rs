//! Crash-safe time-floor persistence for the private Musubi publication service.
#[cfg(test)]
pub mod wire_fixtures;
use super::{
    MusubiPublicationServiceBackendErrorV1, MusubiPublicationServiceClockV1,
    MusubiPublicationSystemClockV1,
};
use iroha_fs::{FileSnapshot, PrivateDirectory, PublishMode};
use std::{
    fmt,
    fs::{self, File},
    io,
    path::Path,
};
const CLOCK_STATE_FILE: &str = "clock-floor-v1.norito";
const CLOCK_LOCK_FILE: &str = "clock-floor-v1.lock";
const CLOCK_STATE_DOMAIN_V1: [u8; 32] = *b"musubi-pub-clock-floor-v1\0\0\0\0\0\0\0";
const CLOCK_STATE_SCHEMA_V1: u8 = 1;
const MAX_CLOCK_STATE_BYTES: usize = 4 * 1024;
/// Stable failure opening a durable private-publication clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableMusubiPublicationServiceClockOpenErrorV1 {
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
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha::musubi_runtime::publication_clock::DurableClockStateV1")]
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
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::Encode,
    norito::derive::Decode,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha::musubi_runtime::publication_clock::DurableClockEnvelopeV1")]
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
/// The caller supplies one existing dedicated owner-private directory. The canonical native
/// filesystem owner validates Unix mode/identity or Windows protected DACL/handle custody.
/// The clock holds an exclusive process lock, durably publishes a canonical Norito floor, and
/// returns a newly observed time only after that floor is durable. No key material is retained.
///
/// This is a crash-safe local wall-clock floor, not an external rollback seal. Native protocol
/// authorization, finality and revocation remain independently necessary.
pub struct DurableMusubiPublicationServiceClockV1 {
    source: Box<dyn MusubiPublicationServiceClockV1>,
    root: PrivateDirectory,
    lock_handle: File,
    lock_snapshot: FileSnapshot,
    state_snapshot: FileSnapshot,
    state: DurableClockStateV1,
    poisoned: bool,
}
#[derive(Clone, Copy)]
struct ClockStorageContext<'a> {
    root: &'a PrivateDirectory,
    lock_handle: &'a File,
    lock_snapshot: FileSnapshot,
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
        let root = PrivateDirectory::open_exact(root)
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot)?;
        let initialization_sample = if initialize {
            ensure_empty_initialization_root(&root)?;
            let sampled = sample_startup_source(source.as_mut())?;
            ensure_empty_initialization_root(&root)?;
            Some(sampled)
        } else {
            None
        };
        let (lock_handle, lock_snapshot) = open_and_lock(&root, initialize)?;
        let storage = ClockStorageContext {
            root: &root,
            lock_handle: &lock_handle,
            lock_snapshot,
        };
        // Committed state must authenticate before staging can be discarded. In particular,
        // an interrupted first initialization never becomes an ordinary-open initializer.
        let loaded = read_state(&root)?;
        if initialize && loaded.is_some() {
            return Err(DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized);
        }
        if !initialize {
            let (state, snapshot) = loaded
                .as_ref()
                .ok_or(DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized)?;
            validate_lock(storage)?;
            root.reconcile_atomic_staging(
                &[CLOCK_STATE_FILE, CLOCK_LOCK_FILE],
                1,
                MAX_CLOCK_STATE_BYTES,
            )
            .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot)?;
            validate_live_state(storage, *snapshot, state)?;
        }
        let sampled = match initialization_sample {
            Some(sampled) => sampled,
            None => sample_startup_source(source.as_mut())?,
        };
        let (state, state_snapshot) = match loaded {
            Some((state, _)) if sampled < state.floor_ms => {
                return Err(DurableMusubiPublicationServiceClockOpenErrorV1::ClockRollback);
            }
            Some((mut state, state_snapshot)) => {
                validate_live_state(storage, state_snapshot, &state)?;
                if sampled > state.floor_ms {
                    let previous_state = state.clone();
                    state.revision = state
                        .revision
                        .checked_add(1)
                        .ok_or(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
                    state.floor_ms = sampled;
                    let state_snapshot =
                        write_state(storage, Some(state_snapshot), Some(&previous_state), &state)?;
                    (state, state_snapshot)
                } else {
                    (state, state_snapshot)
                }
            }
            None => {
                debug_assert!(initialize);
                let state = DurableClockStateV1::new(sampled);
                let state_snapshot = write_state(storage, None, None, &state)?;
                (state, state_snapshot)
            }
        };
        state.validate()?;
        Ok(Self {
            source,
            root,
            lock_handle,
            lock_snapshot,
            state_snapshot,
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
            lock_handle: &self.lock_handle,
            lock_snapshot: self.lock_snapshot,
        }
    }
}
impl MusubiPublicationServiceClockV1 for DurableMusubiPublicationServiceClockV1 {
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
        if self.poisoned {
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        if validate_live_state(self.storage_context(), self.state_snapshot, &self.state).is_err() {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        }
        let sampled = self.source.current_time_ms()?;
        if validate_live_state(self.storage_context(), self.state_snapshot, &self.state).is_err() {
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
        let Ok(state_snapshot) = write_state(
            self.storage_context(),
            Some(self.state_snapshot),
            Some(&self.state),
            &next,
        ) else {
            self.poisoned = true;
            return Err(MusubiPublicationServiceBackendErrorV1::Retryable);
        };
        self.state_snapshot = state_snapshot;
        self.state = next;
        Ok(sampled)
    }
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
fn ensure_empty_initialization_root(
    root: &PrivateDirectory,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    if !root
        .entries(1)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized)?
        .is_empty()
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::AlreadyInitialized);
    }
    Ok(())
}
fn open_and_lock(
    root: &PrivateDirectory,
    initialize: bool,
) -> Result<(File, FileSnapshot), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let file = if initialize {
        root.create_lock(CLOCK_LOCK_FILE)
    } else {
        root.open_existing_lock(CLOCK_LOCK_FILE)
    }
    .map_err(|error| {
        if !initialize && error.kind() == io::ErrorKind::NotFound {
            DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
        } else {
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        }
    })?;
    let snapshot = FileSnapshot::private_journal(&file)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    if file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?
        .len()
        != 0
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => DurableMusubiPublicationServiceClockOpenErrorV1::Locked,
        fs::TryLockError::Error(_) => {
            DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable
        }
    })?;
    validate_lock(ClockStorageContext {
        root,
        lock_handle: &file,
        lock_snapshot: snapshot,
    })?;
    Ok((file, snapshot))
}
fn validate_lock(
    storage: ClockStorageContext<'_>,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let validate = || -> io::Result<()> {
        storage.root.revalidate()?;
        let named = storage.root.open_read(CLOCK_LOCK_FILE)?;
        if FileSnapshot::private_journal(storage.lock_handle)? != storage.lock_snapshot
            || FileSnapshot::private_journal(&named)? != storage.lock_snapshot
            || named.metadata()?.len() != 0
        {
            return Err(io::Error::other("changed clock lock"));
        }
        Ok(())
    };
    validate().map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)
}
fn validate_inventory(
    root: &PrivateDirectory,
    has_state: bool,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    let names = root
        .entries(2)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    if names.len() != if has_state { 2 } else { 1 }
        || !names.iter().any(|name| name == CLOCK_LOCK_FILE)
        || (has_state && !names.iter().any(|name| name == CLOCK_STATE_FILE))
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}
fn read_state(
    root: &PrivateDirectory,
) -> Result<
    Option<(DurableClockStateV1, FileSnapshot)>,
    DurableMusubiPublicationServiceClockOpenErrorV1,
> {
    let file = match root.open_read(CLOCK_STATE_FILE) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState),
    };
    let snapshot = FileSnapshot::private_journal(&file)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    let length = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?
        .len();
    if length == 0 || length > MAX_CLOCK_STATE_BYTES as u64 {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let bytes = root
        .read(CLOCK_STATE_FILE, MAX_CLOCK_STATE_BYTES)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    let named = root
        .open_read(CLOCK_STATE_FILE)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    if FileSnapshot::private_journal(&file).ok() != Some(snapshot)
        || FileSnapshot::private_journal(&named).ok() != Some(snapshot)
        || bytes.len() as u64 != length
    {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    let envelope: DurableClockEnvelopeV1 = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(64, MAX_CLOCK_STATE_BYTES, 128, 64 * 1024, 16),
    )
    .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    envelope.validate()?;
    root.revalidate()
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    Ok(Some((envelope.state, snapshot)))
}
fn validate_live_state(
    storage: ClockStorageContext<'_>,
    snapshot: FileSnapshot,
    state: &DurableClockStateV1,
) -> Result<(), DurableMusubiPublicationServiceClockOpenErrorV1> {
    validate_lock(storage)?;
    validate_inventory(storage.root, true)?;
    if read_state(storage.root).ok().flatten().as_ref() != Some(&(state.clone(), snapshot)) {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    validate_lock(storage)
}
fn write_state(
    storage: ClockStorageContext<'_>,
    expected_snapshot: Option<FileSnapshot>,
    expected_state: Option<&DurableClockStateV1>,
    state: &DurableClockStateV1,
) -> Result<FileSnapshot, DurableMusubiPublicationServiceClockOpenErrorV1> {
    state.validate()?;
    let envelope = DurableClockEnvelopeV1::new(state.clone())?;
    let bytes = norito::encode_canonical(&envelope)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState)?;
    if bytes.len() > MAX_CLOCK_STATE_BYTES {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState);
    }
    validate_lock(storage)?;
    let mode = match (expected_snapshot, expected_state) {
        (Some(snapshot), Some(previous)) => {
            validate_live_state(storage, snapshot, previous)?;
            PublishMode::Replace
        }
        (None, None) => {
            validate_inventory(storage.root, false)?;
            if read_state(storage.root)?.is_some() {
                return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
            }
            PublishMode::CreateNew
        }
        _ => return Err(DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState),
    };
    // The original lock serializes writers. Canonical atomic publication retains the native
    // staged object through rename and directory sync. Any ambiguous error poisons the caller.
    storage
        .root
        .write_atomic(CLOCK_STATE_FILE, &bytes, mode)
        .map_err(|_| DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    validate_lock(storage)?;
    validate_inventory(storage.root, true)?;
    let (persisted, snapshot) = read_state(storage.root)?
        .ok_or(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable)?;
    if &persisted != state {
        return Err(DurableMusubiPublicationServiceClockOpenErrorV1::StorageUnavailable);
    }
    // Canonical decoding already rejects alternative encodings of the exact envelope.
    validate_live_state(storage, snapshot, state)?;
    Ok(snapshot)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    use std::{
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };
    const RETIRED_NEXT_FILE: &str = "clock-floor-v1.next";
    const INTERRUPTED_STAGE: &str = ".iroha-fs-1-0.tmp";
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
    struct PrivateTestRoot {
        private: PrivateDirectory,
        _temporary: tempfile::TempDir,
    }
    impl PrivateTestRoot {
        fn path(&self) -> &Path {
            self.private.path()
        }
    }
    fn private_tempdir() -> PrivateTestRoot {
        let temporary = tempfile::tempdir().expect("temporary workspace");
        let private = PrivateDirectory::open_or_create(temporary.path().join("private"))
            .expect("native owner-private state root");
        PrivateTestRoot {
            private,
            _temporary: temporary,
        }
    }
    #[cfg(unix)]
    #[test]
    fn clock_refuses_replaced_directory_without_mutating_either_namespace() {
        let workspace = private_tempdir();
        let configured = workspace.path().join("clock");
        let displaced = workspace.path().join("displaced-clock");
        let directory = PrivateDirectory::open_or_create(&configured).unwrap();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(&configured, Box::new(source))
                .unwrap();
        let original = directory
            .read(CLOCK_STATE_FILE, MAX_CLOCK_STATE_BYTES)
            .unwrap();
        let lock = directory.read(CLOCK_LOCK_FILE, 1).unwrap();
        fs::rename(&configured, &displaced).unwrap();
        let replacement = PrivateDirectory::open_or_create(&configured).unwrap();
        replacement
            .write_atomic("replacement-only", b"replacement", PublishMode::CreateNew)
            .unwrap();
        current.store(101, Ordering::SeqCst);
        assert_eq!(
            clock.current_time_ms(),
            Err(MusubiPublicationServiceBackendErrorV1::Retryable)
        );
        assert_eq!(
            fs::read(displaced.join(CLOCK_STATE_FILE)).unwrap(),
            original.as_slice()
        );
        assert_eq!(
            fs::read(displaced.join(CLOCK_LOCK_FILE)).unwrap(),
            lock.as_slice()
        );
        assert_eq!(fs::read_dir(&displaced).unwrap().count(), 2);
        assert_eq!(
            replacement.entries(2).unwrap(),
            [std::ffi::OsString::from("replacement-only")]
        );
        assert_eq!(
            replacement.read("replacement-only", 32).unwrap().as_slice(),
            b"replacement"
        );
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
                let bytes = fs::read(&displaced).expect("original state bytes");
                PrivateDirectory::open(self.state_path.parent().unwrap())
                    .unwrap()
                    .write_atomic(CLOCK_STATE_FILE, &bytes, PublishMode::CreateNew)
                    .expect("substitute private state while sampling");
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
    #[cfg(unix)]
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
        let (source, _) = TestClock::new(100);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(corrupt_root.path(), Box::new(source),)
                .expect_err("corrupt state rejected"),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
    }
    #[cfg(unix)]
    #[test]
    fn public_and_linked_roots_are_rejected() {
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
    fn startup_reconciles_only_canonical_private_atomic_staging() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        let clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable state");
        drop(clock);
        let next = root.path().join(INTERRUPTED_STAGE);
        let staged = DurableClockEnvelopeV1::new(DurableClockStateV1 {
            domain: CLOCK_STATE_DOMAIN_V1,
            schema: CLOCK_STATE_SCHEMA_V1,
            revision: 2,
            floor_ms: 500,
        })
        .expect("staged envelope");
        root.private
            .write_atomic(
                INTERRUPTED_STAGE,
                &norito::encode_canonical(&staged).expect("encode staged envelope"),
                PublishMode::CreateNew,
            )
            .expect("write private interrupted state");
        let (source, _) = TestClock::new(100);
        let clock = DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
            .expect("canonical staging file reconciled");
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
    fn live_staging_collision_poisoning_is_sticky() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .expect("initialize durable clock");
        let next = root.path().join(INTERRUPTED_STAGE);
        root.private
            .write_atomic(INTERRUPTED_STAGE, b"occupied", PublishMode::CreateNew)
            .expect("occupy canonical staging path");
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
    #[test]
    fn missing_or_corrupt_floor_never_discards_unpublished_staging() {
        for missing in [true, false] {
            let root = private_tempdir();
            let (source, _) = TestClock::new(100);
            drop(
                DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                    .unwrap(),
            );
            root.private
                .write_atomic(INTERRUPTED_STAGE, b"unpublished", PublishMode::CreateNew)
                .unwrap();
            if missing {
                fs::remove_file(root.path().join(CLOCK_STATE_FILE)).unwrap();
            } else {
                fs::write(root.path().join(CLOCK_STATE_FILE), b"corrupt").unwrap();
            }
            let names = root.private.entries(3).unwrap();
            let (source, _) = TestClock::new(101);
            let error = DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .unwrap_err();
            assert_eq!(
                error,
                if missing {
                    DurableMusubiPublicationServiceClockOpenErrorV1::Uninitialized
                } else {
                    DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
                }
            );
            assert_eq!(root.private.entries(3).unwrap(), names);
            assert_eq!(
                root.private.read(INTERRUPTED_STAGE, 16).unwrap().as_slice(),
                b"unpublished"
            );
        }
    }
    #[test]
    fn retired_fixed_next_file_is_unknown_and_never_removed() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        drop(
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .unwrap(),
        );
        root.private
            .write_atomic(RETIRED_NEXT_FILE, b"retired", PublishMode::CreateNew)
            .unwrap();
        let before = root
            .private
            .read(CLOCK_STATE_FILE, MAX_CLOCK_STATE_BYTES)
            .unwrap();
        let (source, _) = TestClock::new(101);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .unwrap_err(),
            DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
        );
        assert_eq!(
            root.private.read(RETIRED_NEXT_FILE, 7).unwrap().as_slice(),
            b"retired"
        );
        assert_eq!(
            root.private
                .read(CLOCK_STATE_FILE, MAX_CLOCK_STATE_BYTES)
                .unwrap()
                .as_slice(),
            before.as_slice()
        );
    }
    #[cfg(windows)]
    #[test]
    fn windows_retained_directory_and_lock_deny_replacement() {
        let root = private_tempdir();
        let (source, current) = TestClock::new(100);
        let mut clock =
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .unwrap();
        assert!(fs::rename(root.path(), root._temporary.path().join("displaced")).is_err());
        assert!(
            fs::rename(
                root.path().join(CLOCK_LOCK_FILE),
                root.path().join("other-lock")
            )
            .is_err()
        );
        assert!(fs::remove_file(root.path().join(CLOCK_LOCK_FILE)).is_err());
        current.store(101, Ordering::SeqCst);
        assert_eq!(clock.current_time_ms(), Ok(101));
    }
    #[cfg(windows)]
    #[test]
    fn windows_unprotected_state_and_reparse_roots_refuse_native_custody() {
        let root = private_tempdir();
        let (source, _) = TestClock::new(100);
        drop(
            DurableMusubiPublicationServiceClockV1::initialize(root.path(), Box::new(source))
                .unwrap(),
        );
        let bytes = root
            .private
            .read(CLOCK_STATE_FILE, MAX_CLOCK_STATE_BYTES)
            .unwrap();
        let inherited = root._temporary.path().join("inherited-state");
        fs::write(&inherited, bytes.as_slice()).unwrap();
        let unprotected = File::open(&inherited).unwrap();
        assert!(
            FileSnapshot::private_journal(&unprotected).is_err(),
            "fixture has actual unprotected/inherited custody"
        );
        drop(unprotected);
        fs::remove_file(root.path().join(CLOCK_STATE_FILE)).unwrap();
        fs::rename(&inherited, root.path().join(CLOCK_STATE_FILE)).unwrap();
        let (source, _) = TestClock::new(101);
        assert_eq!(
            DurableMusubiPublicationServiceClockV1::open(root.path(), Box::new(source))
                .unwrap_err(),
            DurableMusubiPublicationServiceClockOpenErrorV1::InvalidState
        );
        let link = root._temporary.path().join("linked-root");
        match std::os::windows::fs::symlink_dir(root.path(), &link) {
            Ok(()) => {
                let (source, _) = TestClock::new(101);
                assert_eq!(
                    DurableMusubiPublicationServiceClockV1::open(&link, Box::new(source))
                        .unwrap_err(),
                    DurableMusubiPublicationServiceClockOpenErrorV1::UnsafeRoot
                );
                fs::remove_dir(link).unwrap();
            }
            Err(error) => assert_eq!(
                error.kind(),
                io::ErrorKind::PermissionDenied,
                "Windows may deny symlink creation when developer mode/privilege is unavailable"
            ),
        }
    }
}
