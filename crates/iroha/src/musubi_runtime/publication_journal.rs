//! Crash-safe replay and idempotency persistence for the private Musubi publication service.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

#[cfg(unix)]
use super::publication_filesystem_owner_probe;
use super::{
    InMemoryMusubiPublicationServiceJournalV1, InMemoryPublicationResultV1,
    MAX_CONTROL_RESPONSE_BYTES, MUSUBI_MAX_ARCHIVE_LOCATIONS_V1, MUSUBI_MAX_LOCATION_PROVIDERS_V1,
    MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1, MusubiPublicationIdempotencyKeyV1,
    MusubiPublicationJournalAttemptV1, MusubiPublicationJournalBeginV1,
    MusubiPublicationOperationBindingV1, MusubiPublicationRuntimeOperationV1,
    MusubiPublicationServiceJournalBindingV1, MusubiPublicationServiceJournalErrorV1,
    MusubiPublicationServiceJournalV1, valid_storage_generation_target,
};

#[cfg(unix)]
use crate::musubi_archive_fetch::{
    secure_directory_open_flags, secure_no_follow_nonblocking_flags,
};

const JOURNAL_STATE_FILE: &str = "publication-journal-v1.norito";
const JOURNAL_LOCK_FILE: &str = "publication-journal-v1.lock";
const JOURNAL_NEXT_FILE: &str = "publication-journal-v1.next";
const JOURNAL_STATE_DOMAIN_V1: [u8; 32] = *b"musubi-pub-journal-state-v1\0\0\0\0\0";
const JOURNAL_STATE_SCHEMA_V1: u8 = 1;
const MAX_DURABLE_JOURNAL_OPERATIONS_V1: u32 = 1_000_000;
const MAX_DURABLE_JOURNAL_AUTHORIZATIONS_V1: u32 = 1_000_000;
const MAX_DURABLE_JOURNAL_TOTAL_RESPONSE_BYTES_V1: u64 = 64 * 1024 * 1024;
const MAX_DURABLE_JOURNAL_SNAPSHOT_BYTES_V1: u64 = 96 * 1024 * 1024;
const MIN_DURABLE_JOURNAL_SNAPSHOT_OVERHEAD_BYTES_V1: u64 = 256 * 1024;
const JOURNAL_DECODE_ALLOCATION_MULTIPLIER_V1: usize = 8;
const JOURNAL_DECODE_FIXED_ALLOCATION_BYTES_V1: usize = 64 * 1024;

/// Immutable capacities bound into one durable publication journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
#[allow(
    clippy::struct_field_names,
    reason = "the stable max_* names distinguish immutable upper bounds from live journal counts"
)]
pub struct DurableMusubiPublicationServiceJournalLimitsV1 {
    max_operations: u32,
    max_authorizations: u32,
    max_total_response_bytes: u64,
    max_snapshot_bytes: u64,
}

impl DurableMusubiPublicationServiceJournalLimitsV1 {
    /// Construct deployment-fixed lifetime capacities for one journal.
    ///
    /// The response budget and snapshot must accommodate one maximum-sized control response plus
    /// the bounded deployment, operation, result, authorization, and envelope records needed to
    /// reserve it. Resizing is deliberately not an ordinary-open operation because silently
    /// shrinking replay history would be unsafe.
    ///
    /// # Errors
    ///
    /// Returns [`DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits`] for zero,
    /// overflowing, internally inconsistent, or above-hard-cap values.
    pub fn new(
        max_operations: u32,
        max_authorizations: u32,
        max_total_response_bytes: u64,
        max_snapshot_bytes: u64,
    ) -> Result<Self, DurableMusubiPublicationServiceJournalOpenErrorV1> {
        let limits = Self {
            max_operations,
            max_authorizations,
            max_total_response_bytes,
            max_snapshot_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    /// Maximum immutable publication-operation bindings retained for this journal's lifetime.
    #[must_use]
    pub const fn max_operations(self) -> u32 {
        self.max_operations
    }

    /// Maximum unexpired consumed-authorization digests retained at once.
    #[must_use]
    pub const fn max_authorizations(self) -> u32 {
        self.max_authorizations
    }

    /// Maximum completed-response bytes plus in-flight terminal reservations.
    #[must_use]
    pub const fn max_total_response_bytes(self) -> u64 {
        self.max_total_response_bytes
    }

    /// Maximum complete canonical journal-envelope length.
    #[must_use]
    pub const fn max_snapshot_bytes(self) -> u64 {
        self.max_snapshot_bytes
    }

    fn validate(self) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
        let minimum_response =
            u64::try_from(MAX_CONTROL_RESPONSE_BYTES).expect("control-response bound fits u64");
        if self.max_operations == 0
            || self.max_operations > MAX_DURABLE_JOURNAL_OPERATIONS_V1
            || self.max_authorizations == 0
            || self.max_authorizations > MAX_DURABLE_JOURNAL_AUTHORIZATIONS_V1
            || self.max_total_response_bytes < minimum_response
            || self.max_total_response_bytes > MAX_DURABLE_JOURNAL_TOTAL_RESPONSE_BYTES_V1
            || self.max_snapshot_bytes
                < self
                    .max_total_response_bytes
                    .saturating_add(MIN_DURABLE_JOURNAL_SNAPSHOT_OVERHEAD_BYTES_V1)
            || self.max_snapshot_bytes > MAX_DURABLE_JOURNAL_SNAPSHOT_BYTES_V1
            || usize::try_from(self.max_operations).is_err()
            || usize::try_from(self.max_authorizations).is_err()
            || usize::try_from(self.max_total_response_bytes).is_err()
            || usize::try_from(self.max_snapshot_bytes).is_err()
            || self.max_results_usize().is_none()
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits);
        }
        Ok(())
    }

    fn max_operations_usize(self) -> usize {
        usize::try_from(self.max_operations).expect("validated operation bound fits usize")
    }

    fn max_authorizations_usize(self) -> usize {
        usize::try_from(self.max_authorizations).expect("validated authorization bound fits usize")
    }

    fn max_snapshot_usize(self) -> usize {
        usize::try_from(self.max_snapshot_bytes).expect("validated snapshot bound fits usize")
    }

    fn max_results_usize(self) -> Option<usize> {
        self.max_operations_usize()
            .checked_mul(results_per_operation())
    }
}

/// Stable failure opening or initializing a durable publication journal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableMusubiPublicationServiceJournalOpenErrorV1 {
    /// V1 cannot provide its required filesystem guarantees on this platform.
    UnsupportedPlatform,
    /// The configured directory is missing, shared, linked, or otherwise unsafe.
    UnsafeRoot,
    /// Another process already owns this journal.
    Locked,
    /// Ordinary startup found no previously initialized durable state.
    Uninitialized,
    /// One-time initialization was requested for a nonempty directory.
    AlreadyInitialized,
    /// Requested lifetime capacities are zero, inconsistent, or above V1 hard caps.
    InvalidLimits,
    /// Persisted capacities differ from the exact capacities supplied at startup.
    LimitsMismatch,
    /// Persisted deployment identity differs from the exact immutable service binding.
    ConfigurationMismatch,
    /// Persisted state is malformed, noncanonical, inconsistent, or corrupt.
    InvalidState,
    /// Private durable state could not be read, recovered, or atomically replaced.
    StorageUnavailable,
}

impl DurableMusubiPublicationServiceJournalOpenErrorV1 {
    /// Return the stable operator-facing error code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "MUSUBI_PUBLICATION_JOURNAL_UNSUPPORTED_PLATFORM",
            Self::UnsafeRoot => "MUSUBI_PUBLICATION_JOURNAL_UNSAFE_ROOT",
            Self::Locked => "MUSUBI_PUBLICATION_JOURNAL_LOCKED",
            Self::Uninitialized => "MUSUBI_PUBLICATION_JOURNAL_UNINITIALIZED",
            Self::AlreadyInitialized => "MUSUBI_PUBLICATION_JOURNAL_ALREADY_INITIALIZED",
            Self::InvalidLimits => "MUSUBI_PUBLICATION_JOURNAL_INVALID_LIMITS",
            Self::LimitsMismatch => "MUSUBI_PUBLICATION_JOURNAL_LIMITS_MISMATCH",
            Self::ConfigurationMismatch => "MUSUBI_PUBLICATION_JOURNAL_CONFIGURATION_MISMATCH",
            Self::InvalidState => "MUSUBI_PUBLICATION_JOURNAL_INVALID_STATE",
            Self::StorageUnavailable => "MUSUBI_PUBLICATION_JOURNAL_STORAGE_UNAVAILABLE",
        }
    }
}

