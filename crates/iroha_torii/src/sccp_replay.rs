//! Fail-closed Torii bootstrap and read provider for SCCP replay archives.
//!
//! Archive replicas are availability services, not consensus authorities.
//! Torii accepts a checkpoint only when all three configured HTTPS origins
//! return byte-identical canonical data, every pinned Ed25519 signature
//! verifies, every snapshot rebuilds, predecessor continuity holds, and a
//! fresh Kura scan reproduces the complete Core replay-forest projection.

#[cfg(unix)]
use std::io::Write as _;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read as _,
    num::NonZeroUsize,
    path::Path,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use iroha_config::parameters::actual::{ToriiSccpReplayArchive, ToriiSccpReplayArchiveReplica};
use iroha_core::{
    bridge::rebuild_sccp_replay_archive_from_kura_v1,
    kura::Kura,
    state::{State as CoreState, WorldReadOnly as _},
};
use iroha_data_model::bridge::{
    SccpReplayAccumulatorIdV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayDomainV1,
    SccpReplayForestV1, SccpSparseMerkleWitnessV1,
};
use iroha_sccp::{
    SccpReplayArchiveCheckpointBodyV1, SccpReplayArchiveDecodeLimitsV1,
    SccpReplayArchiveProviderErrorV1, SccpReplayArchiveProviderV1,
    SccpReplayArchiveReplicaBindingV1, SccpReplayArchiveReplicaPolicyV1,
    SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveSnapshotV1, SccpReplayArchiveV1,
    decode_sccp_replay_archive_snapshot_v1, sccp_replay_archive_network_identity_sha256_v1,
    verify_sccp_replay_archive_checkpoint_v1,
};
use mv::storage::StorageReadOnly as _;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

/// Canonical media type served by independent replay replicas.
pub const SCCP_REPLAY_CHECKPOINT_SET_MEDIA_TYPE_V1: &str = "application/x-iroha-norito";
/// Fixed relative endpoint fetched from each configured replica origin.
pub const SCCP_REPLAY_CHECKPOINT_SET_PATH_V1: &str = "v1/sccp/replay/checkpoint-set-v1";

const CHECKPOINT_SET_VERSION_V1: u8 = 1;
const HEAD_MANIFEST_VERSION_V1: u8 = 1;
const HEAD_MANIFEST_FILENAME_V1: &str = "head-v1.norito";
#[cfg(unix)]
const PROCESS_LOCK_FILENAME_V1: &str = "archive-v1.lock";
const CHECKPOINT_SET_DIGEST_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SET-V1";
const MAX_PERSISTED_CHECKPOINT_BYTES_V1: usize = 4 * 1024 * 1024;
#[cfg(unix)]
const SECURE_TEMP_RETRIES_V1: usize = 32;

/// One complete signed checkpoint and its exact canonical snapshot bytes.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayReplicaCheckpointEntryV1 {
    /// Exactly-three-signature checkpoint statement.
    pub checkpoint: SccpReplayArchiveSignedCheckpointV1,
    /// Canonical Norito snapshot whose content hash is signed by the checkpoint.
    pub snapshot_bytes: Vec<u8>,
}

/// Exact checkpoint set returned independently by all three replicas.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayReplicaCheckpointSetV1 {
    /// Schema version; final V1 accepts exactly one.
    pub version: u8,
    /// Strictly accumulator-id-ordered complete replay inventory.
    pub entries: Vec<SccpReplayReplicaCheckpointEntryV1>,
}

/// Payload-free source failure. Replica responses and URLs are never retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayCheckpointSourceErrorV1 {
    /// HTTPS client construction or transport failed.
    Transport,
    /// Status, headers, or response framing was not the exact protocol shape.
    Protocol,
    /// The declared response byte ceiling was exceeded.
    Limit,
}

/// Source of one bounded checkpoint-set response for a pinned replica.
pub trait SccpReplayCheckpointSourceV1: Send + Sync {
    /// Fetch one exact response without following redirects.
    fn fetch(
        &self,
        replica: &ToriiSccpReplayArchiveReplica,
        max_response_bytes: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>, SccpReplayCheckpointSourceErrorV1>;
}

/// HTTPS implementation used by production Torii startup and refreshes.
pub struct HttpsSccpReplayCheckpointSourceV1 {
    client: reqwest::blocking::Client,
}

impl HttpsSccpReplayCheckpointSourceV1 {
    /// Build a redirect-free HTTPS client with one complete request deadline.
    pub fn new(timeout: Duration) -> Result<Self, SccpReplayCheckpointSourceErrorV1> {
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(timeout)
            .timeout(timeout)
            .build()
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
        Ok(Self { client })
    }
}

impl SccpReplayCheckpointSourceV1 for HttpsSccpReplayCheckpointSourceV1 {
    fn fetch(
        &self,
        replica: &ToriiSccpReplayArchiveReplica,
        max_response_bytes: usize,
        _timeout: Duration,
    ) -> Result<Vec<u8>, SccpReplayCheckpointSourceErrorV1> {
        let max_response_bytes_u64 = u64::try_from(max_response_bytes)
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Limit)?;
        let read_limit = max_response_bytes_u64
            .checked_add(1)
            .ok_or(SccpReplayCheckpointSourceErrorV1::Limit)?;
        let url = replica
            .origin
            .join(SCCP_REPLAY_CHECKPOINT_SET_PATH_V1)
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Protocol)?;
        if url.scheme() != "https"
            || url.host_str() != replica.origin.host_str()
            || url.port_or_known_default() != replica.origin.port_or_known_default()
        {
            return Err(SccpReplayCheckpointSourceErrorV1::Protocol);
        }
        let mut response = self
            .client
            .get(url)
            .header(
                reqwest::header::ACCEPT,
                SCCP_REPLAY_CHECKPOINT_SET_MEDIA_TYPE_V1,
            )
            .send()
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
        let mut content_types = response
            .headers()
            .get_all(reqwest::header::CONTENT_TYPE)
            .iter();
        let exact_content_type = content_types.next().and_then(|value| value.to_str().ok())
            == Some(SCCP_REPLAY_CHECKPOINT_SET_MEDIA_TYPE_V1)
            && content_types.next().is_none();
        if response.status() != reqwest::StatusCode::OK
            || response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .is_some()
            || !exact_content_type
        {
            return Err(SccpReplayCheckpointSourceErrorV1::Protocol);
        }
        if response
            .content_length()
            .is_some_and(|length| length > max_response_bytes_u64)
        {
            return Err(SccpReplayCheckpointSourceErrorV1::Limit);
        }
        let mut bytes = Vec::new();
        (&mut response)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
        if bytes.is_empty() || bytes.len() > max_response_bytes {
            return Err(SccpReplayCheckpointSourceErrorV1::Limit);
        }
        Ok(bytes)
    }
}

/// Payload-free failure from the local consensus authority boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayLocalAuthorityErrorV1 {
    /// The supplied head is not the exact current committed coordinate.
    Finality,
    /// The signed forest set differs from current Core state.
    CoreMismatch,
    /// Kura execution could not reproduce the complete forest set.
    Rebuild,
}