impl fmt::Display for DurableMusubiPublicationServiceJournalOpenErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::error::Error for DurableMusubiPublicationServiceJournalOpenErrorV1 {}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurablePublicationOperationRecordV1 {
    operation_id: [u8; 32],
    binding: MusubiPublicationOperationBindingV1,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
enum DurablePublicationResultStateV1 {
    #[codec(index = 0)]
    Pending {
        request_digest: [u8; 32],
        response_reservation: u64,
    },
    #[codec(index = 1)]
    Aborted { request_digest: [u8; 32] },
    #[codec(index = 2)]
    Refreshing {
        request_digest: [u8; 32],
        previous_response: Vec<u8>,
        response_reservation: u64,
    },
    #[codec(index = 3)]
    Complete {
        request_digest: [u8; 32],
        response: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurablePublicationResultRecordV1 {
    key: MusubiPublicationIdempotencyKeyV1,
    state: DurablePublicationResultStateV1,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurablePublicationAuthorizationRecordV1 {
    authorization_digest: [u8; 32],
    expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurablePublicationJournalStateV1 {
    domain: [u8; 32],
    schema: u8,
    revision: u64,
    deployment: MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
    operations: Vec<DurablePublicationOperationRecordV1>,
    results: Vec<DurablePublicationResultRecordV1>,
    authorizations: Vec<DurablePublicationAuthorizationRecordV1>,
    total_response_bytes: u64,
    reserved_response_bytes: u64,
}

impl DurablePublicationJournalStateV1 {
    fn digest(&self) -> Result<[u8; 32], CandidateStateErrorV1> {
        let encoded = norito::encode_canonical(self).map_err(|_| CandidateStateErrorV1::Invalid)?;
        let mut hasher =
            blake3::Hasher::new_derive_key("iroha:musubi:publication-journal-state:v1");
        hasher.update(&encoded);
        Ok(*hasher.finalize().as_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct DurablePublicationJournalEnvelopeV1 {
    state: DurablePublicationJournalStateV1,
    state_digest: [u8; 32],
}

impl DurablePublicationJournalEnvelopeV1 {
    fn new(state: DurablePublicationJournalStateV1) -> Result<Self, CandidateStateErrorV1> {
        let state_digest = state.digest()?;
        Ok(Self {
            state,
            state_digest,
        })
    }

    fn validate_digest(&self) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
        if self
            .state
            .digest()
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?
            != self.state_digest
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CandidateStateErrorV1 {
    Capacity,
    Invalid,
}

/// Restart-persistent bounded journal for one exact private publication-service deployment.
///
/// V1 uses a dedicated Unix `0700` directory, holds one exclusive owner lock for its lifetime,
/// and commits a complete canonical Norito snapshot through a private fixed temporary file,
/// atomic rename, and file/directory durability barriers. It contains no CAR body, authorization
/// bytes, credentials, URLs, tokens, or provider secrets.
// TODO: Bind each committed revision/digest to a deployment-sealed monotonic CAS and replace
// pathname child mutation with qualified directory-relative primitives before production rollout.
pub struct DurableMusubiPublicationServiceJournalV1 {
    binding: MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
    journal: InMemoryMusubiPublicationServiceJournalV1,
    revision: u64,
    root: PathBuf,
    root_identity: JournalFileIdentity,
    root_owner: u32,
    root_handle: File,
    lock_handle: File,
    lock_identity: JournalFileIdentity,
    state_version: PersistedJournalVersionV1,
    poisoned: bool,
}

impl fmt::Debug for DurableMusubiPublicationServiceJournalV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableMusubiPublicationServiceJournalV1")
            .field("revision", &self.revision)
            .field("operations", &self.journal.operation_bindings.len())
            .field("results", &self.journal.results.len())
            .field("authorizations", &self.journal.authorization_expiry.len())
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl DurableMusubiPublicationServiceJournalV1 {
    /// Explicitly initialize one empty private journal directory.
    ///
    /// Initialization is deliberately separate from [`Self::open`]. A crash after the lifetime
    /// lock is installed but before the first state snapshot leaves a fail-closed directory that
    /// requires audited operator recovery; ordinary startup never treats it as a first boot.
    ///
    /// # Errors
    ///
    /// Returns a stable path-free category when deployment identity, capacities, filesystem safety,
    /// exclusivity, or durable initialization cannot be established.
    pub fn initialize(
        root: &Path,
        binding: MusubiPublicationServiceJournalBindingV1,
        limits: DurableMusubiPublicationServiceJournalLimitsV1,
    ) -> Result<Self, DurableMusubiPublicationServiceJournalOpenErrorV1> {
        Self::open_inner(root, binding, limits, true)
    }

    /// Open an initialized journal and durably recover every interrupted transition.
    ///
    /// `Pending` records become retryable aborted tombstones and interrupted receipt refreshes
    /// restore their prior completed response before this function returns.
    ///
    /// # Errors
    ///
    /// Returns a stable path-free category when deployment identity/capacities differ, state is unsafe
    /// or invalid, another owner is live, or recovery cannot be committed durably.
    pub fn open(
        root: &Path,
        binding: MusubiPublicationServiceJournalBindingV1,
        limits: DurableMusubiPublicationServiceJournalLimitsV1,
    ) -> Result<Self, DurableMusubiPublicationServiceJournalOpenErrorV1> {
        Self::open_inner(root, binding, limits, false)
    }

    fn open_inner(
        root: &Path,
        binding: MusubiPublicationServiceJournalBindingV1,
        limits: DurableMusubiPublicationServiceJournalLimitsV1,
        initialize: bool,
    ) -> Result<Self, DurableMusubiPublicationServiceJournalOpenErrorV1> {
        if !cfg!(unix) {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsupportedPlatform);
        }
        binding.validate().map_err(|_| {
            DurableMusubiPublicationServiceJournalOpenErrorV1::ConfigurationMismatch
        })?;
        limits.validate()?;
        let max_operations = limits.max_operations_usize();
        let max_authorizations = limits.max_authorizations_usize();
        let empty_journal = InMemoryMusubiPublicationServiceJournalV1::new(
            binding.clone(),
            max_operations,
            max_authorizations,
        )
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits)?;
        let initial_bytes = if initialize {
            Some(
                encode_candidate(&empty_journal, &binding, limits, 1)
                    .map_err(candidate_open_error)?,
            )
        } else {
            None
        };

        let (root, root_handle, root_identity, root_owner) = open_private_root(root)?;
        if initialize {
            ensure_empty_initialization_root(&root)?;
        }
        let lock_mode = if initialize {
            JournalLockOpenMode::CreateNew
        } else {
            JournalLockOpenMode::Existing
        };
        let (lock_handle, lock_identity) = open_and_lock(&root, root_owner, lock_mode)?;
        let storage = JournalStorageContext {
            root: &root,
            root_handle: &root_handle,
            root_identity,
            root_owner,
            lock_handle: &lock_handle,
            lock_identity,
        };
        reconcile_directory(storage, limits.max_snapshot_usize())?;

        let state_path = root.join(JOURNAL_STATE_FILE);
        let loaded = read_journal_state(&state_path, root_owner, &binding, limits)?;
        if initialize && loaded.is_some() {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::AlreadyInitialized);
        }
        if !initialize && loaded.is_none() {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized);
        }

        let (journal, revision, state_version) = loaded.map_or_else(
            || {
                debug_assert!(initialize);
                let bytes = initial_bytes.expect("initial bytes exist for initialization");
                let state_version =
                    write_state(storage, None, &bytes, limits.max_snapshot_usize())?;
                Ok((empty_journal, 1, state_version))
            },
            |(mut journal, revision, state_version)| {
                if recover_interrupted_results(&mut journal) {
                    let revision = revision
                        .checked_add(1)
                        .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
                    let bytes = encode_candidate(&journal, &binding, limits, revision)
                        .map_err(candidate_open_error)?;
                    let state_version = write_state(
                        storage,
                        Some(state_version),
                        &bytes,
                        limits.max_snapshot_usize(),
                    )?;
                    Ok((journal, revision, state_version))
                } else {
                    validate_live_state(storage, state_version, limits.max_snapshot_usize())?;
                    Ok((journal, revision, state_version))
                }
            },
        )?;

        Ok(Self {
            binding,
            limits,
            journal,
            revision,
            root,
            root_identity,
            root_owner,
            root_handle,
            lock_handle,
            lock_identity,
            state_version,
            poisoned: false,
        })
    }

    /// Return the durably committed snapshot revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Return the deployment-fixed capacities persisted in this journal.
    #[must_use]
    pub const fn limits(&self) -> DurableMusubiPublicationServiceJournalLimitsV1 {
        self.limits
    }

    fn storage_context(&self) -> JournalStorageContext<'_> {
        JournalStorageContext {
            root: &self.root,
            root_handle: &self.root_handle,
            root_identity: self.root_identity,
            root_owner: self.root_owner,
            lock_handle: &self.lock_handle,
            lock_identity: self.lock_identity,
        }
    }

    fn transition<T>(
        &mut self,
        apply: impl FnOnce(
            &mut InMemoryMusubiPublicationServiceJournalV1,
        ) -> Result<T, MusubiPublicationServiceJournalErrorV1>,
    ) -> Result<T, MusubiPublicationServiceJournalErrorV1> {
        // TODO: Replace this qualified whole-snapshot request path with a bounded transition WAL,
        // small atomic head, and off-path checkpoint compaction before selecting deployment
        // limits whose measured peak memory or latency exceed the V1 service budget.
        self.ensure_live()?;
        let mut next = self.journal.clone();
        let value = match apply(&mut next) {
            Ok(value) => value,
            Err(error) => {
                self.ensure_live()?;
                return Err(error);
            }
        };
        if next == self.journal {
            self.ensure_live()?;
            return Ok(value);
        }
        let Some(revision) = self.revision.checked_add(1) else {
            self.poisoned = true;
            return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
        };
        let bytes = match encode_candidate(&next, &self.binding, self.limits, revision) {
            Ok(bytes) => bytes,
            Err(CandidateStateErrorV1::Capacity) => {
                self.ensure_live()?;
                return Err(MusubiPublicationServiceJournalErrorV1::Capacity);
            }
            Err(CandidateStateErrorV1::Invalid) => {
                self.poisoned = true;
                return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
            }
        };
        let Ok(state_version) = write_state(
            self.storage_context(),
            Some(self.state_version),
            &bytes,
            self.limits.max_snapshot_usize(),
        ) else {
            self.poisoned = true;
            return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
        };
        self.journal = next;
        self.revision = revision;
        self.state_version = state_version;
        Ok(value)
    }

    fn ensure_live(&mut self) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        if self.poisoned {
            return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
        }
        if validate_live_state(
            self.storage_context(),
            self.state_version,
            self.limits.max_snapshot_usize(),
        )
        .is_err()
        {
            self.poisoned = true;
            return Err(MusubiPublicationServiceJournalErrorV1::Unavailable);
        }
        Ok(())
    }
}

impl MusubiPublicationServiceJournalV1 for DurableMusubiPublicationServiceJournalV1 {
    fn deployment_binding(&self) -> &MusubiPublicationServiceJournalBindingV1 {
        &self.binding
    }

    fn begin(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        current_time_ms: u64,
    ) -> Result<MusubiPublicationJournalBeginV1, MusubiPublicationServiceJournalErrorV1> {
        self.transition(|journal| journal.begin(attempt, current_time_ms))
    }

    fn refresh_expired_seed_receipt(
        &mut self,
        attempt: &MusubiPublicationJournalAttemptV1,
        expected_response: &[u8],
        current_time_ms: u64,
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        self.transition(|journal| {
            journal.refresh_expired_seed_receipt(attempt, expected_response, current_time_ms)
        })
    }

    fn commit(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
        response: &[u8],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        self.transition(|journal| journal.commit(key, request_digest, response))
    }

    fn abort(
        &mut self,
        key: MusubiPublicationIdempotencyKeyV1,
        request_digest: [u8; 32],
    ) -> Result<(), MusubiPublicationServiceJournalErrorV1> {
        self.transition(|journal| journal.abort(key, request_digest))
    }
}

fn candidate_open_error(
    error: CandidateStateErrorV1,
) -> DurableMusubiPublicationServiceJournalOpenErrorV1 {
    match error {
        CandidateStateErrorV1::Capacity => {
            DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits
        }
        CandidateStateErrorV1::Invalid => {
            DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState
        }
    }
}

fn results_per_operation() -> usize {
    maximum_historical_readbacks_per_operation()
        .checked_add(1)
        .and_then(|with_ingress| {
            with_ingress.checked_add(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1)
        })
        .expect("Musubi result bound is a fixed small constant")
}

fn maximum_historical_readbacks_per_operation() -> usize {
    let bound = MUSUBI_MAX_ARCHIVE_LOCATIONS_V1
        .checked_mul(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
        .expect("Musubi readback history bound is a fixed small constant");
    debug_assert!(
        bound
            >= MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1
                .checked_mul(2)
                .expect("publication readback minimum is fixed")
    );
    bound
}

fn recover_interrupted_results(journal: &mut InMemoryMusubiPublicationServiceJournalV1) -> bool {
    let mut changed = false;
    for result in journal.results.values_mut() {
        match result {
            InMemoryPublicationResultV1::Pending(request_digest) => {
                *result = InMemoryPublicationResultV1::Aborted(*request_digest);
                changed = true;
            }
            InMemoryPublicationResultV1::Refreshing {
                request_digest,
                previous_response,
            } => {
                *result = InMemoryPublicationResultV1::Complete {
                    request_digest: *request_digest,
                    response: previous_response.clone(),
                };
                changed = true;
            }
            InMemoryPublicationResultV1::Aborted(_)
            | InMemoryPublicationResultV1::Complete { .. } => {}
        }
    }
    changed
}

fn encode_candidate(
    journal: &InMemoryMusubiPublicationServiceJournalV1,
    binding: &MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
    revision: u64,
) -> Result<Vec<u8>, CandidateStateErrorV1> {
    if revision == 0 {
        return Err(CandidateStateErrorV1::Invalid);
    }
    let state = state_from_journal(journal, binding, limits, revision)?;
    if state.reserved_response_bytes != 0 {
        let projected = terminal_projection(&state)?;
        let projected_bytes = encode_envelope(projected)?;
        if projected_bytes.len() > limits.max_snapshot_usize() {
            return Err(CandidateStateErrorV1::Capacity);
        }
    }
    let bytes = encode_envelope(state)?;
    if bytes.is_empty() || bytes.len() > limits.max_snapshot_usize() {
        return Err(CandidateStateErrorV1::Capacity);
    }
    Ok(bytes)
}

fn encode_envelope(
    state: DurablePublicationJournalStateV1,
) -> Result<Vec<u8>, CandidateStateErrorV1> {
    let envelope = DurablePublicationJournalEnvelopeV1::new(state)?;
    norito::encode_canonical(&envelope).map_err(|_| CandidateStateErrorV1::Invalid)
}

fn validate_candidate_journal(
    journal: &InMemoryMusubiPublicationServiceJournalV1,
    binding: &MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
) -> Result<(), CandidateStateErrorV1> {
    binding
        .validate()
        .map_err(|_| CandidateStateErrorV1::Invalid)?;
    limits
        .validate()
        .map_err(|_| CandidateStateErrorV1::Invalid)?;
    if journal.binding != *binding
        || journal.max_operations != limits.max_operations_usize()
        || journal.max_results
            != limits
                .max_results_usize()
                .ok_or(CandidateStateErrorV1::Invalid)?
        || journal.max_authorizations != limits.max_authorizations_usize()
        || journal.operation_bindings.len() > journal.max_operations
        || journal.results.len() > journal.max_results
        || journal.authorization_expiry.len() > journal.max_authorizations
        || journal.expiry_index.len() != journal.authorization_expiry.len()
    {
        return Err(CandidateStateErrorV1::Invalid);
    }

    for (operation_id, operation_binding) in &journal.operation_bindings {
        if !digest_is_nonzero(operation_id)
            || operation_binding.operation_id != *operation_id
            || operation_binding.network_id != binding.network_id
            || operation_binding.validate().is_err()
        {
            return Err(CandidateStateErrorV1::Invalid);
        }
    }

    let mut result_operations = BTreeSet::new();
    let mut per_operation = BTreeMap::<[u8; 32], (bool, usize, usize)>::new();
    for (key, result) in &journal.results {
        if !valid_result_key(*key)
            || !journal.operation_bindings.contains_key(&key.operation_id)
            || match result {
                InMemoryPublicationResultV1::Pending(request_digest)
                | InMemoryPublicationResultV1::Aborted(request_digest) => {
                    !digest_is_nonzero(request_digest)
                }
                InMemoryPublicationResultV1::Refreshing {
                    request_digest,
                    previous_response,
                } => !digest_is_nonzero(request_digest) || !valid_response(previous_response),
                InMemoryPublicationResultV1::Complete {
                    request_digest,
                    response,
                } => !digest_is_nonzero(request_digest) || !valid_response(response),
            }
        {
            return Err(CandidateStateErrorV1::Invalid);
        }
        result_operations.insert(key.operation_id);
        let counts = per_operation.entry(key.operation_id).or_default();
        match key.operation {
            MusubiPublicationRuntimeOperationV1::SeedIngress if !counts.0 => counts.0 = true,
            MusubiPublicationRuntimeOperationV1::StorageCoordination
                if counts.1 < MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 =>
            {
                counts.1 += 1;
            }
            MusubiPublicationRuntimeOperationV1::ProviderReadback
                if counts.2 < maximum_historical_readbacks_per_operation() =>
            {
                counts.2 += 1;
            }
            _ => return Err(CandidateStateErrorV1::Invalid),
        }
    }
    if result_operations.len() != journal.operation_bindings.len()
        || journal
            .operation_bindings
            .keys()
            .any(|operation_id| !result_operations.contains(operation_id))
    {
        return Err(CandidateStateErrorV1::Invalid);
    }

    if journal.authorization_expiry.iter().any(|(digest, expiry)| {
        !digest_is_nonzero(digest)
            || *expiry == 0
            || !journal.expiry_index.contains(&(*expiry, *digest))
    }) || journal.expiry_index.iter().any(|(expiry, digest)| {
        *expiry == 0 || journal.authorization_expiry.get(digest) != Some(expiry)
    }) {
        return Err(CandidateStateErrorV1::Invalid);
    }
    Ok(())
}

fn state_from_journal(
    journal: &InMemoryMusubiPublicationServiceJournalV1,
    binding: &MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
    revision: u64,
) -> Result<DurablePublicationJournalStateV1, CandidateStateErrorV1> {
    validate_candidate_journal(journal, binding, limits)?;
    let operations = journal
        .operation_bindings
        .iter()
        .map(
            |(operation_id, binding)| DurablePublicationOperationRecordV1 {
                operation_id: *operation_id,
                binding: binding.clone(),
            },
        )
        .collect();
    let maximum_response =
        u64::try_from(MAX_CONTROL_RESPONSE_BYTES).expect("control-response bound fits u64");
    let mut total_response_bytes = 0_u64;
    let mut reserved_response_bytes = 0_u64;
    let mut results = Vec::with_capacity(journal.results.len());
    for (key, result) in &journal.results {
        let state = match result {
            InMemoryPublicationResultV1::Pending(request_digest) => {
                reserved_response_bytes = reserved_response_bytes
                    .checked_add(maximum_response)
                    .ok_or(CandidateStateErrorV1::Capacity)?;
                DurablePublicationResultStateV1::Pending {
                    request_digest: *request_digest,
                    response_reservation: maximum_response,
                }
            }
            InMemoryPublicationResultV1::Aborted(request_digest) => {
                DurablePublicationResultStateV1::Aborted {
                    request_digest: *request_digest,
                }
            }
            InMemoryPublicationResultV1::Refreshing {
                request_digest,
                previous_response,
            } => {
                let response_length = u64::try_from(previous_response.len())
                    .map_err(|_| CandidateStateErrorV1::Invalid)?;
                total_response_bytes = total_response_bytes
                    .checked_add(response_length)
                    .ok_or(CandidateStateErrorV1::Capacity)?;
                let reservation = maximum_response
                    .checked_sub(response_length)
                    .ok_or(CandidateStateErrorV1::Invalid)?;
                reserved_response_bytes = reserved_response_bytes
                    .checked_add(reservation)
                    .ok_or(CandidateStateErrorV1::Capacity)?;
                DurablePublicationResultStateV1::Refreshing {
                    request_digest: *request_digest,
                    previous_response: previous_response.clone(),
                    response_reservation: reservation,
                }
            }
            InMemoryPublicationResultV1::Complete {
                request_digest,
                response,
            } => {
                total_response_bytes = total_response_bytes
                    .checked_add(
                        u64::try_from(response.len())
                            .map_err(|_| CandidateStateErrorV1::Invalid)?,
                    )
                    .ok_or(CandidateStateErrorV1::Capacity)?;
                DurablePublicationResultStateV1::Complete {
                    request_digest: *request_digest,
                    response: response.clone(),
                }
            }
        };
        results.push(DurablePublicationResultRecordV1 { key: *key, state });
    }
    if total_response_bytes
        .checked_add(reserved_response_bytes)
        .is_none_or(|total| total > limits.max_total_response_bytes)
    {
        return Err(CandidateStateErrorV1::Capacity);
    }
    let authorizations = journal
        .authorization_expiry
        .iter()
        .map(
            |(authorization_digest, expires_at_ms)| DurablePublicationAuthorizationRecordV1 {
                authorization_digest: *authorization_digest,
                expires_at_ms: *expires_at_ms,
            },
        )
        .collect();
    Ok(DurablePublicationJournalStateV1 {
        domain: JOURNAL_STATE_DOMAIN_V1,
        schema: JOURNAL_STATE_SCHEMA_V1,
        revision,
        deployment: binding.clone(),
        limits,
        operations,
        results,
        authorizations,
        total_response_bytes,
        reserved_response_bytes,
    })
}

fn terminal_projection(
    state: &DurablePublicationJournalStateV1,
) -> Result<DurablePublicationJournalStateV1, CandidateStateErrorV1> {
    let mut projected = state.clone();
    for record in &mut projected.results {
        match &record.state {
            DurablePublicationResultStateV1::Pending { request_digest, .. }
            | DurablePublicationResultStateV1::Refreshing { request_digest, .. } => {
                record.state = DurablePublicationResultStateV1::Complete {
                    request_digest: *request_digest,
                    response: vec![0_u8; MAX_CONTROL_RESPONSE_BYTES],
                };
            }
            DurablePublicationResultStateV1::Aborted { .. }
            | DurablePublicationResultStateV1::Complete { .. } => {}
        }
    }
    projected.total_response_bytes = projected
        .total_response_bytes
        .checked_add(projected.reserved_response_bytes)
        .ok_or(CandidateStateErrorV1::Capacity)?;
    projected.reserved_response_bytes = 0;
    Ok(projected)
}

#[allow(
    clippy::too_many_lines,
    reason = "durable snapshot admission keeps every ordering, capacity, replay, and accounting invariant in one validator"
)]
fn journal_from_state(
    state: &DurablePublicationJournalStateV1,
    expected_binding: &MusubiPublicationServiceJournalBindingV1,
    expected_limits: DurableMusubiPublicationServiceJournalLimitsV1,
) -> Result<
    InMemoryMusubiPublicationServiceJournalV1,
    DurableMusubiPublicationServiceJournalOpenErrorV1,
> {
    expected_binding
        .validate()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::ConfigurationMismatch)?;
    if state.domain != JOURNAL_STATE_DOMAIN_V1
        || state.schema != JOURNAL_STATE_SCHEMA_V1
        || state.revision == 0
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    if &state.deployment != expected_binding {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::ConfigurationMismatch);
    }
    if state.limits != expected_limits {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::LimitsMismatch);
    }
    expected_limits.validate()?;
    if state.operations.len() > expected_limits.max_operations_usize()
        || state.results.len()
            > expected_limits
                .max_results_usize()
                .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits)?
        || state.authorizations.len() > expected_limits.max_authorizations_usize()
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }

    let mut operation_bindings = BTreeMap::new();
    let mut previous_operation_id = None;
    for record in &state.operations {
        if record.operation_id.iter().all(|byte| *byte == 0)
            || previous_operation_id.is_some_and(|previous| previous >= record.operation_id)
            || record.operation_id != record.binding.operation_id
            || record.binding.network_id != expected_binding.network_id
            || record.binding.validate().is_err()
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
        }
        previous_operation_id = Some(record.operation_id);
        operation_bindings.insert(record.operation_id, record.binding.clone());
    }

    let mut results = BTreeMap::new();
    let mut previous_key = None;
    let mut per_operation = BTreeMap::<[u8; 32], (bool, usize, usize)>::new();
    let mut calculated_total_response_bytes = 0_u64;
    let mut calculated_reserved_response_bytes = 0_u64;
    let maximum_response =
        u64::try_from(MAX_CONTROL_RESPONSE_BYTES).expect("control-response bound fits u64");
    for record in &state.results {
        if previous_key.is_some_and(|previous| previous >= record.key)
            || !valid_result_key(record.key)
            || !operation_bindings.contains_key(&record.key.operation_id)
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
        }
        previous_key = Some(record.key);
        let counts = per_operation.entry(record.key.operation_id).or_default();
        match record.key.operation {
            MusubiPublicationRuntimeOperationV1::SeedIngress if !counts.0 => counts.0 = true,
            MusubiPublicationRuntimeOperationV1::StorageCoordination
                if counts.1 < MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 =>
            {
                counts.1 += 1;
            }
            MusubiPublicationRuntimeOperationV1::ProviderReadback
                if counts.2 < maximum_historical_readbacks_per_operation() =>
            {
                counts.2 += 1;
            }
            _ => {
                return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
            }
        }
        let result = match &record.state {
            DurablePublicationResultStateV1::Pending {
                request_digest,
                response_reservation,
            } if digest_is_nonzero(request_digest) && *response_reservation == maximum_response => {
                calculated_reserved_response_bytes = calculated_reserved_response_bytes
                    .checked_add(*response_reservation)
                    .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
                InMemoryPublicationResultV1::Pending(*request_digest)
            }
            DurablePublicationResultStateV1::Aborted { request_digest }
                if digest_is_nonzero(request_digest) =>
            {
                InMemoryPublicationResultV1::Aborted(*request_digest)
            }
            DurablePublicationResultStateV1::Refreshing {
                request_digest,
                previous_response,
                response_reservation,
            } if digest_is_nonzero(request_digest)
                && valid_response(previous_response)
                && *response_reservation
                    == maximum_response
                        .checked_sub(u64::try_from(previous_response.len()).unwrap_or(u64::MAX))
                        .unwrap_or(u64::MAX) =>
            {
                calculated_total_response_bytes = calculated_total_response_bytes
                    .checked_add(u64::try_from(previous_response.len()).unwrap_or(u64::MAX))
                    .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
                calculated_reserved_response_bytes = calculated_reserved_response_bytes
                    .checked_add(*response_reservation)
                    .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
                InMemoryPublicationResultV1::Refreshing {
                    request_digest: *request_digest,
                    previous_response: previous_response.clone(),
                }
            }
            DurablePublicationResultStateV1::Complete {
                request_digest,
                response,
            } if digest_is_nonzero(request_digest) && valid_response(response) => {
                calculated_total_response_bytes = calculated_total_response_bytes
                    .checked_add(u64::try_from(response.len()).unwrap_or(u64::MAX))
                    .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
                InMemoryPublicationResultV1::Complete {
                    request_digest: *request_digest,
                    response: response.clone(),
                }
            }
            _ => {
                return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
            }
        };
        results.insert(record.key, result);
    }
    if calculated_total_response_bytes != state.total_response_bytes
        || calculated_reserved_response_bytes != state.reserved_response_bytes
        || per_operation.len() != operation_bindings.len()
        || calculated_total_response_bytes
            .checked_add(calculated_reserved_response_bytes)
            .is_none_or(|total| total > expected_limits.max_total_response_bytes)
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }

    let mut authorization_expiry = BTreeMap::new();
    let mut expiry_index = BTreeSet::new();
    let mut previous_authorization = None;
    for record in &state.authorizations {
        if !digest_is_nonzero(&record.authorization_digest)
            || record.expires_at_ms == 0
            || previous_authorization
                .is_some_and(|previous| previous >= record.authorization_digest)
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
        }
        previous_authorization = Some(record.authorization_digest);
        authorization_expiry.insert(record.authorization_digest, record.expires_at_ms);
        expiry_index.insert((record.expires_at_ms, record.authorization_digest));
    }

    Ok(InMemoryMusubiPublicationServiceJournalV1 {
        binding: expected_binding.clone(),
        max_operations: expected_limits.max_operations_usize(),
        max_results: expected_limits
            .max_results_usize()
            .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits)?,
        max_authorizations: expected_limits.max_authorizations_usize(),
        operation_bindings,
        results,
        authorization_expiry,
        expiry_index,
    })
}

fn digest_is_nonzero(digest: &[u8; 32]) -> bool {
    digest.iter().any(|byte| *byte != 0)
}

fn valid_response(response: &[u8]) -> bool {
    !response.is_empty() && response.len() <= MAX_CONTROL_RESPONSE_BYTES
}

fn valid_result_key(key: MusubiPublicationIdempotencyKeyV1) -> bool {
    digest_is_nonzero(&key.operation_id)
        && match key.operation {
            MusubiPublicationRuntimeOperationV1::ProviderReadback => digest_is_nonzero(&key.target),
            MusubiPublicationRuntimeOperationV1::SeedIngress => {
                key.target.iter().all(|byte| *byte == 0)
            }
            MusubiPublicationRuntimeOperationV1::StorageCoordination => {
                valid_storage_generation_target(key.target)
            }
        }
}

#[derive(Clone, Copy)]
struct JournalStorageContext<'a> {
    root: &'a Path,
    root_handle: &'a File,
    root_identity: JournalFileIdentity,
    root_owner: u32,
    lock_handle: &'a File,
    lock_identity: JournalFileIdentity,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PersistedJournalVersionV1 {
    identity: JournalFileIdentity,
    length: u64,
    digest: [u8; 32],
}

fn read_journal_state(
    path: &Path,
    root_owner: u32,
    binding: &MusubiPublicationServiceJournalBindingV1,
    limits: DurableMusubiPublicationServiceJournalLimitsV1,
) -> Result<
    Option<(
        InMemoryMusubiPublicationServiceJournalV1,
        u64,
        PersistedJournalVersionV1,
    )>,
    DurableMusubiPublicationServiceJournalOpenErrorV1,