/// Narrow boundary that proves a remote forest inventory against local Core
/// state and commit-authenticated Kura execution.
pub trait SccpReplayLocalAuthorityV1: Send + Sync {
    /// Return the Kura-rebuilt archive only after all local checks succeed.
    fn rebuild_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1>;
}

struct CoreKuraSccpReplayLocalAuthorityV1 {
    state: Arc<CoreState>,
    kura: Arc<Kura>,
}

impl SccpReplayLocalAuthorityV1 for CoreKuraSccpReplayLocalAuthorityV1 {
    fn rebuild_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        let committed_height = self.state.committed_height();
        if usize::try_from(finality.finalized_height).ok() != Some(committed_height)
            || sccp_replay_archive_network_identity_sha256_v1(self.state.network_id_ref())
                != finality.network_identity_sha256
        {
            return Err(SccpReplayLocalAuthorityErrorV1::Finality);
        }
        let height =
            NonZeroUsize::new(committed_height).ok_or(SccpReplayLocalAuthorityErrorV1::Finality)?;
        if self.kura.get_block_hash(height).map(|hash| *hash.as_ref())
            != Some(finality.finalized_block_hash)
        {
            return Err(SccpReplayLocalAuthorityErrorV1::Finality);
        }

        let authoritative = authoritative_core_replay_inventory(&self.state)?;
        if &authoritative != expected {
            return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
        }

        rebuild_sccp_replay_archive_from_kura_v1(&self.kura, height, expected)
            .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)
    }
}

fn authoritative_core_replay_inventory(
    state: &CoreState,
) -> Result<
    BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    SccpReplayLocalAuthorityErrorV1,
> {
    let registry = state.sccp_registry_snapshot();
    let world = state.world_view();
    let mut authoritative = BTreeMap::new();
    for route in registry.lanes().iter().flat_map(|lane| &lane.routes) {
        let route_key = route.key();
        let route_configuration_hash = route
            .route_configuration_hash()
            .map_err(|_| SccpReplayLocalAuthorityErrorV1::CoreMismatch)?;
        for (boundary, source_network, target_network) in [
            (
                SccpReplayBoundaryV1::SoraOutboundLock,
                route.lane_id.target,
                route.lane_id.source,
            ),
            (
                SccpReplayBoundaryV1::SoraInboundRelease,
                route.lane_id.source,
                route.lane_id.target,
            ),
        ] {
            let accumulator_id = SccpReplayAccumulatorIdV1 {
                route_key: route_key.clone(),
                boundary,
            };
            let domain = SccpReplayDomainV1 {
                source_network,
                target_network,
                boundary,
                route_revision: route.revision,
                route_configuration_hash,
                actor: SccpReplayActorV1::Route,
            };
            let forest = world
                .sccp_replay_forests()
                .get(&accumulator_id)
                .cloned()
                .unwrap_or_default();
            if authoritative
                .insert(accumulator_id, (domain, forest))
                .is_some()
            {
                return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
            }
        }
    }
    if world.sccp_replay_forests().iter().any(|(id, forest)| {
        authoritative
            .get(id)
            .is_none_or(|(_, expected)| expected != forest)
    }) {
        return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
    }
    Ok(authoritative)
}

/// Stable, payload-free startup/refresh failures suitable for operator logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToriiSccpReplayStartupErrorV1 {
    /// Configured transport could not return one bounded response per replica.
    Transport,
    /// Replica responses were not byte-identical.
    ReplicaDisagreement,
    /// Checkpoint-set or snapshot framing was malformed or exceeded limits.
    Malformed,
    /// One or more pinned signatures failed authentication.
    ReplicaAuthentication,
    /// A cached, regressed, forked, or discontinuous head was supplied.
    Continuity,
    /// Current Core or Kura state disagreed with the signed forest inventory.
    LocalAuthority,
    /// The owner-only descriptor-relative store could not be trusted or synced.
    Persistence,
    /// Secure descriptor-relative publication is unavailable on this platform.
    UnsupportedPlatform,
}

impl core::fmt::Display for ToriiSccpReplayStartupErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Transport => "SCCP replay replica transport unavailable",
            Self::ReplicaDisagreement => "SCCP replay replicas disagree",
            Self::Malformed => "malformed SCCP replay checkpoint set",
            Self::ReplicaAuthentication => "SCCP replay replica authentication failed",
            Self::Continuity => "SCCP replay checkpoint continuity failed",
            Self::LocalAuthority => "SCCP replay checkpoint differs from local authority",
            Self::Persistence => "SCCP replay checkpoint persistence failed",
            Self::UnsupportedPlatform => "secure SCCP replay persistence is unsupported",
        })
    }
}

impl std::error::Error for ToriiSccpReplayStartupErrorV1 {}

/// Typed, nonleaking error consumed by replay-specific HTTP adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToriiSccpReplayEndpointErrorV1 {
    /// Replay archive service is disabled by configuration.
    Disabled,
    /// The exact accumulator or key is not retained.
    NotFound,
    /// The service has no locally authenticated current head.
    Unavailable,
    /// Locally retained data no longer passes its integrity checks.
    Integrity,
}

impl ToriiSccpReplayEndpointErrorV1 {
    /// Stable machine code; no attacker-controlled detail is included.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Disabled => "sccp_replay_disabled",
            Self::NotFound => "sccp_replay_not_found",
            Self::Unavailable => "sccp_replay_unavailable",
            Self::Integrity => "sccp_replay_integrity",
        }
    }
}

impl core::fmt::Display for ToriiSccpReplayEndpointErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ToriiSccpReplayEndpointErrorV1 {}