> {
    let Some(named_before) = optional_metadata(path)? else {
        return Ok(None);
    };
    validate_private_file(&named_before, root_owner)?;
    validate_state_length(named_before.len(), limits.max_snapshot_usize())?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_no_follow_nonblocking_flags());
    let mut file = options
        .open(path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened_before = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_before, root_owner)?;
    validate_state_length(opened_before.len(), limits.max_snapshot_usize())?;
    if !same_file_version(&named_before, &opened_before) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    let expected_length = usize::try_from(opened_before.len())
        .unwrap_or_else(|_| limits.max_snapshot_usize())
        .min(limits.max_snapshot_usize());
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_length)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    std::io::Read::by_ref(&mut file)
        .take(
            u64::try_from(limits.max_snapshot_usize()).expect("validated snapshot bound fits u64")
                + 1,
        )
        .read_to_end(&mut bytes)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened_after = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let named_after = fs::symlink_metadata(path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_after, root_owner)?;
    validate_private_file(&named_after, root_owner)?;
    if bytes.is_empty()
        || bytes.len() > limits.max_snapshot_usize()
        || u64::try_from(bytes.len()).ok() != Some(opened_before.len())
        || !same_file_version(&opened_before, &opened_after)
        || !same_file_version(&opened_after, &named_after)
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    let decode_limits = journal_decode_limits(bytes.len())?;
    let envelope: DurablePublicationJournalEnvelopeV1 =
        norito::decode_canonical_with_limits(&bytes, decode_limits)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
    envelope.validate_digest()?;
    let revision = envelope.state.revision;
    let journal = journal_from_state(&envelope.state, binding, limits)?;
    let canonical = encode_candidate(&journal, binding, limits, revision)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
    if canonical != bytes {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    Ok(Some((
        journal,
        revision,
        PersistedJournalVersionV1 {
            identity: JournalFileIdentity::from_metadata(&opened_after),
            length: opened_after.len(),
            digest: journal_file_digest(&bytes),
        },
    )))
}

fn journal_decode_limits(
    payload_bytes: usize,
) -> Result<norito::DecodeLimits, DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let max_total_allocated_bytes = payload_bytes
        .checked_mul(JOURNAL_DECODE_ALLOCATION_MULTIPLIER_V1)
        .and_then(|bytes| bytes.checked_add(JOURNAL_DECODE_FIXED_ALLOCATION_BYTES_V1))
        .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState)?;
    Ok(norito::DecodeLimits::new(
        payload_bytes,
        payload_bytes,
        payload_bytes,
        max_total_allocated_bytes,
        64,
    ))
}

fn journal_file_digest(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:publication-journal-file:v1");
    hasher.update(bytes);
    *hasher.finalize().as_bytes()
}

fn write_state(
    storage: JournalStorageContext<'_>,
    expected: Option<PersistedJournalVersionV1>,
    bytes: &[u8],
    maximum_bytes: usize,
) -> Result<PersistedJournalVersionV1, DurableMusubiPublicationServiceJournalOpenErrorV1> {
    if bytes.is_empty() || bytes.len() > maximum_bytes {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    let JournalStorageContext {
        root,
        root_handle,
        root_identity,
        root_owner,
        lock_handle,
        lock_identity,
    } = storage;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    let target = root.join(JOURNAL_STATE_FILE);
    validate_persisted_state(&target, expected, root_owner, maximum_bytes)?;
    let mut pending = PrivateJournalTemporaryFile::create(root, root_owner)?;
    pending
        .file
        .write_all(bytes)
        .and_then(|()| pending.file.flush())
        .and_then(|()| pending.file.sync_all())
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let pending_version = PersistedJournalVersionV1 {
        identity: pending.identity,
        length: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        digest: journal_file_digest(bytes),
    };
    pending.validate(root_owner)?;
    validate_exact_state_file(&pending.path, pending_version, root_owner, maximum_bytes)?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    validate_persisted_state(&target, expected, root_owner, maximum_bytes)?;
    validate_exact_state_file(&pending.path, pending_version, root_owner, maximum_bytes)?;
    fs::rename(&pending.path, &target)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    pending.disarm();
    validate_exact_state_file(&target, pending_version, root_owner, maximum_bytes)?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    root_handle
        .sync_all()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_root_identity(root, root_handle, root_identity, root_owner)?;
    validate_lock_identity(root, lock_handle, lock_identity, root_owner)?;
    validate_exact_state_file(&target, pending_version, root_owner, maximum_bytes)?;
    Ok(pending_version)
}

fn validate_live_state(
    storage: JournalStorageContext<'_>,
    expected: PersistedJournalVersionV1,
    maximum_bytes: usize,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    validate_root_identity(
        storage.root,
        storage.root_handle,
        storage.root_identity,
        storage.root_owner,
    )?;
    validate_lock_identity(
        storage.root,
        storage.lock_handle,
        storage.lock_identity,
        storage.root_owner,
    )?;
    validate_exact_state_file(
        &storage.root.join(JOURNAL_STATE_FILE),
        expected,
        storage.root_owner,
        maximum_bytes,
    )
}

fn validate_persisted_state(
    path: &Path,
    expected: Option<PersistedJournalVersionV1>,
    root_owner: u32,
    maximum_bytes: usize,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    expected.map_or_else(
        || match fs::symlink_metadata(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            _ => Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable),
        },
        |expected| validate_exact_state_file(path, expected, root_owner, maximum_bytes),
    )
}