impl From<SccpReplayArchiveProviderErrorV1> for ToriiSccpReplayEndpointErrorV1 {
    fn from(error: SccpReplayArchiveProviderErrorV1) -> Self {
        match error {
            SccpReplayArchiveProviderErrorV1::NotFound => Self::NotFound,
            SccpReplayArchiveProviderErrorV1::Unavailable => Self::Unavailable,
            SccpReplayArchiveProviderErrorV1::Integrity => Self::Integrity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct PersistedReplayHeadEntryV1 {
    accumulator_id: SccpReplayAccumulatorIdV1,
    snapshot_sha256: [u8; 32],
    checkpoint_agreement_digest: [u8; 32],
    checkpoint_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct PersistedReplayHeadV1 {
    version: u8,
    checkpoint_set_sha256: [u8; 32],
    network_identity_sha256: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
    entries: Vec<PersistedReplayHeadEntryV1>,
}

struct ValidatedCheckpointEntryV1 {
    snapshot: SccpReplayArchiveSnapshotV1,
    snapshot_bytes: Vec<u8>,
    checkpoint: SccpReplayArchiveSignedCheckpointV1,
    checkpoint_bytes: Vec<u8>,
    checkpoint_agreement_digest: [u8; 32],
    checkpoint_sha256: [u8; 32],
}

struct PersistedReplayHeadStateV1 {
    manifest: PersistedReplayHeadV1,
    entries: Vec<ValidatedCheckpointEntryV1>,
}

struct PublishedReplayStateV1 {
    archive: SccpReplayArchiveV1,
    checkpoints: BTreeMap<SccpReplayAccumulatorIdV1, SccpReplayArchiveSignedCheckpointV1>,
    checkpoint_set_sha256: [u8; 32],
}

struct CandidateReplayStateV1 {
    published: PublishedReplayStateV1,
    manifest: PersistedReplayHeadV1,
    entries: Vec<ValidatedCheckpointEntryV1>,
}

/// Live Torii provider. The visible state changes only after every immutable
/// artifact and the manifest-last head are durably published.
pub struct ToriiSccpReplayArchiveServiceV1 {
    config: ToriiSccpReplayArchive,
    source: Arc<dyn SccpReplayCheckpointSourceV1>,
    local_authority: Arc<dyn SccpReplayLocalAuthorityV1>,
    store: SecureReplayStoreV1,
    update_lock: Mutex<()>,
    published: RwLock<PublishedReplayStateV1>,
}

impl ToriiSccpReplayArchiveServiceV1 {
    /// Bootstrap the production HTTPS reader against current Core and Kura.
    pub fn bootstrap(
        config: ToriiSccpReplayArchive,
        state: Arc<CoreState>,
        kura: Arc<Kura>,
    ) -> Result<Arc<Self>, ToriiSccpReplayStartupErrorV1> {
        validate_runtime_config(&config)?;
        let source = Arc::new(HttpsSccpReplayCheckpointSourceV1::new(
            config.request_timeout,
        )?);
        let local_authority = Arc::new(CoreKuraSccpReplayLocalAuthorityV1 { state, kura });
        Self::bootstrap_with_components(config, source, local_authority)
    }

    /// Bootstrap with explicit transport and local-authority boundaries.
    ///
    /// This constructor exists for deterministic integration testing and for
    /// deployments that wrap the same pinned HTTPS policy in audited transport
    /// isolation. Neither boundary can inject a signing key.
    pub fn bootstrap_with_components(
        config: ToriiSccpReplayArchive,
        source: Arc<dyn SccpReplayCheckpointSourceV1>,
        local_authority: Arc<dyn SccpReplayLocalAuthorityV1>,
    ) -> Result<Arc<Self>, ToriiSccpReplayStartupErrorV1> {
        validate_runtime_config(&config)?;
        let store = SecureReplayStoreV1::open(&config.state_dir)?;
        let previous = store.load_head(&config)?;
        let bytes = fetch_exact_three(&config, source.as_ref())?;
        let candidate =
            validate_candidate(&config, &bytes, previous.as_ref(), local_authority.as_ref())?;
        store.persist_candidate(&config, &candidate)?;
        Ok(Arc::new(Self {
            config,
            source,
            local_authority,
            store,
            update_lock: Mutex::new(()),
            published: RwLock::new(candidate.published),
        }))
    }

    /// Fetch and atomically publish one strictly newer three-replica head.
    pub fn refresh(&self) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let previous = self
            .store
            .load_head(&self.config)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        let bytes = fetch_exact_three(&self.config, self.source.as_ref())?;
        let candidate = validate_candidate(
            &self.config,
            &bytes,
            Some(&previous),
            self.local_authority.as_ref(),
        )?;
        self.store.persist_candidate(&self.config, &candidate)?;
        let mut published = self
            .published
            .write()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        *published = candidate.published;
        Ok(())
    }

    /// Digest of the exact currently visible three-replica checkpoint set.
    pub fn checkpoint_set_sha256(&self) -> Result<[u8; 32], ToriiSccpReplayEndpointErrorV1> {
        self.published
            .read()
            .map(|state| state.checkpoint_set_sha256)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)
    }
}

fn validate_runtime_config(
    config: &ToriiSccpReplayArchive,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    use iroha_config::parameters::defaults::torii::sccp_replay_archive as limits;

    replica_policy(config)
        .validate()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)?;
    let mut origins = BTreeSet::new();
    for replica in &config.replicas {
        let origin = &replica.origin;
        if origin.scheme() != "https"
            || origin.host_str().is_none_or(|host| host.ends_with('.'))
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.as_str() != format!("{}/", origin.origin().ascii_serialization())
            || !origins.insert(origin.as_str())
        {
            return Err(ToriiSccpReplayStartupErrorV1::Malformed);
        }
    }
    let response_bytes = u64::try_from(config.max_response_bytes)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let snapshot_bytes = u64::try_from(config.max_snapshot_bytes)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let snapshot_leaves = u64::try_from(config.max_snapshot_leaves)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let accumulators = u64::try_from(config.max_accumulators)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if response_bytes == 0
        || response_bytes > limits::MAX_RESPONSE_BYTES_HARD
        || snapshot_bytes == 0
        || snapshot_bytes > response_bytes
        || snapshot_bytes > limits::MAX_SNAPSHOT_BYTES_HARD
        || snapshot_leaves == 0
        || snapshot_leaves > limits::MAX_SNAPSHOT_LEAVES_HARD
        || accumulators == 0
        || accumulators > limits::MAX_ACCUMULATORS_HARD
        || config.request_timeout.is_zero()
        || config.request_timeout > limits::REQUEST_TIMEOUT_HARD
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    Ok(())
}

impl SccpReplayArchiveProviderV1 for ToriiSccpReplayArchiveServiceV1 {
    fn forest(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<(SccpReplayDomainV1, SccpReplayForestV1), SccpReplayArchiveProviderErrorV1> {
        let published = self
            .published
            .read()
            .map_err(|_| SccpReplayArchiveProviderErrorV1::Integrity)?;
        SccpReplayArchiveProviderV1::forest(&published.archive, accumulator_id)
    }

    fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveProviderErrorV1> {
        let published = self
            .published
            .read()
            .map_err(|_| SccpReplayArchiveProviderErrorV1::Integrity)?;
        SccpReplayArchiveProviderV1::witness(&published.archive, accumulator_id, key)
    }

    fn checkpoint(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveProviderErrorV1> {
        self.published
            .read()
            .map_err(|_| SccpReplayArchiveProviderErrorV1::Integrity)?
            .checkpoints
            .get(accumulator_id)
            .cloned()
            .ok_or(SccpReplayArchiveProviderErrorV1::NotFound)
    }
}

fn replica_policy(config: &ToriiSccpReplayArchive) -> SccpReplayArchiveReplicaPolicyV1 {
    SccpReplayArchiveReplicaPolicyV1 {
        replicas: config
            .replicas
            .clone()
            .map(|replica| SccpReplayArchiveReplicaBindingV1 {
                replica_id: replica.replica_id,
                ed25519_public_key: replica.ed25519_public_key,
            }),
    }
}

fn fetch_exact_three(
    config: &ToriiSccpReplayArchive,
    source: &dyn SccpReplayCheckpointSourceV1,
) -> Result<Vec<u8>, ToriiSccpReplayStartupErrorV1> {
    let mut agreed: Option<Vec<u8>> = None;
    for replica in &config.replicas {
        let bytes = source
            .fetch(replica, config.max_response_bytes, config.request_timeout)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?;
        if let Some(expected) = &agreed {
            if expected != &bytes {
                return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
            }
        } else {
            agreed = Some(bytes);
        }
    }
    agreed.ok_or(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement)
}

fn validate_candidate(
    config: &ToriiSccpReplayArchive,
    bytes: &[u8],
    previous: Option<&PersistedReplayHeadStateV1>,
    local_authority: &dyn SccpReplayLocalAuthorityV1,
) -> Result<CandidateReplayStateV1, ToriiSccpReplayStartupErrorV1> {
    if bytes.is_empty() || bytes.len() > config.max_response_bytes {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let set: SccpReplayReplicaCheckpointSetV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if norito::to_bytes(&set).ok().as_deref() != Some(bytes)
        || set.version != CHECKPOINT_SET_VERSION_V1
        || set.entries.is_empty()
        || set.entries.len() > config.max_accumulators
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let policy = replica_policy(config);
    let limits = SccpReplayArchiveDecodeLimitsV1 {
        max_snapshot_bytes: config.max_snapshot_bytes,
        max_snapshot_leaves: config.max_snapshot_leaves,
    };
    let mut entries = Vec::with_capacity(set.entries.len());
    let mut expected = BTreeMap::new();
    let mut checkpoints = BTreeMap::new();
    let mut previous_id = None;
    let mut common_finality = None;
    for entry in set.entries {
        let validated = validate_entry(entry.checkpoint, entry.snapshot_bytes, &policy, limits)?;
        let id = validated.snapshot.accumulator_id.clone();
        if previous_id.as_ref().is_some_and(|prior| prior >= &id)
            || expected
                .insert(
                    id.clone(),
                    (validated.snapshot.domain, validated.snapshot.forest.clone()),
                )
                .is_some()
            || checkpoints
                .insert(id.clone(), validated.checkpoint.clone())
                .is_some()
        {
            return Err(ToriiSccpReplayStartupErrorV1::Malformed);
        }
        previous_id = Some(id);
        let coordinate = (
            validated.snapshot.finality.network_identity_sha256,
            validated.snapshot.finality.finalized_height,
            validated.snapshot.finality.finalized_block_hash,
        );
        if common_finality.is_some_and(|expected| expected != coordinate) {
            return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
        }
        common_finality = Some(coordinate);
        entries.push(validated);
    }
    let (network_identity_sha256, finalized_height, finalized_block_hash) =
        common_finality.ok_or(ToriiSccpReplayStartupErrorV1::Malformed)?;
    validate_continuity(
        previous,
        &entries,
        network_identity_sha256,
        finalized_height,
        finalized_block_hash,
    )?;
    let finality = entries[0].snapshot.finality;
    let archive = local_authority
        .rebuild_and_verify(finality, &expected)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
    let checkpoint_set_sha256 = sha256(&[CHECKPOINT_SET_DIGEST_DOMAIN_V1, bytes]);
    let manifest_entries = entries
        .iter()
        .map(|entry| PersistedReplayHeadEntryV1 {
            accumulator_id: entry.snapshot.accumulator_id.clone(),
            snapshot_sha256: entry.checkpoint.body.snapshot_sha256,
            checkpoint_agreement_digest: entry.checkpoint_agreement_digest,
            checkpoint_sha256: entry.checkpoint_sha256,
        })
        .collect();
    Ok(CandidateReplayStateV1 {
        published: PublishedReplayStateV1 {
            archive,
            checkpoints,
            checkpoint_set_sha256,
        },
        manifest: PersistedReplayHeadV1 {
            version: HEAD_MANIFEST_VERSION_V1,
            checkpoint_set_sha256,
            network_identity_sha256,
            finalized_height,
            finalized_block_hash,
            entries: manifest_entries,
        },
        entries,
    })
}

fn validate_entry(
    checkpoint: SccpReplayArchiveSignedCheckpointV1,
    snapshot_bytes: Vec<u8>,
    policy: &SccpReplayArchiveReplicaPolicyV1,
    limits: SccpReplayArchiveDecodeLimitsV1,
) -> Result<ValidatedCheckpointEntryV1, ToriiSccpReplayStartupErrorV1> {
    verify_sccp_replay_archive_checkpoint_v1(policy, &checkpoint)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)?;
    let snapshot = decode_sccp_replay_archive_snapshot_v1(&snapshot_bytes, limits)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let content_sha256 = snapshot
        .content_sha256()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if checkpoint.body.snapshot_sha256 != content_sha256
        || checkpoint.body.accumulator_id != snapshot.accumulator_id
        || checkpoint.body.domain != snapshot.domain
        || checkpoint.body.finality != snapshot.finality
        || checkpoint.body.forest != snapshot.forest
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let checkpoint_agreement_digest = checkpoint
        .body
        .agreement_digest()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let checkpoint_bytes =
        norito::to_bytes(&checkpoint).map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if checkpoint_bytes.is_empty() || checkpoint_bytes.len() > MAX_PERSISTED_CHECKPOINT_BYTES_V1 {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let checkpoint_sha256 = sha256(&[&checkpoint_bytes]);
    Ok(ValidatedCheckpointEntryV1 {
        snapshot,
        snapshot_bytes,
        checkpoint,
        checkpoint_bytes,
        checkpoint_agreement_digest,
        checkpoint_sha256,
    })
}

fn validate_continuity(
    previous: Option<&PersistedReplayHeadStateV1>,
    entries: &[ValidatedCheckpointEntryV1],
    network_identity_sha256: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let Some(previous) = previous else {
        if entries
            .iter()
            .any(|entry| entry.snapshot.finality.predecessor_snapshot_sha256 != [0; 32])
        {
            return Err(ToriiSccpReplayStartupErrorV1::Continuity);
        }
        return Ok(());
    };
    if network_identity_sha256 != previous.manifest.network_identity_sha256
        || finalized_height <= previous.manifest.finalized_height
        || finalized_block_hash == previous.manifest.finalized_block_hash
    {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    let current = entries
        .iter()
        .map(|entry| (entry.snapshot.accumulator_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for prior in &previous.entries {
        let next = current
            .get(&prior.snapshot.accumulator_id)
            .ok_or(ToriiSccpReplayStartupErrorV1::Continuity)?;
        if next.snapshot.finality.predecessor_snapshot_sha256
            != prior.checkpoint.body.snapshot_sha256
            || next.snapshot.domain != prior.snapshot.domain
            || next.snapshot.forest.leaf_count < prior.snapshot.forest.leaf_count
            || next.snapshot.forest.update_sequence < prior.snapshot.forest.update_sequence
            || !snapshot_contains_prior_leaves(&prior.snapshot, &next.snapshot)
        {
            return Err(ToriiSccpReplayStartupErrorV1::Continuity);
        }
    }
    let old_ids = previous
        .entries
        .iter()
        .map(|entry| entry.snapshot.accumulator_id.clone())
        .collect::<BTreeSet<_>>();
    if entries.iter().any(|entry| {
        !old_ids.contains(&entry.snapshot.accumulator_id)
            && entry.snapshot.finality.predecessor_snapshot_sha256 != [0; 32]
    }) {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    Ok(())
}

fn snapshot_contains_prior_leaves(
    previous: &SccpReplayArchiveSnapshotV1,
    next: &SccpReplayArchiveSnapshotV1,
) -> bool {
    let mut next_leaves = next.leaves.iter().peekable();
    for prior in &previous.leaves {
        loop {
            match next_leaves.peek() {
                Some(candidate) if candidate.key < prior.key => {
                    next_leaves.next();
                }
                Some(candidate)
                    if candidate.key == prior.key
                        && candidate.record_digest == prior.record_digest =>
                {
                    next_leaves.next();
                    break;
                }
                Some(_) | None => return false,
            }
        }
    }
    true
}

struct SecureReplayStoreV1 {
    directory: File,
    _process_lock: File,
}

impl SecureReplayStoreV1 {
    fn open(path: &Path) -> Result<Self, ToriiSccpReplayStartupErrorV1> {
        let directory = open_secure_state_directory(path)?;
        let process_lock = open_and_lock_process_file(&directory)?;
        Ok(Self {
            directory,
            _process_lock: process_lock,
        })
    }

    fn manifest_limit(config: &ToriiSccpReplayArchive) -> usize {
        config
            .max_accumulators
            .checked_mul(512)
            .and_then(|bytes| bytes.checked_add(4_096))
            .unwrap_or(usize::MAX)
            .min(config.max_response_bytes)
    }

    fn load_head(
        &self,
        config: &ToriiSccpReplayArchive,
    ) -> Result<Option<PersistedReplayHeadStateV1>, ToriiSccpReplayStartupErrorV1> {
        let Some(bytes) = secure_read_relative(
            &self.directory,
            HEAD_MANIFEST_FILENAME_V1,
            Self::manifest_limit(config),
        )?
        else {
            return Ok(None);
        };
        let manifest: PersistedReplayHeadV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if norito::to_bytes(&manifest).ok().as_deref() != Some(bytes.as_slice())
            || manifest.version != HEAD_MANIFEST_VERSION_V1
            || manifest.checkpoint_set_sha256 == [0; 32]
            || manifest.network_identity_sha256 == [0; 32]
            || manifest.finalized_height == 0
            || manifest.finalized_block_hash == [0; 32]
            || manifest.entries.is_empty()
            || manifest.entries.len() > config.max_accumulators
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let policy = replica_policy(config);
        let limits = SccpReplayArchiveDecodeLimitsV1 {
            max_snapshot_bytes: config.max_snapshot_bytes,
            max_snapshot_leaves: config.max_snapshot_leaves,
        };
        let mut entries = Vec::with_capacity(manifest.entries.len());
        let mut wire_entries = Vec::with_capacity(manifest.entries.len());
        let mut previous_id = None;
        for head in &manifest.entries {
            if previous_id
                .as_ref()
                .is_some_and(|prior| prior >= &head.accumulator_id)
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            previous_id = Some(head.accumulator_id.clone());
            let snapshot_name = snapshot_filename(head.snapshot_sha256);
            let checkpoint_name = checkpoint_filename(head.checkpoint_sha256);
            let snapshot_bytes =
                secure_read_relative(&self.directory, &snapshot_name, config.max_snapshot_bytes)?
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            let checkpoint_bytes = secure_read_relative(
                &self.directory,
                &checkpoint_name,
                MAX_PERSISTED_CHECKPOINT_BYTES_V1,
            )?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            if sha256(&[&snapshot_bytes]) != head.snapshot_sha256
                || sha256(&[&checkpoint_bytes]) != head.checkpoint_sha256
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            let checkpoint: SccpReplayArchiveSignedCheckpointV1 =
                norito::decode_canonical_with_limits(
                    &checkpoint_bytes,
                    norito::canonical_decode_limits(checkpoint_bytes.len()),
                )
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if norito::to_bytes(&checkpoint).ok().as_deref() != Some(checkpoint_bytes.as_slice()) {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            let entry = validate_entry(checkpoint.clone(), snapshot_bytes.clone(), &policy, limits)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if entry.snapshot.accumulator_id != head.accumulator_id
                || entry.checkpoint_agreement_digest != head.checkpoint_agreement_digest
                || entry.checkpoint_sha256 != head.checkpoint_sha256
                || entry.checkpoint.body.snapshot_sha256 != head.snapshot_sha256
                || entry.snapshot.finality.network_identity_sha256
                    != manifest.network_identity_sha256
                || entry.snapshot.finality.finalized_height != manifest.finalized_height
                || entry.snapshot.finality.finalized_block_hash != manifest.finalized_block_hash
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            wire_entries.push(SccpReplayReplicaCheckpointEntryV1 {
                checkpoint,
                snapshot_bytes,
            });
            entries.push(entry);
        }
        let wire = SccpReplayReplicaCheckpointSetV1 {
            version: CHECKPOINT_SET_VERSION_V1,
            entries: wire_entries,
        };
        let wire_bytes =
            norito::to_bytes(&wire).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if sha256(&[CHECKPOINT_SET_DIGEST_DOMAIN_V1, &wire_bytes]) != manifest.checkpoint_set_sha256
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        Ok(Some(PersistedReplayHeadStateV1 { manifest, entries }))
    }

    fn persist_candidate(
        &self,
        config: &ToriiSccpReplayArchive,
        candidate: &CandidateReplayStateV1,
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        for entry in &candidate.entries {
            secure_write_immutable_relative(
                &self.directory,
                &snapshot_filename(entry.checkpoint.body.snapshot_sha256),
                &entry.snapshot_bytes,
                config.max_snapshot_bytes,
            )?;
            secure_write_immutable_relative(
                &self.directory,
                &checkpoint_filename(entry.checkpoint_sha256),
                &entry.checkpoint_bytes,
                MAX_PERSISTED_CHECKPOINT_BYTES_V1,
            )?;
        }
        let manifest_bytes = norito::to_bytes(&candidate.manifest)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        secure_write_manifest_last_relative(
            &self.directory,
            HEAD_MANIFEST_FILENAME_V1,
            &manifest_bytes,
            Self::manifest_limit(config),
        )
    }
}

fn snapshot_filename(digest: [u8; 32]) -> String {
    format!("snapshot-{}.norito", hex::encode(digest))
}

fn checkpoint_filename(digest: [u8; 32]) -> String {
    format!("checkpoint-{}.norito", hex::encode(digest))
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

#[cfg(unix)]
fn open_secure_state_directory(path: &Path) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;
    use std::path::Component;

    let mut components = path.components();
    if !matches!(components.next(), Some(Component::RootDir)) {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let components = components
        .map(|component| match component {
            Component::Normal(name) => Ok(name),
            Component::RootDir
            | Component::CurDir
            | Component::ParentDir
            | Component::Prefix(_) => Err(ToriiSccpReplayStartupErrorV1::Persistence),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if components.is_empty() {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }

    let mut current = File::from(
        rustix::fs::open(
            Path::new("/"),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
    );
    let effective_uid = rustix::process::geteuid().as_raw();
    for (index, name) in components.iter().enumerate() {
        let is_final = index + 1 == components.len();
        let created = if is_final {
            match rustix::fs::mkdirat(&current, *name, rustix::fs::Mode::RWXU) {
                Ok(()) => true,
                Err(rustix::io::Errno::EXIST) => false,
                Err(_) => return Err(ToriiSccpReplayStartupErrorV1::Persistence),
            }
        } else {
            false
        };
        let before = rustix::fs::statat(&current, *name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let child = File::from(
            rustix::fs::openat(
                &current,
                *name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
        );
        if created {
            rustix::fs::fchmod(&child, rustix::fs::Mode::RWXU)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            child
                .sync_all()
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            current
                .sync_all()
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        }
        let opened = child
            .metadata()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let after = rustix::fs::statat(&current, *name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if !opened.is_dir()
            || u64::try_from(before.st_dev).ok() != Some(opened.dev())
            || u64::try_from(before.st_ino).ok() != Some(opened.ino())
            || u64::try_from(after.st_dev).ok() != Some(opened.dev())
            || u64::try_from(after.st_ino).ok() != Some(opened.ino())
            || (is_final && (opened.uid() != effective_uid || opened.mode() & 0o777 != 0o700))
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        current = child;
    }
    Ok(current)
}

#[cfg(not(unix))]
fn open_secure_state_directory(_path: &Path) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn open_and_lock_process_file(directory: &File) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    let flags = rustix::fs::OFlags::RDWR
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::NONBLOCK
        | rustix::fs::OFlags::CLOEXEC;
    let (file, created) = match rustix::fs::openat(
        directory,
        PROCESS_LOCK_FILENAME_V1,
        flags | rustix::fs::OFlags::CREATE | rustix::fs::OFlags::EXCL,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    ) {
        Ok(file) => (File::from(file), true),
        Err(rustix::io::Errno::EXIST) => {
            let existing = rustix::fs::statat(
                directory,
                PROCESS_LOCK_FILENAME_V1,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if rustix::fs::FileType::from_raw_mode(existing.st_mode)
                != rustix::fs::FileType::RegularFile
                || existing.st_nlink != 1
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            (
                File::from(
                    rustix::fs::openat(
                        directory,
                        PROCESS_LOCK_FILENAME_V1,
                        flags,
                        rustix::fs::Mode::empty(),
                    )
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
                ),
                false,
            )
        }
        Err(_) => return Err(ToriiSccpReplayStartupErrorV1::Persistence),
    };
    if created {
        rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        file.sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    }
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let opened = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let named = rustix::fs::statat(
        directory,
        PROCESS_LOCK_FILENAME_V1,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o777 != 0o600
        || opened.nlink() != 1
        || rustix::fs::FileType::from_raw_mode(named.st_mode) != rustix::fs::FileType::RegularFile
        || u64::try_from(named.st_dev).ok() != Some(opened.dev())
        || u64::try_from(named.st_ino).ok() != Some(opened.ino())
        || named.st_nlink != 1
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_and_lock_process_file(_directory: &File) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn secure_read_relative(
    directory: &File,
    name: &str,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    if !secure_filename(name) || max_bytes == 0 {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let max_bytes_u64 =
        u64::try_from(max_bytes).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let read_limit = max_bytes_u64
        .checked_add(1)
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
    let before = match rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(ToriiSccpReplayStartupErrorV1::Persistence),
    };
    if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::RegularFile
        || before.st_nlink != 1
        || u64::try_from(before.st_size)
            .ok()
            .is_none_or(|size| size > max_bytes_u64)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let mut file = File::from(
        rustix::fs::openat(
            directory,
            name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
    );
    let opened = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o077 != 0
        || opened.nlink() != 1
        || u64::try_from(before.st_dev).ok() != Some(opened.dev())
        || u64::try_from(before.st_ino).ok() != Some(opened.ino())
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let opened_len =
        usize::try_from(opened.len()).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(opened_len)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    (&mut file)
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if bytes.len() > max_bytes {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let after = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if after.st_dev != before.st_dev
        || after.st_ino != before.st_ino
        || after.st_size != before.st_size
        || after.st_mtime != before.st_mtime
        || after.st_mtime_nsec != before.st_mtime_nsec
        || after.st_ctime != before.st_ctime
        || after.st_ctime_nsec != before.st_ctime_nsec
        || u64::try_from(bytes.len()).ok() != Some(opened.len())
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(Some(bytes))
}

#[cfg(not(unix))]
fn secure_read_relative(
    _directory: &File,
    _name: &str,
    _max_bytes: usize,
) -> Result<Option<Vec<u8>>, ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn secure_write_immutable_relative(
    directory: &File,
    name: &str,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    if bytes.is_empty() || bytes.len() > max_bytes || !secure_filename(name) {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    if let Some(existing) = secure_read_relative(directory, name, max_bytes)? {
        return (existing == bytes)
            .then_some(())
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let (mut temporary, temporary_name) = create_secure_temporary(directory, name)?;
    let publication = (|| {
        temporary
            .write_all(bytes)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        temporary
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        publish_noreplace(directory, &temporary_name, name)?;
        directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let readback = secure_read_relative(directory, name, max_bytes)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        (readback == bytes)
            .then_some(())
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)
    })();
    if publication.is_err() {
        let _ = rustix::fs::unlinkat(
            directory,
            temporary_name.as_str(),
            rustix::fs::AtFlags::empty(),
        );
    }
    publication
}

#[cfg(not(unix))]
fn secure_write_immutable_relative(
    _directory: &File,
    _name: &str,
    _bytes: &[u8],
    _max_bytes: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn secure_write_manifest_last_relative(
    directory: &File,
    name: &str,
    bytes: &[u8],
    max_bytes: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    if bytes.is_empty() || bytes.len() > max_bytes || !secure_filename(name) {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let (mut temporary, temporary_name) = create_secure_temporary(directory, name)?;
    let publication = (|| {
        temporary
            .write_all(bytes)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        temporary
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        rustix::fs::renameat(directory, temporary_name.as_str(), directory, name)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let readback = secure_read_relative(directory, name, max_bytes)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        (readback == bytes)
            .then_some(())
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)
    })();
    if publication.is_err() {
        let _ = rustix::fs::unlinkat(
            directory,
            temporary_name.as_str(),
            rustix::fs::AtFlags::empty(),
        );
    }
    publication
}

#[cfg(not(unix))]
fn secure_write_manifest_last_relative(
    _directory: &File,
    _name: &str,
    _bytes: &[u8],
    _max_bytes: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn create_secure_temporary(
    directory: &File,
    destination: &str,
) -> Result<(File, String), ToriiSccpReplayStartupErrorV1> {
    for _ in 0..SECURE_TEMP_RETRIES_V1 {
        let suffix: [u8; 16] = rand::random();
        let name = format!(".{destination}.{}.tmp", hex::encode(suffix));
        match rustix::fs::openat(
            directory,
            name.as_str(),
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        ) {
            Ok(file) => return Ok((File::from(file), name)),
            Err(rustix::io::Errno::EXIST) => continue,
            Err(_) => return Err(ToriiSccpReplayStartupErrorV1::Persistence),
        }
    }
    Err(ToriiSccpReplayStartupErrorV1::Persistence)
}

#[cfg(unix)]
fn publish_noreplace(
    directory: &File,
    source: &str,
    destination: &str,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    match rustix::fs::renameat_with(
        directory,
        source,
        directory,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.raw_os_error(),
                libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP
            ) =>
        {
            rustix::fs::linkat(
                directory,
                source,
                directory,
                destination,
                rustix::fs::AtFlags::empty(),
            )
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            rustix::fs::unlinkat(directory, source, rustix::fs::AtFlags::empty())
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)
        }
        Err(_) => Err(ToriiSccpReplayStartupErrorV1::Persistence),
    }
}

#[cfg(unix)]
fn secure_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 192
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::bridge::{
        SccpLaneIdV1, SccpNetworkV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayForestV1,
        SccpRouteKeyV1,
    };
    use iroha_sccp::{
        SccpReplayArchiveFinalityV1, SccpReplayArchiveReplicaAttestationV1,
        sccp_replay_archive_checkpoint_signing_message_v1,
    };
    use tempfile::TempDir;
    use url::Url;

    use super::*;

    #[derive(Default)]
    struct MutableSource {
        responses: Mutex<BTreeMap<[u8; 32], Vec<u8>>>,
    }

    impl MutableSource {
        fn set_all(&self, replicas: &[ToriiSccpReplayArchiveReplica; 3], bytes: &[u8]) {
            let mut responses = self.responses.lock().expect("source lock is healthy");
            responses.clear();
            for replica in replicas {
                responses.insert(replica.replica_id, bytes.to_vec());
            }
        }

        fn set_one(&self, replica_id: [u8; 32], bytes: Vec<u8>) {
            self.responses
                .lock()
                .expect("source lock is healthy")
                .insert(replica_id, bytes);
        }
    }

    impl SccpReplayCheckpointSourceV1 for MutableSource {
        fn fetch(
            &self,
            replica: &ToriiSccpReplayArchiveReplica,
            max_response_bytes: usize,
            _timeout: Duration,
        ) -> Result<Vec<u8>, SccpReplayCheckpointSourceErrorV1> {
            let bytes = self
                .responses
                .lock()
                .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?
                .get(&replica.replica_id)
                .cloned()
                .ok_or(SccpReplayCheckpointSourceErrorV1::Transport)?;
            if bytes.len() > max_response_bytes {
                return Err(SccpReplayCheckpointSourceErrorV1::Limit);
            }
            Ok(bytes)
        }
    }

    #[derive(Default)]
    struct EmptyForestLocalAuthority;

    impl SccpReplayLocalAuthorityV1 for EmptyForestLocalAuthority {
        fn rebuild_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            if finality.finalized_height == 0
                || expected.is_empty()
                || expected
                    .values()
                    .any(|(_, forest)| forest != &SccpReplayForestV1::default())
            {
                return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
            }
            let mut archive = SccpReplayArchiveV1::default();
            for (id, (domain, _)) in expected {
                archive
                    .initialize_accumulator(id.clone(), *domain)
                    .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)?;
            }
            Ok(archive)
        }
    }

    struct RejectingLocalAuthority;

    impl SccpReplayLocalAuthorityV1 for RejectingLocalAuthority {
        fn rebuild_and_verify(
            &self,
            _finality: SccpReplayArchiveFinalityV1,
            _expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch)
        }
    }

    struct Fixture {
        _temporary_root: TempDir,
        config: ToriiSccpReplayArchive,
        source: Arc<MutableSource>,
        key_pairs: [KeyPair; 3],
        accumulator_id: SccpReplayAccumulatorIdV1,
        domain: SccpReplayDomainV1,
        first_snapshot_sha256: [u8; 32],
        first_bytes: Vec<u8>,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary_root = tempfile::tempdir().expect("temporary directory is created");
            let canonical_root = temporary_root
                .path()
                .canonicalize()
                .expect("temporary root canonicalizes");
            let key_pairs = [
                KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519),
                KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519),
                KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519),
            ];
            let replicas = core::array::from_fn(|index| {
                let (algorithm, public_key) = key_pairs[index].public_key().to_bytes();
                assert_eq!(algorithm, Algorithm::Ed25519);
                ToriiSccpReplayArchiveReplica {
                    replica_id: [u8::try_from(index + 1).expect("small replica index"); 32],
                    origin: Url::parse(&format!("https://replay-{}.example/", index + 1))
                        .expect("canonical HTTPS origin"),
                    ed25519_public_key: public_key
                        .try_into()
                        .expect("Ed25519 public keys are 32 bytes"),
                }
            });
            let config = ToriiSccpReplayArchive {
                state_dir: canonical_root.join("sccp-replay"),
                replicas,
                max_response_bytes: 1024 * 1024,
                max_snapshot_bytes: 512 * 1024,
                max_snapshot_leaves: 1024,
                max_accumulators: 16,
                request_timeout: Duration::from_secs(1),
            };
            let accumulator_id = accumulator_id();
            let domain = domain();
            let first_snapshot = snapshot(
                &accumulator_id,
                domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: 1,
                    finalized_block_hash: [0x41; 32],
                    predecessor_snapshot_sha256: [0; 32],
                },
            );
            let first_snapshot_sha256 = first_snapshot
                .content_sha256()
                .expect("snapshot content hash is defined");
            let first_bytes = checkpoint_set_bytes(&first_snapshot, &key_pairs, &replicas);
            let source = Arc::new(MutableSource::default());
            source.set_all(&replicas, &first_bytes);
            Self {
                _temporary_root: temporary_root,
                config,
                source,
                key_pairs,
                accumulator_id,
                domain,
                first_snapshot_sha256,
                first_bytes,
            }
        }

        fn bytes_at(
            &self,
            height: u64,
            block_hash: [u8; 32],
            predecessor_snapshot_sha256: [u8; 32],
        ) -> (Vec<u8>, [u8; 32]) {
            self.bytes_at_with_domain(height, block_hash, predecessor_snapshot_sha256, self.domain)
        }

        fn bytes_at_with_domain(
            &self,
            height: u64,
            block_hash: [u8; 32],
            predecessor_snapshot_sha256: [u8; 32],
            domain: SccpReplayDomainV1,
        ) -> (Vec<u8>, [u8; 32]) {
            let snapshot = snapshot(
                &self.accumulator_id,
                domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: height,
                    finalized_block_hash: block_hash,
                    predecessor_snapshot_sha256,
                },
            );
            let digest = snapshot
                .content_sha256()
                .expect("snapshot content hash is defined");
            (
                checkpoint_set_bytes(&snapshot, &self.key_pairs, &self.config.replicas),
                digest,
            )
        }

        fn bootstrap(
            &self,
        ) -> Result<Arc<ToriiSccpReplayArchiveServiceV1>, ToriiSccpReplayStartupErrorV1> {
            ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
                self.config.clone(),
                self.source.clone(),
                Arc::new(EmptyForestLocalAuthority),
            )
        }
    }

    fn accumulator_id() -> SccpReplayAccumulatorIdV1 {
        SccpReplayAccumulatorIdV1 {
            route_key: SccpRouteKeyV1::new(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                "taira_eth_xor".to_owned(),
                "xor".to_owned(),
                7,
            )
            .expect("valid final-V1 route key"),
            boundary: SccpReplayBoundaryV1::SoraOutboundLock,
        }
    }

    fn domain() -> SccpReplayDomainV1 {
        SccpReplayDomainV1 {
            source_network: SccpNetworkV1::SoraTaira,
            target_network: SccpNetworkV1::EthereumMainnet,
            boundary: SccpReplayBoundaryV1::SoraOutboundLock,
            route_revision: 7,
            route_configuration_hash: [0x44; 32],
            actor: SccpReplayActorV1::Route,
        }
    }

    fn snapshot(
        accumulator_id: &SccpReplayAccumulatorIdV1,
        domain: SccpReplayDomainV1,
        finality: SccpReplayArchiveFinalityV1,
    ) -> SccpReplayArchiveSnapshotV1 {
        SccpReplayArchiveSnapshotV1 {
            version: 1,
            accumulator_id: accumulator_id.clone(),
            domain,
            finality,
            forest: SccpReplayForestV1::default(),
            leaves: Vec::new(),
        }
    }

    fn checkpoint_set_bytes(
        snapshot: &SccpReplayArchiveSnapshotV1,
        key_pairs: &[KeyPair; 3],
        replicas: &[ToriiSccpReplayArchiveReplica; 3],
    ) -> Vec<u8> {
        let body = SccpReplayArchiveCheckpointBodyV1::from_snapshot(snapshot)
            .expect("valid snapshot produces a checkpoint");
        let message = sccp_replay_archive_checkpoint_signing_message_v1(&body)
            .expect("checkpoint signing message is defined");
        let attestations = core::array::from_fn(|index| {
            let signature = Signature::try_new(key_pairs[index].private_key(), &message)
                .expect("fixture checkpoint signs");
            SccpReplayArchiveReplicaAttestationV1 {
                replica_id: replicas[index].replica_id,
                signature: signature
                    .payload()
                    .try_into()
                    .expect("Ed25519 signatures are 64 bytes"),
            }
        });
        norito::to_bytes(&SccpReplayReplicaCheckpointSetV1 {
            version: CHECKPOINT_SET_VERSION_V1,
            entries: vec![SccpReplayReplicaCheckpointEntryV1 {
                checkpoint: SccpReplayArchiveSignedCheckpointV1 { body, attestations },
                snapshot_bytes: norito::to_bytes(snapshot).expect("snapshot encodes"),
            }],
        })
        .expect("checkpoint set encodes")
    }

    #[test]
    fn bootstrap_persists_manifest_last_and_serves_verified_empty_witness() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        let (served_domain, forest) = service
            .forest(&fixture.accumulator_id)
            .expect("verified forest is served");
        assert_eq!(served_domain, fixture.domain);
        assert_eq!(forest, SccpReplayForestV1::default());
        let witness = service
            .witness(&fixture.accumulator_id, [0; 32])
            .expect("zero replay key receives a canonical witness");
        forest
            .verify_key_digest([0; 32], [0; 32], &witness)
            .expect("served non-membership witness verifies");
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("authenticated checkpoint is served")
                .body
                .snapshot_sha256,
            fixture.first_snapshot_sha256
        );

        let directory = fs::metadata(&fixture.config.state_dir).expect("state dir exists");
        assert_eq!(directory.permissions().mode() & 0o777, 0o700);
        let head = fixture.config.state_dir.join(HEAD_MANIFEST_FILENAME_V1);
        assert!(head.is_file());
        for entry in fs::read_dir(&fixture.config.state_dir).expect("state dir is readable") {
            let metadata = entry
                .expect("valid directory entry")
                .metadata()
                .expect("entry metadata is readable");
            assert!(metadata.is_file());
            assert_eq!(metadata.permissions().mode() & 0o077, 0);
        }
    }

    #[test]
    fn bootstrap_rejects_replica_disagreement_before_manifest_publication() {
        let fixture = Fixture::new();
        let mut divergent = fixture.first_bytes.clone();
        divergent.push(0);
        fixture
            .source
            .set_one(fixture.config.replicas[2].replica_id, divergent);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement)
        );
        assert!(
            !fixture
                .config
                .state_dir
                .join(HEAD_MANIFEST_FILENAME_V1)
                .exists()
        );
    }

    #[test]
    fn bootstrap_rejects_a_canonically_encoded_forged_attestation() {
        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        set.entries[0].checkpoint.attestations[1].signature[0] ^= 1;
        let forged = norito::to_bytes(&set).expect("forged set remains canonical");
        fixture.source.set_all(&fixture.config.replicas, &forged);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)
        );
    }

    #[test]
    fn refresh_accepts_only_strict_successors_and_rejects_cached_or_forked_heads() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (successor, successor_hash) =
            fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service.refresh().expect("strict successor refreshes");
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "a cached head is never re-accepted"
        );

        let (same_height_fork, _) = fixture.bytes_at(2, [0x52; 32], successor_hash);
        fixture
            .source
            .set_all(&fixture.config.replicas, &same_height_fork);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "an equal-height different block is a fork"
        );
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("failed refresh retains the authenticated checkpoint")
                .body
                .snapshot_sha256,
            successor_hash,
            "a rejected fork cannot replace visible replay state"
        );