fn validate_exact_state_file(
    path: &Path,
    expected: PersistedJournalVersionV1,
    root_owner: u32,
    maximum_bytes: usize,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let named_before = fs::symlink_metadata(path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&named_before, root_owner)?;
    validate_state_length(named_before.len(), maximum_bytes)?;
    if !expected.identity.matches(&named_before) || named_before.len() != expected.length {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_no_follow_nonblocking_flags());
    let mut file = options
        .open(path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened_before = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_before, root_owner)?;
    if !same_file_version(&named_before, &opened_before) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
    }
    let mut hasher = blake3::Hasher::new_derive_key("iroha:musubi:publication-journal-file:v1");
    let mut total = 0_usize;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read)
            .ok_or(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        if total > maximum_bytes {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
        }
        hasher.update(&buffer[..read]);
    }
    let opened_after = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let named_after = fs::symlink_metadata(path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened_after, root_owner)?;
    validate_private_file(&named_after, root_owner)?;
    if !same_file_version(&opened_before, &opened_after)
        || !same_file_version(&opened_after, &named_after)
        || u64::try_from(total).ok() != Some(expected.length)
        || *hasher.finalize().as_bytes() != expected.digest
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}

fn validate_state_length(
    length: u64,
    maximum_bytes: usize,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    if length == 0
        || usize::try_from(length)
            .ok()
            .is_none_or(|length| length > maximum_bytes)
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    Ok(())
}

fn open_private_root(
    root: &Path,
) -> Result<
    (PathBuf, File, JournalFileIdentity, u32),
    DurableMusubiPublicationServiceJournalOpenErrorV1,
> {
    let linked = fs::symlink_metadata(root)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot)?;
    validate_private_root(&linked)?;
    let canonical = fs::canonicalize(root)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot)?;
    let canonical_metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot)?;
    validate_private_root(&canonical_metadata)?;
    if !same_file(&linked, &canonical_metadata) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    let filesystem_owner = publication_filesystem_owner_probe(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    #[cfg(unix)]
    if metadata_owner(&linked) != filesystem_owner
        || metadata_owner(&canonical_metadata) != filesystem_owner
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(secure_directory_open_flags());
    let handle = options
        .open(&canonical)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened = handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_root(&opened)?;
    if !same_file(&canonical_metadata, &opened) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    if metadata_owner(&opened) != filesystem_owner {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    Ok((
        canonical,
        handle,
        JournalFileIdentity::from_metadata(&opened),
        metadata_owner(&opened),
    ))
}

#[derive(Clone, Copy)]
enum JournalLockOpenMode {
    Existing,
    CreateNew,
}

fn ensure_empty_initialization_root(
    root: &Path,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let mut entries = fs::read_dir(root)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    if entries
        .next()
        .transpose()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?
        .is_some()
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::AlreadyInitialized);
    }
    Ok(())
}

fn open_and_lock(
    root: &Path,
    root_owner: u32,
    mode: JournalLockOpenMode,
) -> Result<(File, JournalFileIdentity), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let path = root.join(JOURNAL_LOCK_FILE);
    let before = optional_metadata(&path)?;
    match (mode, before.is_some()) {
        (JournalLockOpenMode::Existing, false) => {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized);
        }
        (JournalLockOpenMode::CreateNew, true) => {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::AlreadyInitialized);
        }
        _ => {}
    }
    if let Some(metadata) = &before {
        validate_private_file(metadata, root_owner)?;
        if metadata.len() != 0 {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
        }
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).truncate(false);
    if matches!(mode, JournalLockOpenMode::CreateNew) {
        options.create_new(true);
    }
    #[cfg(unix)]
    options
        .mode(0o600)
        .custom_flags(secure_no_follow_nonblocking_flags());
    let file = options
        .open(&path)
        .map_err(|error| match (mode, error.kind()) {
            (JournalLockOpenMode::Existing, io::ErrorKind::NotFound) => {
                DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized
            }
            (JournalLockOpenMode::CreateNew, io::ErrorKind::AlreadyExists) => {
                DurableMusubiPublicationServiceJournalOpenErrorV1::AlreadyInitialized
            }
            _ => DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable,
        })?;
    if before.is_none() {
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    }
    let opened = file
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&opened, root_owner)?;
    if opened.len() != 0 {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    if before
        .as_ref()
        .is_some_and(|metadata| !same_file(metadata, &opened))
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    let named = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&named, root_owner)?;
    if named.len() != 0 || !same_file(&opened, &named) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => DurableMusubiPublicationServiceJournalOpenErrorV1::Locked,
        fs::TryLockError::Error(_) => {
            DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable
        }
    })?;
    let after = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&after, root_owner)?;
    if after.len() != 0 || !same_file(&opened, &after) {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    Ok((file, JournalFileIdentity::from_metadata(&opened)))
}

fn reconcile_directory(
    storage: JournalStorageContext<'_>,
    maximum_bytes: usize,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    validate_root_identity(
        storage.root,
        storage.root_handle,
        storage.root_identity,
        storage.root_owner,
    )?;
    validate_lock_identity(
        storage.root,
        storage.lock_handle,
        storage.lock_identity,
        storage.root_owner,
    )?;
    let mut remove_next = false;
    for entry in fs::read_dir(storage.root)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?
    {
        let entry = entry
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        let name = entry.file_name();
        if name == JOURNAL_LOCK_FILE || name == JOURNAL_STATE_FILE {
            continue;
        }
        if name == JOURNAL_NEXT_FILE && !remove_next {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|_| {
                DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable
            })?;
            validate_private_file(&metadata, storage.root_owner)?;
            if usize::try_from(metadata.len())
                .ok()
                .is_none_or(|length| length > maximum_bytes)
            {
                return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
            }
            remove_next = true;
            continue;
        }
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    if remove_next {
        let path = storage.root.join(JOURNAL_NEXT_FILE);
        let before = fs::symlink_metadata(&path)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        validate_private_file(&before, storage.root_owner)?;
        fs::remove_file(&path)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        storage
            .root_handle
            .sync_all()
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    }
    validate_root_identity(
        storage.root,
        storage.root_handle,
        storage.root_identity,
        storage.root_owner,
    )?;
    validate_lock_identity(
        storage.root,
        storage.lock_handle,
        storage.lock_identity,
        storage.root_owner,
    )
}

fn validate_lock_identity(
    root: &Path,
    lock_handle: &File,
    identity: JournalFileIdentity,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let path = root.join(JOURNAL_LOCK_FILE);
    let named = fs::symlink_metadata(&path)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened = lock_handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_file(&named, root_owner)?;
    validate_private_file(&opened, root_owner)?;
    if named.len() != 0
        || opened.len() != 0
        || !identity.matches(&named)
        || !identity.matches(&opened)
        || !same_file(&named, &opened)
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}

fn validate_root_identity(
    root: &Path,
    root_handle: &File,
    identity: JournalFileIdentity,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    let named = fs::symlink_metadata(root)
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    let opened = root_handle
        .metadata()
        .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
    validate_private_root(&named)?;
    validate_private_root(&opened)?;
    if metadata_owner(&named) != root_owner
        || metadata_owner(&opened) != root_owner
        || !identity.matches(&named)
        || !identity.matches(&opened)
    {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
    }
    Ok(())
}

fn optional_metadata(
    path: &Path,
) -> Result<Option<fs::Metadata>, DurableMusubiPublicationServiceJournalOpenErrorV1> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable),
    }
}

fn validate_private_root(
    metadata: &fs::Metadata,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    #[cfg(unix)]
    if metadata.mode() & 0o7777 != 0o700 {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot);
    }
    Ok(())
}

fn validate_private_file(
    metadata: &fs::Metadata,
    root_owner: u32,
) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
    #[cfg(not(unix))]
    let _ = root_owner;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    #[cfg(unix)]
    if metadata.mode() & 0o7777 != 0o600 || metadata.nlink() != 1 || metadata.uid() != root_owner {
        return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState);
    }
    Ok(())
}