        let (broken_predecessor, _) = fixture.bytes_at(3, [0x43; 32], [0; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &broken_predecessor);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity)
        );

        let mut substituted_domain = fixture.domain;
        substituted_domain.route_configuration_hash[0] ^= 1;
        let (substituted_domain_head, _) =
            fixture.bytes_at_with_domain(3, [0x43; 32], successor_hash, substituted_domain);
        fixture
            .source
            .set_all(&fixture.config.replicas, &substituted_domain_head);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "an accumulator domain is immutable across signed successors"
        );
    }

    #[test]
    fn process_lock_and_persisted_head_both_reject_duplicate_bootstrap() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "a second writer cannot race the retained manifest"
        );
        drop(service);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "a restart cannot re-accept the cached signed head"
        );
    }

    #[test]
    fn startup_rejects_insecure_or_symlinked_state_directories() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let fixture = Fixture::new();
        fs::create_dir(&fixture.config.state_dir).expect("state dir is created");
        fs::set_permissions(&fixture.config.state_dir, fs::Permissions::from_mode(0o755))
            .expect("permissions change");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence)
        );

        fs::remove_dir(&fixture.config.state_dir).expect("insecure empty directory is removed");
        let target = fixture
            .config
            .state_dir
            .parent()
            .expect("state path has parent")
            .join("target");
        fs::create_dir(&target).expect("symlink target is created");
        symlink(&target, &fixture.config.state_dir).expect("state symlink is created");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence)
        );
    }

    #[test]
    fn bootstrap_revalidates_actual_replica_independence() {
        let mut fixture = Fixture::new();
        fixture.config.replicas[1].origin = fixture.config.replicas[0].origin.clone();
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Malformed)
        );
    }

    #[test]
    fn bootstrap_fails_before_publication_when_local_core_or_kura_disagrees() {
        let fixture = Fixture::new();
        assert_eq!(
            ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
                fixture.config.clone(),
                fixture.source.clone(),
                Arc::new(RejectingLocalAuthority),
            )
            .map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::LocalAuthority)
        );
        assert!(
            !fixture
                .config
                .state_dir
                .join(HEAD_MANIFEST_FILENAME_V1)
                .exists()
        );
    }

    #[test]
    fn startup_never_follows_a_substituted_lock_file() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let fixture = Fixture::new();
        fs::create_dir(&fixture.config.state_dir).expect("private state dir is created");
        fs::set_permissions(&fixture.config.state_dir, fs::Permissions::from_mode(0o700))
            .expect("private state permissions are installed");
        let target = fixture
            .config
            .state_dir
            .parent()
            .expect("state path has parent")
            .join("substituted-lock-target");
        fs::write(&target, b"not a process lock").expect("substitute target is created");
        symlink(
            &target,
            fixture.config.state_dir.join(PROCESS_LOCK_FILENAME_V1),
        )
        .expect("lock symlink is created");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence)
        );
    }
}