struct PrivateJournalTemporaryFile {
    path: PathBuf,
    file: File,
    identity: JournalFileIdentity,
    armed: bool,
}

impl PrivateJournalTemporaryFile {
    fn create(
        root: &Path,
        root_owner: u32,
    ) -> Result<Self, DurableMusubiPublicationServiceJournalOpenErrorV1> {
        let path = root.join(JOURNAL_NEXT_FILE);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        options
            .mode(0o600)
            .custom_flags(secure_no_follow_nonblocking_flags());
        let file = options
            .open(&path)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        let metadata = file
            .metadata()
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        let pending = Self {
            path,
            file,
            identity: JournalFileIdentity::from_metadata(&metadata),
            armed: true,
        };
        pending.validate(root_owner)?;
        Ok(pending)
    }

    fn validate(
        &self,
        root_owner: u32,
    ) -> Result<(), DurableMusubiPublicationServiceJournalOpenErrorV1> {
        let opened = self
            .file
            .metadata()
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        let named = fs::symlink_metadata(&self.path)
            .map_err(|_| DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable)?;
        validate_private_file(&opened, root_owner)?;
        validate_private_file(&named, root_owner)?;
        if !self.identity.matches(&opened)
            || !self.identity.matches(&named)
            || !same_file(&opened, &named)
        {
            return Err(DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable);
        }
        Ok(())
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PrivateJournalTemporaryFile {
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
struct JournalFileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl JournalFileIdentity {
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
struct JournalFileIdentity;

#[cfg(not(unix))]
impl JournalFileIdentity {
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
    use std::os::unix::fs::PermissionsExt as _;

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        NetworkId,
        account::AccountId,
        block::BlockHeader,
        musubi::{ArchiveId, MusubiContentDigestV1},
        sorafs::capacity::ProviderId,
    };

    use super::*;
    use crate::musubi_runtime::MusubiPublicationServiceConfigurationV1;

    fn network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            [seed; 32],
        )))
    }

    fn private_tempdir() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("private journal root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("set private journal-root permissions");
        root
    }

    fn configuration() -> MusubiPublicationServiceConfigurationV1 {
        let broker_key = KeyPair::try_from_seed(
            b"musubi-durable-publication-journal-test".to_vec(),
            Algorithm::Ed25519,
        )
        .expect("derive broker key");
        MusubiPublicationServiceConfigurationV1 {
            network_id: network_id(0x11),
            ingress_broker: AccountId::new(broker_key.public_key().clone()),
            seed_provider: ProviderId::new([0x12; 32]),
            max_future_clock_skew_ms: 1_000,
            receipt_lifetime_ms: 60_000,
        }
    }

    fn journal_binding(
        configuration: &MusubiPublicationServiceConfigurationV1,
    ) -> MusubiPublicationServiceJournalBindingV1 {
        MusubiPublicationServiceJournalBindingV1::from_configuration(configuration)
    }

    fn limits() -> DurableMusubiPublicationServiceJournalLimitsV1 {
        DurableMusubiPublicationServiceJournalLimitsV1::new(
            8,
            32,
            u64::try_from(MAX_CONTROL_RESPONSE_BYTES).expect("response bound fits u64"),
            u64::try_from(MAX_CONTROL_RESPONSE_BYTES + 1024 * 1024)
                .expect("snapshot bound fits u64"),
        )
        .expect("valid journal limits")
    }

    fn attempt(
        configuration: &MusubiPublicationServiceConfigurationV1,
        operation_id: u8,
        authorization_digest: u8,
    ) -> MusubiPublicationJournalAttemptV1 {
        let operation_id = [operation_id; 32];
        let binding = MusubiPublicationOperationBindingV1 {
            operation_id,
            network_id: configuration.network_id,
            publisher: configuration.ingress_broker.clone(),
            archive_id: ArchiveId::new([0x21; 32]),
            car_body_digest: MusubiContentDigestV1::new([0x22; 32]),
            car_body_length: 99,
        };
        MusubiPublicationJournalAttemptV1 {
            key: MusubiPublicationIdempotencyKeyV1 {
                operation: MusubiPublicationRuntimeOperationV1::SeedIngress,
                operation_id,
                target: [0; 32],
            },
            binding,
            request_digest: [0x23; 32],
            authorization_digest: [authorization_digest; 32],
            authorization_expires_at_ms: 20_000,
        }
    }

    #[test]
    fn error_codes_and_limits_are_stable() {
        let cases = [
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::UnsupportedPlatform,
                "MUSUBI_PUBLICATION_JOURNAL_UNSUPPORTED_PLATFORM",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot,
                "MUSUBI_PUBLICATION_JOURNAL_UNSAFE_ROOT",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::Locked,
                "MUSUBI_PUBLICATION_JOURNAL_LOCKED",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized,
                "MUSUBI_PUBLICATION_JOURNAL_UNINITIALIZED",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::AlreadyInitialized,
                "MUSUBI_PUBLICATION_JOURNAL_ALREADY_INITIALIZED",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits,
                "MUSUBI_PUBLICATION_JOURNAL_INVALID_LIMITS",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::LimitsMismatch,
                "MUSUBI_PUBLICATION_JOURNAL_LIMITS_MISMATCH",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::ConfigurationMismatch,
                "MUSUBI_PUBLICATION_JOURNAL_CONFIGURATION_MISMATCH",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState,
                "MUSUBI_PUBLICATION_JOURNAL_INVALID_STATE",
            ),
            (
                DurableMusubiPublicationServiceJournalOpenErrorV1::StorageUnavailable,
                "MUSUBI_PUBLICATION_JOURNAL_STORAGE_UNAVAILABLE",
            ),
        ];
        for (error, code) in cases {
            assert_eq!(error.as_str(), code);
            assert_eq!(error.to_string(), code);
        }
        assert_eq!(limits().max_operations(), 8);
        assert_eq!(limits().max_authorizations(), 32);
        assert_eq!(
            results_per_operation(),
            MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 * MUSUBI_MAX_LOCATION_PROVIDERS_V1
                + 1
                + MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1
        );
        assert_eq!(
            DurableMusubiPublicationServiceJournalLimitsV1::new(0, 1, 1, 1),
            Err(DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidLimits)
        );
    }

    #[test]
    fn initialization_is_explicit_and_lifetime_lock_is_exclusive() {
        let root = private_tempdir();
        let configuration = configuration();
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("ordinary open must not initialize"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized
        );
        let journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        assert_eq!(journal.revision(), 1);
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("second owner rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::Locked
        );
        drop(journal);
        let reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("lock released after drop");
        assert_eq!(reopened.revision(), 1);
    }

    #[test]
    fn root_mode_rejects_special_permission_bits() {
        let root = private_tempdir();
        let configuration = configuration();
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o1700))
            .expect("set sticky private mode");
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::initialize(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("special mode bits rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::UnsafeRoot
        );
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700))
            .expect("restore tempdir mode");
    }

    #[test]
    fn completed_response_and_consumed_authorizations_survive_restart() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        let first = attempt(&configuration, 0x31, 0x32);
        assert_eq!(
            journal.begin(&first, 10_000).expect("reserve attempt"),
            MusubiPublicationJournalBeginV1::Execute
        );
        journal
            .commit(first.key, first.request_digest, b"canonical response")
            .expect("commit response");
        drop(journal);

        let mut reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("reopen journal");
        let mut retry = first.clone();
        retry.authorization_digest = [0x33; 32];
        assert_eq!(
            reopened.begin(&retry, 10_001).expect("cached retry"),
            MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
        );
        assert_eq!(
            reopened.begin(&retry, 10_001).expect("cached replay"),
            MusubiPublicationJournalBeginV1::Cached(b"canonical response".to_vec())
        );
        let mut cross_route_replay = first;
        cross_route_replay.key.operation = MusubiPublicationRuntimeOperationV1::ProviderReadback;
        cross_route_replay.key.target = [0x34; 32];
        assert_eq!(
            reopened.begin(&cross_route_replay, 10_002),
            Err(MusubiPublicationServiceJournalErrorV1::Replay)
        );
    }

    #[test]
    fn all_storage_location_generations_survive_durable_restart_and_ninth_fails_closed() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");

        for generation in 1..=MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 {
            let generation = u8::try_from(generation).expect("generation fits u8");
            let mut generation_attempt = attempt(&configuration, 0x35, 0x40 + generation);
            generation_attempt.key.operation =
                MusubiPublicationRuntimeOperationV1::StorageCoordination;
            generation_attempt.key.target =
                crate::musubi_runtime::storage_generation_target(generation);
            generation_attempt.request_digest = [0x50 + generation; 32];
            assert_eq!(
                journal
                    .begin(&generation_attempt, 10_000)
                    .expect("reserve storage generation"),
                MusubiPublicationJournalBeginV1::Execute
            );
            journal
                .commit(
                    generation_attempt.key,
                    generation_attempt.request_digest,
                    &[generation],
                )
                .expect("commit storage generation");
        }
        let revision = journal.revision();

        let mut ninth = attempt(&configuration, 0x35, 0x70);
        ninth.key.operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
        ninth.key.target = crate::musubi_runtime::storage_generation_target(
            u8::try_from(MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 + 1)
                .expect("ninth generation fits u8"),
        );
        ninth.request_digest = [0x71; 32];
        assert_eq!(
            journal.begin(&ninth, 10_001),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );

        let mut malformed = ninth;
        malformed.key.target = crate::musubi_runtime::storage_generation_target(1);
        malformed.key.target[1] = 1;
        malformed.authorization_digest = [0x72; 32];
        assert_eq!(
            journal.begin(&malformed, 10_001),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );
        assert_eq!(journal.revision(), revision);
        drop(journal);

        let mut reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("reopen all storage generations");
        for generation in 1..=MUSUBI_MAX_PUBLICATION_LOCATION_ATTEMPTS_V1 {
            let generation = u8::try_from(generation).expect("generation fits u8");
            let mut retry = attempt(&configuration, 0x35, 0x80 + generation);
            retry.key.operation = MusubiPublicationRuntimeOperationV1::StorageCoordination;
            retry.key.target = crate::musubi_runtime::storage_generation_target(generation);
            retry.request_digest = [0x50 + generation; 32];
            assert_eq!(
                reopened
                    .begin(&retry, 10_002)
                    .expect("recover cached storage generation"),
                MusubiPublicationJournalBeginV1::Cached(vec![generation])
            );
        }
    }

    #[test]
    fn startup_recovers_pending_to_aborted_without_dropping_replay_state() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        let first = attempt(&configuration, 0x41, 0x42);
        assert_eq!(
            journal.begin(&first, 10_000).expect("reserve attempt"),
            MusubiPublicationJournalBeginV1::Execute
        );
        let revision = journal.revision();
        drop(journal);

        let mut reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("recover journal");
        assert_eq!(reopened.revision(), revision + 1);
        assert_eq!(
            reopened.begin(&first, 10_001),
            Err(MusubiPublicationServiceJournalErrorV1::Replay)
        );
        let mut fresh = first;
        fresh.authorization_digest = [0x43; 32];
        assert_eq!(
            reopened.begin(&fresh, 10_001).expect("fresh retry"),
            MusubiPublicationJournalBeginV1::Execute
        );
    }

    #[test]
    fn startup_restores_previous_response_from_interrupted_refresh() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        let first = attempt(&configuration, 0x51, 0x52);
        journal.begin(&first, 10_000).expect("reserve attempt");
        journal
            .commit(first.key, first.request_digest, b"prior receipt")
            .expect("commit prior receipt");
        let mut refresh = first.clone();
        refresh.authorization_digest = [0x53; 32];
        assert_eq!(
            journal
                .begin(&refresh, 10_001)
                .expect("read cached receipt"),
            MusubiPublicationJournalBeginV1::Cached(b"prior receipt".to_vec())
        );
        journal
            .refresh_expired_seed_receipt(&refresh, b"prior receipt", 10_001)
            .expect("begin refresh");
        drop(journal);

        let mut reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("recover refresh");
        let mut retry = first;
        retry.authorization_digest = [0x54; 32];
        assert_eq!(
            reopened
                .begin(&retry, 10_002)
                .expect("cached prior response"),
            MusubiPublicationJournalBeginV1::Cached(b"prior receipt".to_vec())
        );
    }

    #[test]
    fn worst_case_response_capacity_is_reserved_before_backend_work() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        let first = attempt(&configuration, 0x61, 0x62);
        journal.begin(&first, 10_000).expect("first reservation");
        let second = attempt(&configuration, 0x63, 0x64);
        assert_eq!(
            journal.begin(&second, 10_000),
            Err(MusubiPublicationServiceJournalErrorV1::Capacity)
        );
        journal
            .abort(first.key, first.request_digest)
            .expect("release first reservation");
        assert_eq!(
            journal.begin(&second, 10_001).expect("capacity released"),
            MusubiPublicationJournalBeginV1::Execute
        );
    }

    #[test]
    fn invalid_attempts_cannot_persist_a_snapshot_that_reopen_rejects() {
        let root = private_tempdir();
        let configuration = configuration();
        let mut journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        let initial_revision = journal.revision();

        let mut foreign_chain = attempt(&configuration, 0x65, 0x66);
        foreign_chain.binding.network_id = network_id(0x70);
        assert_eq!(
            journal.begin(&foreign_chain, 10_000),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );

        let mut zero_expiry = attempt(&configuration, 0x67, 0x68);
        zero_expiry.authorization_expires_at_ms = 0;
        assert_eq!(
            journal.begin(&zero_expiry, 10_000),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );

        let zero_time = attempt(&configuration, 0x69, 0x6a);
        assert_eq!(
            journal.begin(&zero_time, 0),
            Err(MusubiPublicationServiceJournalErrorV1::Invalid)
        );
        assert_eq!(journal.revision(), initial_revision);
        drop(journal);
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect("invalid attempts left canonical state")
            .revision(),
            initial_revision
        );
    }

    #[test]
    fn candidate_validation_rejects_orphans_and_expiry_index_divergence() {
        let configuration = configuration();
        let binding = journal_binding(&configuration);
        let limits = limits();
        let mut journal = InMemoryMusubiPublicationServiceJournalV1::new(
            binding.clone(),
            limits.max_operations_usize(),
            limits.max_authorizations_usize(),
        )
        .expect("bounded journal");
        let first = attempt(&configuration, 0x6b, 0x6c);
        journal
            .operation_bindings
            .insert(first.binding.operation_id, first.binding.clone());
        assert_eq!(
            state_from_journal(&journal, &binding, limits, 2),
            Err(CandidateStateErrorV1::Invalid)
        );

        journal.operation_bindings.clear();
        journal.begin(&first, 10_000).expect("valid reservation");
        journal.expiry_index.clear();
        assert_eq!(
            state_from_journal(&journal, &binding, limits, 2),
            Err(CandidateStateErrorV1::Invalid)
        );
    }

    #[test]
    fn nonempty_lifetime_lock_fails_closed() {
        let root = private_tempdir();
        let configuration = configuration();
        let journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        drop(journal);
        fs::write(
            root.path().join(JOURNAL_LOCK_FILE),
            b"substituted lock state",
        )
        .expect("mutate lock file");
        fs::set_permissions(
            root.path().join(JOURNAL_LOCK_FILE),
            fs::Permissions::from_mode(0o600),
        )
        .expect("retain private lock mode");
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("nonempty lock rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState
        );
    }

    #[test]
    fn configuration_limits_corruption_and_deleted_state_fail_closed() {
        let root = private_tempdir();
        let configuration = configuration();
        let journal = DurableMusubiPublicationServiceJournalV1::initialize(
            root.path(),
            journal_binding(&configuration),
            limits(),
        )
        .expect("initialize journal");
        drop(journal);

        let mut wrong_configuration = configuration.clone();
        wrong_configuration.network_id = network_id(0x71);
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&wrong_configuration),
                limits(),
            )
            .expect_err("configuration mismatch rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::ConfigurationMismatch
        );
        let different_limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
            9,
            32,
            u64::try_from(MAX_CONTROL_RESPONSE_BYTES).expect("response bound fits u64"),
            u64::try_from(MAX_CONTROL_RESPONSE_BYTES + 1024 * 1024)
                .expect("snapshot bound fits u64"),
        )
        .expect("valid alternate limits");
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                different_limits,
            )
            .expect_err("limits mismatch rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::LimitsMismatch
        );

        let mut changed_timing_policy = configuration.clone();
        changed_timing_policy.max_future_clock_skew_ms = 2_000;
        changed_timing_policy.receipt_lifetime_ms = 30_000;
        let reopened = DurableMusubiPublicationServiceJournalV1::open(
            root.path(),
            journal_binding(&changed_timing_policy),
            limits(),
        )
        .expect("timing policy is not durable replay identity");
        drop(reopened);

        fs::write(root.path().join(JOURNAL_STATE_FILE), b"not norito")
            .expect("corrupt journal state");
        fs::set_permissions(
            root.path().join(JOURNAL_STATE_FILE),
            fs::Permissions::from_mode(0o600),
        )
        .expect("retain private state mode");
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("corrupt state rejected"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::InvalidState
        );
        fs::remove_file(root.path().join(JOURNAL_STATE_FILE)).expect("delete corrupt state");
        assert_eq!(
            DurableMusubiPublicationServiceJournalV1::open(
                root.path(),
                journal_binding(&configuration),
                limits(),
            )
            .expect_err("missing state not regenerated"),
            DurableMusubiPublicationServiceJournalOpenErrorV1::Uninitialized
        );
    }
}
