//! Fail-closed Torii bootstrap and read provider for SCCP replay archives.
//!
//! Archive replicas are availability services, not consensus authorities.
//! Torii accepts a checkpoint only when all three configured HTTPS origins
//! return byte-identical canonical data, every pinned Ed25519 signature
//! verifies, every snapshot rebuilds, predecessor continuity holds, and a
//! fresh Kura scan reproduces the complete Core replay-forest projection.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read as _, Write as _},
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use base64::Engine as _;
use iroha_config::parameters::actual::{ToriiSccpReplayArchive, ToriiSccpReplayArchiveReplica};
use iroha_core::{
    bridge::rebuild_sccp_replay_archive_from_kura_v1,
    kura::Kura,
    state::{State as CoreState, WorldReadOnly as _},
};
use iroha_data_model::bridge::{
    SccpLaneIdV1, SccpNetworkV1, SccpReplayAccumulatorIdV1, SccpReplayActorV1,
    SccpReplayBoundaryV1, SccpReplayDomainV1, SccpReplayForestV1, SccpRouteKeyV1,
    SccpSparseMerkleWitnessV1, sccp_replay_domain_hash_v1, sccp_replay_key_v1,
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
/// Minimum delay between failed or otherwise unnecessary replica refresh attempts.
pub const SCCP_REPLAY_REFRESH_RETRY_MINIMUM_V1: Duration = Duration::from_secs(30);

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

/// Strictly accumulator-id-ordered complete V1 replay inventory returned by every replica.
///
/// The top-level sequence shape permits the configured accumulator count to
/// be rejected before offset tables or entry storage are allocated. The fixed
/// V1 endpoint, Norito schema hash, and signed checkpoint-body version provide
/// the wire-version binding without a redundant unsigned envelope field.
pub type SccpReplayReplicaCheckpointSetV1 = Vec<SccpReplayReplicaCheckpointEntryV1>;

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
    /// The supplied checkpoint coordinate is not committed by local Kura.
    Finality,
    /// The signed forest set differs from current Core state.
    CoreMismatch,
    /// Kura execution could not reproduce the complete forest set.
    Rebuild,
}

/// Narrow boundary that proves a remote forest inventory against local Core
/// state and commit-authenticated Kura execution.
pub trait SccpReplayLocalAuthorityV1: Send + Sync {
    /// Confirm that the complete published replay inventory still equals current Core state.
    fn validate_current_inventory(
        &self,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1>;

    /// Return the Kura-rebuilt archive only for the exact current committed coordinate.
    ///
    /// A published archive may remain readable after unrelated blocks while
    /// [`Self::validate_current_inventory`] continues to succeed. Requiring an
    /// exact coordinate here prevents a newly fetched checkpoint from claiming
    /// that a route created later already existed at an older height.
    fn rebuild_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1>;

    /// Rebuild a previously admitted, securely persisted checkpoint.
    ///
    /// Unlike a new remote candidate, this checkpoint may precede the current
    /// global tip. Implementations must still authenticate its exact Kura block
    /// and require its complete replay inventory to equal current Core state.
    fn rebuild_persisted_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        self.rebuild_and_verify(finality, expected)
    }
}

struct CoreKuraSccpReplayLocalAuthorityV1 {
    state: Arc<CoreState>,
    kura: Arc<Kura>,
}

impl CoreKuraSccpReplayLocalAuthorityV1 {
    fn rebuild_at_checkpoint(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
        require_current_coordinate: bool,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        let committed_height = self.state.committed_height();
        let checkpoint_height = usize::try_from(finality.finalized_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .filter(|height| {
                if require_current_coordinate {
                    height.get() == committed_height
                } else {
                    height.get() <= committed_height
                }
            })
            .ok_or(SccpReplayLocalAuthorityErrorV1::Finality)?;
        if finality.network_identity_sha256
            != sccp_replay_archive_network_identity_sha256_v1(self.state.network_id_ref())
            || self
                .kura
                .get_block_hash(checkpoint_height)
                .map(|hash| *hash.as_ref())
                != Some(finality.finalized_block_hash)
        {
            return Err(SccpReplayLocalAuthorityErrorV1::Finality);
        }
        self.validate_current_inventory(expected)?;

        rebuild_sccp_replay_archive_from_kura_v1(&self.kura, checkpoint_height, expected)
            .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)
    }
}

impl SccpReplayLocalAuthorityV1 for CoreKuraSccpReplayLocalAuthorityV1 {
    fn validate_current_inventory(
        &self,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        (&authoritative_core_replay_inventory(&self.state)? == expected)
            .then_some(())
            .ok_or(SccpReplayLocalAuthorityErrorV1::CoreMismatch)
    }

    fn rebuild_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        self.rebuild_at_checkpoint(finality, expected, true)
    }

    fn rebuild_persisted_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        self.rebuild_at_checkpoint(finality, expected, false)
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
            let domain = SccpReplayDomainV1 {
                source_network,
                target_network,
                boundary,
                route_revision: route.revision,
                route_configuration_hash,
                actor: SccpReplayActorV1::Route,
            };
            let accumulator_id = SccpReplayAccumulatorIdV1::from_domain(route_key.clone(), &domain)
                .map_err(|_| SccpReplayLocalAuthorityErrorV1::CoreMismatch)?;
            let forest = world
                .sccp_replay_forests()
                .get(&accumulator_id)
                .cloned()
                .unwrap_or_default();
            if authoritative
                .insert(accumulator_id, (domain, forest))
                .is_some()
            {
                return Err(SccpReplayLocalAuthorityErrorV1::Finality);
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
    /// Checkpoint-set or snapshot framing was malformed.
    Malformed,
    /// A bounded response or decode budget was exceeded.
    ResourceLimit,
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
            Self::ResourceLimit => "SCCP replay checkpoint resource limit exceeded",
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
    /// Route coordinates, boundary, or replay identifier are not canonical V1 input.
    InvalidRequest,
    /// Replay archive service is disabled by configuration.
    Disabled,
    /// The exact accumulator or key is not retained.
    NotFound,
    /// The requested replay identifier is already occupied.
    Occupied,
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
            Self::InvalidRequest => "sccp_replay_invalid_request",
            Self::Disabled => "sccp_replay_disabled",
            Self::NotFound => "sccp_replay_not_found",
            Self::Occupied => "sccp_replay_occupied",
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

/// Atomic, locally current SCCP replay non-membership acquisition response.
#[derive(Debug, Clone, PartialEq, Eq, norito::JsonSerialize, norito::NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub struct ToriiSccpReplayWitnessResponseV1 {
    /// Response schema version. First release accepts exactly `1`.
    pub version: u8,
    /// Exact governed route, boundary, and committed replay-domain hash.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Complete replay domain whose hash is committed by `accumulator_id`.
    pub domain: SccpReplayDomainV1,
    /// Lowercase hexadecimal form of the committed domain hash.
    pub domain_hash_hex: String,
    /// Lowercase hexadecimal sparse-Merkle key derived from the replay identifier.
    pub replay_key_hex: String,
    /// Current consensus forest authenticated by the returned checkpoint.
    pub forest: SccpReplayForestV1,
    /// Canonical padded base64 of one canonical Norito non-membership witness.
    pub replay_witness_b64: String,
    /// Canonical padded base64 of the exactly-three-signature checkpoint.
    pub checkpoint_b64: String,
    /// Lowercase hexadecimal digest of the exact agreed checkpoint-set response.
    pub checkpoint_set_sha256_hex: String,
    /// Finalized height at which the returned replay inventory was checkpointed.
    pub checkpoint_height: u64,
    /// Lowercase hexadecimal hash of the checkpointed finalized block.
    pub checkpoint_block_hash_hex: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReplayFinalityCoordinateV1 {
    network_identity_sha256: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
}

impl From<iroha_sccp::SccpReplayArchiveFinalityV1> for ReplayFinalityCoordinateV1 {
    fn from(finality: iroha_sccp::SccpReplayArchiveFinalityV1) -> Self {
        Self {
            network_identity_sha256: finality.network_identity_sha256,
            finalized_height: finality.finalized_height,
            finalized_block_hash: finality.finalized_block_hash,
        }
    }
}

struct PublishedReplayStateV1 {
    archive: SccpReplayArchiveV1,
    inventory: BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    checkpoints: BTreeMap<SccpReplayAccumulatorIdV1, SccpReplayArchiveSignedCheckpointV1>,
    checkpoint_set_sha256: [u8; 32],
    finality: ReplayFinalityCoordinateV1,
}

struct CandidateReplayStateV1 {
    published: PublishedReplayStateV1,
    manifest: PersistedReplayHeadV1,
    entries: Vec<ValidatedCheckpointEntryV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestPublicationFailureV1 {
    BeforeCommit,
    AfterRename,
}

fn restore_persisted_if_current(
    persisted: &PersistedReplayHeadStateV1,
    local_authority: &dyn SccpReplayLocalAuthorityV1,
) -> Result<Option<PublishedReplayStateV1>, ToriiSccpReplayStartupErrorV1> {
    let mut inventory = BTreeMap::new();
    let mut checkpoints = BTreeMap::new();
    for entry in &persisted.entries {
        let accumulator_id = entry.snapshot.accumulator_id.clone();
        if inventory
            .insert(
                accumulator_id.clone(),
                (entry.snapshot.domain, entry.snapshot.forest.clone()),
            )
            .is_some()
            || checkpoints
                .insert(accumulator_id, entry.checkpoint.clone())
                .is_some()
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
    }
    if local_authority
        .validate_current_inventory(&inventory)
        .is_err()
    {
        return Ok(None);
    }
    let finality = persisted
        .entries
        .first()
        .map(|entry| entry.snapshot.finality)
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
    let Ok(archive) = local_authority.rebuild_persisted_and_verify(finality, &inventory) else {
        return Ok(None);
    };
    Ok(Some(PublishedReplayStateV1 {
        archive,
        inventory,
        checkpoints,
        checkpoint_set_sha256: persisted.manifest.checkpoint_set_sha256,
        finality: finality.into(),
    }))
}

/// Live Torii provider. The visible state changes only after every immutable
/// artifact and the manifest-last head are durably published.
pub struct ToriiSccpReplayArchiveServiceV1 {
    config: ToriiSccpReplayArchive,
    source: Arc<dyn SccpReplayCheckpointSourceV1>,
    local_authority: Arc<dyn SccpReplayLocalAuthorityV1>,
    store: SecureReplayStoreV1,
    update_lock: Mutex<()>,
    available: AtomicBool,
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
        let source = Arc::new(
            HttpsSccpReplayCheckpointSourceV1::new(config.request_timeout)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?,
        );
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
        store.recover(&config)?;
        let previous = store.load_head(&config)?;
        store.prune_to_manifest(&config, previous.as_ref().map(|head| &head.manifest))?;
        if let Some(published) = previous
            .as_ref()
            .map(|persisted| restore_persisted_if_current(persisted, local_authority.as_ref()))
            .transpose()?
            .flatten()
        {
            return Ok(Arc::new(Self {
                config,
                source,
                local_authority,
                store,
                update_lock: Mutex::new(()),
                available: AtomicBool::new(true),
                published: RwLock::new(published),
            }));
        }
        let bytes = fetch_exact_three(&config, source.as_ref())?;
        let candidate =
            validate_candidate(&config, &bytes, previous.as_ref(), local_authority.as_ref())?;
        store.write_candidate_artifacts(&config, &candidate)?;
        local_authority
            .validate_current_inventory(&candidate.published.inventory)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
        match store.publish_candidate_manifest(&config, &candidate) {
            Ok(()) => {}
            Err(ManifestPublicationFailureV1::BeforeCommit) => {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            Err(ManifestPublicationFailureV1::AfterRename) => {
                store.confirm_candidate_manifest(&config, &candidate)?;
            }
        }
        store.prune_to_manifest(&config, Some(&candidate.manifest))?;
        Ok(Arc::new(Self {
            config,
            source,
            local_authority,
            store,
            update_lock: Mutex::new(()),
            available: AtomicBool::new(true),
            published: RwLock::new(candidate.published),
        }))
    }

    /// Fetch and atomically publish the current or one strictly newer three-replica head.
    pub fn refresh(&self) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let previous = self
            .store
            .load_head(&self.config)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        let published_digest = self
            .published
            .read()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
            .checkpoint_set_sha256;
        if published_digest != previous.manifest.checkpoint_set_sha256 {
            self.available.store(false, Ordering::Release);
            if let Some(restored) =
                restore_persisted_if_current(&previous, self.local_authority.as_ref())?
            {
                *self
                    .published
                    .write()
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)? = restored;
                self.available.store(true, Ordering::Release);
            }
        }
        let visible_digest_before_refresh = self
            .published
            .read()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
            .checkpoint_set_sha256;
        self.store
            .prune_to_manifest(&self.config, Some(&previous.manifest))?;
        let bytes = fetch_exact_three(&self.config, self.source.as_ref())?;
        let candidate = validate_candidate(
            &self.config,
            &bytes,
            Some(&previous),
            self.local_authority.as_ref(),
        )?;
        self.store
            .write_candidate_artifacts(&self.config, &candidate)?;
        self.local_authority
            .validate_current_inventory(&candidate.published.inventory)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
        match self
            .store
            .publish_candidate_manifest(&self.config, &candidate)
        {
            Ok(()) => {}
            Err(ManifestPublicationFailureV1::BeforeCommit) => {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            Err(ManifestPublicationFailureV1::AfterRename) => {
                if self
                    .store
                    .confirm_candidate_manifest(&self.config, &candidate)
                    .is_err()
                {
                    self.available.store(false, Ordering::Release);
                    return Err(ToriiSccpReplayStartupErrorV1::Persistence);
                }
            }
        }
        let mut published = self
            .published
            .write()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if published.checkpoint_set_sha256 != visible_digest_before_refresh {
            self.available.store(false, Ordering::Release);
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        *published = candidate.published;
        self.available.store(true, Ordering::Release);
        drop(published);
        if self
            .store
            .prune_to_manifest(&self.config, Some(&candidate.manifest))
            .is_err()
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        Ok(())
    }

    /// Refresh only when the published replay inventory no longer matches Core.
    ///
    /// Returns `true` when a new head was fetched and atomically published.
    pub fn refresh_if_stale(&self) -> Result<bool, ToriiSccpReplayStartupErrorV1> {
        let published = self
            .published
            .read()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if self.available.load(Ordering::Acquire)
            && self
                .local_authority
                .validate_current_inventory(&published.inventory)
                .is_ok()
        {
            return Ok(false);
        }
        drop(published);
        self.refresh()?;
        Ok(true)
    }

    /// Return the minimum period used by the background stale-head retry loop.
    #[must_use]
    pub const fn refresh_retry_interval() -> Duration {
        SCCP_REPLAY_REFRESH_RETRY_MINIMUM_V1
    }

    /// Atomically read one locally current, authenticated replay non-membership bundle.
    pub fn read_non_membership_witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<ToriiSccpReplayWitnessResponseV1, ToriiSccpReplayEndpointErrorV1> {
        let published = self
            .current_published()
            .map_err(ToriiSccpReplayEndpointErrorV1::from)?;

        let (domain, forest) =
            SccpReplayArchiveProviderV1::forest(&published.archive, accumulator_id)?;
        let domain_hash = sccp_replay_domain_hash_v1(&domain)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        if accumulator_id.domain_hash != domain_hash
            || accumulator_id.validate_domain(&domain).is_err()
        {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }

        let witness =
            SccpReplayArchiveProviderV1::witness(&published.archive, accumulator_id, key)?;
        if witness.prior_record_digest != [0; 32] {
            return Err(ToriiSccpReplayEndpointErrorV1::Occupied);
        }
        forest
            .verify_key_digest(key, [0; 32], &witness)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;

        let checkpoint = published
            .checkpoints
            .get(accumulator_id)
            .ok_or(ToriiSccpReplayEndpointErrorV1::NotFound)?;
        let authenticated =
            verify_sccp_replay_archive_checkpoint_v1(&replica_policy(&self.config), checkpoint)
                .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        if &authenticated.accumulator_id != accumulator_id
            || authenticated.domain != domain
            || authenticated.forest != forest
            || ReplayFinalityCoordinateV1::from(authenticated.finality) != published.finality
            || authenticated.snapshot_sha256 == [0; 32]
        {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }

        let replay_witness = norito::encode_canonical(&witness)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        let checkpoint = norito::encode_canonical(checkpoint)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        Ok(ToriiSccpReplayWitnessResponseV1 {
            version: 1,
            accumulator_id: accumulator_id.clone(),
            domain,
            domain_hash_hex: hex::encode(domain_hash),
            replay_key_hex: hex::encode(key),
            forest,
            replay_witness_b64: base64::engine::general_purpose::STANDARD.encode(replay_witness),
            checkpoint_b64: base64::engine::general_purpose::STANDARD.encode(checkpoint),
            checkpoint_set_sha256_hex: hex::encode(published.checkpoint_set_sha256),
            checkpoint_height: published.finality.finalized_height,
            checkpoint_block_hash_hex: hex::encode(published.finality.finalized_block_hash),
        })
    }

    /// Digest of the exact currently visible three-replica checkpoint set.
    pub fn checkpoint_set_sha256(&self) -> Result<[u8; 32], ToriiSccpReplayEndpointErrorV1> {
        self.current_published()
            .map(|state| state.checkpoint_set_sha256)
            .map_err(ToriiSccpReplayEndpointErrorV1::from)
    }

    fn current_published(
        &self,
    ) -> Result<
        std::sync::RwLockReadGuard<'_, PublishedReplayStateV1>,
        SccpReplayArchiveProviderErrorV1,
    > {
        let published = self
            .published
            .read()
            .map_err(|_| SccpReplayArchiveProviderErrorV1::Integrity)?;
        if !self.available.load(Ordering::Acquire)
            || published.checkpoint_set_sha256 == [0; 32]
            || self
                .local_authority
                .validate_current_inventory(&published.inventory)
                .is_err()
        {
            return Err(SccpReplayArchiveProviderErrorV1::Unavailable);
        }
        Ok(published)
    }
}

/// Resolve canonical public route coordinates and a replay identifier into the
/// exact archive key consumed by [`ToriiSccpReplayArchiveServiceV1`].
pub fn resolve_sora_replay_witness_request_v1(
    state: &CoreState,
    source_profile: &str,
    route_id: &str,
    asset_key: &str,
    revision: u32,
    boundary: &str,
    replay_id_hex: &str,
) -> Result<(SccpReplayAccumulatorIdV1, [u8; 32]), ToriiSccpReplayEndpointErrorV1> {
    let source = SccpNetworkV1::from_profile_key(source_profile)
        .filter(|network| network.is_external())
        .ok_or(ToriiSccpReplayEndpointErrorV1::InvalidRequest)?;
    let route_key = SccpRouteKeyV1::new(
        SccpLaneIdV1 {
            source,
            target: SccpNetworkV1::SoraTaira,
        },
        route_id.to_owned(),
        asset_key.to_owned(),
        revision,
    )
    .map_err(|_| ToriiSccpReplayEndpointErrorV1::InvalidRequest)?;
    let registry = state.sccp_registry_snapshot();
    let route = registry
        .route(&route_key)
        .ok_or(ToriiSccpReplayEndpointErrorV1::NotFound)?;
    let route_configuration_hash = route
        .route_configuration_hash()
        .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
    let (boundary, source_network, target_network) = match boundary {
        "sora-outbound-lock" => (
            SccpReplayBoundaryV1::SoraOutboundLock,
            SccpNetworkV1::SoraTaira,
            route.lane_id.source,
        ),
        "sora-inbound-release" => (
            SccpReplayBoundaryV1::SoraInboundRelease,
            route.lane_id.source,
            route.lane_id.target,
        ),
        _ => return Err(ToriiSccpReplayEndpointErrorV1::InvalidRequest),
    };
    let domain = SccpReplayDomainV1 {
        source_network,
        target_network,
        boundary,
        route_revision: route.revision,
        route_configuration_hash,
        actor: SccpReplayActorV1::Route,
    };
    let accumulator_id = SccpReplayAccumulatorIdV1::from_domain(route_key, &domain)
        .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;

    if replay_id_hex.len() != 64
        || !replay_id_hex
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(ToriiSccpReplayEndpointErrorV1::InvalidRequest);
    }
    let mut replay_id = [0_u8; 32];
    hex::decode_to_slice(replay_id_hex, &mut replay_id)
        .map_err(|_| ToriiSccpReplayEndpointErrorV1::InvalidRequest)?;
    if replay_id == [0; 32] {
        return Err(ToriiSccpReplayEndpointErrorV1::InvalidRequest);
    }
    let key = sccp_replay_key_v1(accumulator_id.domain_hash, replay_id);
    Ok((accumulator_id, key))
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
        let published = self.current_published()?;
        SccpReplayArchiveProviderV1::forest(&published.archive, accumulator_id)
    }

    fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveProviderErrorV1> {
        let published = self.current_published()?;
        SccpReplayArchiveProviderV1::witness(&published.archive, accumulator_id, key)
    }

    fn checkpoint(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveProviderErrorV1> {
        self.current_published()?
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
    let responses = std::thread::scope(|scope| {
        let handles = config.replicas.each_ref().map(|replica| {
            scope.spawn(move || {
                source.fetch(replica, config.max_response_bytes, config.request_timeout)
            })
        });
        handles.map(|handle| handle.join())
    });
    let mut agreed: Option<Vec<u8>> = None;
    for response in responses {
        let bytes = response
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?
            .map_err(|error| match error {
                SccpReplayCheckpointSourceErrorV1::Limit => {
                    ToriiSccpReplayStartupErrorV1::ResourceLimit
                }
                SccpReplayCheckpointSourceErrorV1::Transport
                | SccpReplayCheckpointSourceErrorV1::Protocol => {
                    ToriiSccpReplayStartupErrorV1::Transport
                }
            })?;
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
    if bytes.is_empty() {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    if bytes.len() > config.max_response_bytes {
        return Err(ToriiSccpReplayStartupErrorV1::ResourceLimit);
    }
    norito::inspect_stream_vec_len_bounded_from_reader::<_, SccpReplayReplicaCheckpointEntryV1>(
        bytes,
        config.max_accumulators,
    )
    .map_err(|error| {
        if error.is_decode_resource_limit() {
            ToriiSccpReplayStartupErrorV1::ResourceLimit
        } else {
            ToriiSccpReplayStartupErrorV1::Malformed
        }
    })?;
    let set: SccpReplayReplicaCheckpointSetV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|error| {
            if error.is_decode_resource_limit() {
                ToriiSccpReplayStartupErrorV1::ResourceLimit
            } else {
                ToriiSccpReplayStartupErrorV1::Malformed
            }
        })?;
    if set.is_empty() {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let policy = replica_policy(config);
    let limits = SccpReplayArchiveDecodeLimitsV1 {
        max_snapshot_bytes: config.max_snapshot_bytes,
        max_snapshot_leaves: config.max_snapshot_leaves,
    };
    let mut entries = Vec::with_capacity(set.len());
    let mut expected = BTreeMap::new();
    let mut checkpoints = BTreeMap::new();
    let mut previous_id = None;
    let mut common_finality = None;
    for entry in set {
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
    let checkpoint_set_sha256 = sha256(&[CHECKPOINT_SET_DIGEST_DOMAIN_V1, bytes]);
    validate_continuity(
        previous,
        &entries,
        checkpoint_set_sha256,
        network_identity_sha256,
        finalized_height,
        finalized_block_hash,
    )?;
    let finality = entries[0].snapshot.finality;
    let archive = local_authority
        .rebuild_and_verify(finality, &expected)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
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
            inventory: expected,
            checkpoints,
            checkpoint_set_sha256,
            finality: finality.into(),
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
    let authenticated_body = verify_sccp_replay_archive_checkpoint_v1(policy, &checkpoint)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)?;
    // Authenticate the exact raw content address before invoking canonical
    // decoding and sparse-forest reconstruction.
    if snapshot_bytes.is_empty()
        || snapshot_bytes.len() > limits.max_snapshot_bytes
        || sha256(&[snapshot_bytes.as_slice()]) != authenticated_body.snapshot_sha256
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let snapshot = decode_sccp_replay_archive_snapshot_v1(&snapshot_bytes, limits)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if authenticated_body.accumulator_id != snapshot.accumulator_id
        || authenticated_body.domain != snapshot.domain
        || authenticated_body.finality != snapshot.finality
        || authenticated_body.forest != snapshot.forest
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let checkpoint_agreement_digest = authenticated_body
        .agreement_digest()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let checkpoint_bytes = norito::encode_canonical(&checkpoint)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
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
    checkpoint_set_sha256: [u8; 32],
    network_identity_sha256: [u8; 32],
    finalized_height: u64,
    finalized_block_hash: [u8; 32],
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let Some(previous) = previous else {
        // A fresh replica has no local snapshot head from which to authenticate
        // archive history. The signed current snapshot is nevertheless safe to
        // adopt because the local authority independently rebuilds its complete
        // forest from finalized Kura execution below.
        return Ok(());
    };
    if network_identity_sha256 != previous.manifest.network_identity_sha256 {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    if finalized_height == previous.manifest.finalized_height {
        let entries_are_identical = previous.entries.len() == entries.len()
            && previous
                .entries
                .iter()
                .zip(entries)
                .all(|(prior, current)| {
                    prior.snapshot_bytes == current.snapshot_bytes
                        && prior.checkpoint_bytes == current.checkpoint_bytes
                });
        return (finalized_block_hash == previous.manifest.finalized_block_hash
            && checkpoint_set_sha256 == previous.manifest.checkpoint_set_sha256
            && entries_are_identical)
            .then_some(())
            .ok_or(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    if finalized_height < previous.manifest.finalized_height
        || finalized_block_hash == previous.manifest.finalized_block_hash
    {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    let current = entries
        .iter()
        .map(|entry| (entry.snapshot.accumulator_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    let previous_ids = previous
        .entries
        .iter()
        .map(|entry| entry.snapshot.accumulator_id.clone())
        .collect::<BTreeSet<_>>();
    if current.iter().any(|(id, entry)| {
        !previous_ids.contains(id) && entry.snapshot.finality.predecessor_snapshot_sha256 != [0; 32]
    }) {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    for prior in &previous.entries {
        let next = current
            .get(&prior.snapshot.accumulator_id)
            .ok_or(ToriiSccpReplayStartupErrorV1::Continuity)?;
        // The endpoint carries only the latest complete snapshot, so an
        // offline node may legitimately miss one or more predecessor heads.
        // The full local Kura rebuild authenticates the current forest; these
        // checks additionally prove that the candidate did not discard any
        // locally observed replay fact while advancing.
        let predecessor = next.snapshot.finality.predecessor_snapshot_sha256;
        let is_adjacent = previous
            .manifest
            .finalized_height
            .checked_add(1)
            .is_some_and(|height| height == finalized_height);
        if predecessor == [0; 32]
            || (is_adjacent && predecessor != prior.checkpoint.body.snapshot_sha256)
            || next.snapshot.domain != prior.snapshot.domain
            || next.snapshot.forest.leaf_count < prior.snapshot.forest.leaf_count
            || next.snapshot.forest.update_sequence < prior.snapshot.forest.update_sequence
            || !snapshot_contains_prior_leaves(&prior.snapshot, &next.snapshot)
        {
            return Err(ToriiSccpReplayStartupErrorV1::Continuity);
        }
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

    fn recover(
        &self,
        config: &ToriiSccpReplayArchive,
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        #[cfg(unix)]
        {
            return recover_interrupted_publications(
                &self.directory,
                max_store_names(config.max_accumulators)?,
            );
        }
        #[cfg(not(unix))]
        {
            let _ = config;
            Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
        }
    }

    fn prune_to_manifest(
        &self,
        config: &ToriiSccpReplayArchive,
        manifest: Option<&PersistedReplayHeadV1>,
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        #[cfg(unix)]
        {
            let mut retained = BTreeSet::new();
            if let Some(manifest) = manifest {
                for entry in &manifest.entries {
                    if !retained.insert(snapshot_filename(entry.snapshot_sha256))
                        || !retained.insert(checkpoint_filename(entry.checkpoint_sha256))
                    {
                        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
                    }
                }
            }
            return prune_replay_artifacts(
                &self.directory,
                &retained,
                max_store_names(config.max_accumulators)?,
            );
        }
        #[cfg(not(unix))]
        {
            let _ = (config, manifest);
            Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
        }
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
        if manifest.version != HEAD_MANIFEST_VERSION_V1
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
        let mut wire_entries = Vec::with_capacity(manifest.entries.len());
        let mut previous_id = None;
        let mut aggregate_bytes = 0_usize;
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
            let snapshot_limit = config.max_snapshot_bytes.min(
                config
                    .max_response_bytes
                    .checked_sub(aggregate_bytes)
                    .filter(|remaining| *remaining != 0)
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?,
            );
            let snapshot_bytes =
                secure_read_relative(&self.directory, &snapshot_name, snapshot_limit)?
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            aggregate_bytes = aggregate_bytes
                .checked_add(snapshot_bytes.len())
                .filter(|bytes| *bytes <= config.max_response_bytes)
                .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            let checkpoint_limit = MAX_PERSISTED_CHECKPOINT_BYTES_V1.min(
                config
                    .max_response_bytes
                    .checked_sub(aggregate_bytes)
                    .filter(|remaining| *remaining != 0)
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?,
            );
            let checkpoint_bytes =
                secure_read_relative(&self.directory, &checkpoint_name, checkpoint_limit)?
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            aggregate_bytes = aggregate_bytes
                .checked_add(checkpoint_bytes.len())
                .filter(|bytes| *bytes <= config.max_response_bytes)
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
            wire_entries.push(SccpReplayReplicaCheckpointEntryV1 {
                checkpoint,
                snapshot_bytes,
            });
        }
        let wire: SccpReplayReplicaCheckpointSetV1 = wire_entries;
        if checkpoint_set_digest_bounded(&wire, config.max_response_bytes)?
            != manifest.checkpoint_set_sha256
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let mut entries = Vec::with_capacity(manifest.entries.len());
        for (head, wire_entry) in manifest.entries.iter().zip(wire) {
            let entry = validate_entry(
                wire_entry.checkpoint,
                wire_entry.snapshot_bytes,
                &policy,
                limits,
            )
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
            entries.push(entry);
        }
        Ok(Some(PersistedReplayHeadStateV1 { manifest, entries }))
    }

    fn write_candidate_artifacts(
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
        Ok(())
    }

    fn publish_candidate_manifest(
        &self,
        config: &ToriiSccpReplayArchive,
        candidate: &CandidateReplayStateV1,
    ) -> Result<(), ManifestPublicationFailureV1> {
        let manifest_bytes = norito::encode_canonical(&candidate.manifest)
            .map_err(|_| ManifestPublicationFailureV1::BeforeCommit)?;
        secure_write_manifest_last_relative(
            &self.directory,
            HEAD_MANIFEST_FILENAME_V1,
            &manifest_bytes,
            Self::manifest_limit(config),
        )
    }

    fn confirm_candidate_manifest(
        &self,
        config: &ToriiSccpReplayArchive,
        candidate: &CandidateReplayStateV1,
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        let expected = norito::encode_canonical(&candidate.manifest)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        self.directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let actual = secure_read_relative(
            &self.directory,
            HEAD_MANIFEST_FILENAME_V1,
            Self::manifest_limit(config),
        )?
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        (actual == expected)
            .then_some(())
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)
    }
}

fn snapshot_filename(digest: [u8; 32]) -> String {
    format!("snapshot-{}.norito", hex::encode(digest))
}

fn checkpoint_filename(digest: [u8; 32]) -> String {
    format!("checkpoint-{}.norito", hex::encode(digest))
}

#[cfg(unix)]
fn max_store_names(max_accumulators: usize) -> Result<usize, ToriiSccpReplayStartupErrorV1> {
    max_accumulators
        .checked_mul(4)
        .and_then(|count| count.checked_add(8))
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)
}

#[cfg(unix)]
fn is_lower_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(unix)]
fn is_replay_artifact_filename(name: &str) -> bool {
    ["snapshot-", "checkpoint-"].iter().any(|prefix| {
        name.strip_prefix(prefix)
            .and_then(|suffix| suffix.strip_suffix(".norito"))
            .is_some_and(|digest| digest.len() == 64 && is_lower_hex(digest.as_bytes()))
    })
}

#[cfg(unix)]
fn temporary_destination(name: &str) -> Option<&str> {
    let body = name.strip_prefix('.')?.strip_suffix(".tmp")?;
    let (destination, suffix) = body.rsplit_once('.')?;
    (suffix.len() == 32
        && is_lower_hex(suffix.as_bytes())
        && (destination == HEAD_MANIFEST_FILENAME_V1 || is_replay_artifact_filename(destination)))
    .then_some(destination)
}

#[cfg(unix)]
fn secure_store_names(
    directory: &File,
    max_entries: usize,
) -> Result<BTreeSet<String>, ToriiSccpReplayStartupErrorV1> {
    let mut names = BTreeSet::new();
    let mut entries = rustix::fs::Dir::read_from(directory)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    for entry in &mut entries {
        let entry = entry.map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        let name = std::str::from_utf8(bytes)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
            .to_owned();
        if !names.insert(name) || names.len() > max_entries {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
    }
    Ok(names)
}

#[cfg(unix)]
fn validate_owned_private_regular(
    directory: &File,
    name: &str,
) -> Result<rustix::fs::Stat, ToriiSccpReplayStartupErrorV1> {
    let stat = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o777 != 0o600
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(stat)
}

#[cfg(unix)]
fn unlink_and_sync_all(
    directory: &File,
    names: &[String],
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let mut mutated = false;
    let mut failed = false;
    for name in names {
        match rustix::fs::unlinkat(directory, name.as_str(), rustix::fs::AtFlags::empty()) {
            Ok(()) => mutated = true,
            Err(_) => failed = true,
        }
    }
    if mutated && directory.sync_all().is_err() {
        failed = true;
    }
    if failed {
        Err(ToriiSccpReplayStartupErrorV1::Persistence)
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn recover_interrupted_publications(
    directory: &File,
    max_entries: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let names = secure_store_names(directory, max_entries)?;
    let mut temporary_names = Vec::new();
    for name in &names {
        let Some(destination) = temporary_destination(name) else {
            continue;
        };
        let temporary = validate_owned_private_regular(directory, name)?;
        match temporary.st_nlink {
            1 => {}
            2 => {
                let destination = validate_owned_private_regular(directory, destination)?;
                if destination.st_nlink != 2
                    || destination.st_dev != temporary.st_dev
                    || destination.st_ino != temporary.st_ino
                {
                    return Err(ToriiSccpReplayStartupErrorV1::Persistence);
                }
            }
            _ => return Err(ToriiSccpReplayStartupErrorV1::Persistence),
        }
        temporary_names.push(name.clone());
    }
    unlink_and_sync_all(directory, &temporary_names)
}

#[cfg(unix)]
fn prune_replay_artifacts(
    directory: &File,
    retained: &BTreeSet<String>,
    max_entries: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    recover_interrupted_publications(directory, max_entries)?;
    let names = secure_store_names(directory, max_entries)?;
    let mut removals = Vec::new();
    for name in names {
        if name == PROCESS_LOCK_FILENAME_V1 || name == HEAD_MANIFEST_FILENAME_V1 {
            continue;
        }
        if !is_replay_artifact_filename(&name) {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let stat = validate_owned_private_regular(directory, &name)?;
        if stat.st_nlink != 1 {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        if !retained.contains(&name) {
            removals.push(name);
        }
    }
    unlink_and_sync_all(directory, &removals)
}

struct BoundedSha256WriterV1 {
    hasher: Sha256,
    written: usize,
    max_bytes: usize,
}

impl BoundedSha256WriterV1 {
    fn new(domain: &[u8], max_bytes: usize) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(domain);
        Self {
            hasher,
            written: 0,
            max_bytes,
        }
    }

    fn finish(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }
}

impl std::io::Write for BoundedSha256WriterV1 {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self
            .written
            .checked_add(bytes.len())
            .filter(|next| *next <= self.max_bytes)
            .ok_or_else(|| std::io::Error::other("bounded SCCP replay digest exceeded"))?;
        self.hasher.update(bytes);
        self.written = next;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn checkpoint_set_digest_bounded(
    set: &SccpReplayReplicaCheckpointSetV1,
    max_bytes: usize,
) -> Result<[u8; 32], ToriiSccpReplayStartupErrorV1> {
    let mut writer = BoundedSha256WriterV1::new(CHECKPOINT_SET_DIGEST_DOMAIN_V1, max_bytes);
    norito::core::write_canonical_to_writer(set, &mut writer)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    Ok(writer.finish())
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
) -> Result<(), ManifestPublicationFailureV1> {
    if bytes.is_empty() || bytes.len() > max_bytes || !secure_filename(name) {
        return Err(ManifestPublicationFailureV1::BeforeCommit);
    }
    let (mut temporary, temporary_name) = create_secure_temporary(directory, name)
        .map_err(|_| ManifestPublicationFailureV1::BeforeCommit)?;
    let precommit = temporary
        .write_all(bytes)
        .and_then(|()| temporary.sync_all())
        .map_err(|_| ManifestPublicationFailureV1::BeforeCommit)
        .and_then(|()| {
            rustix::fs::renameat(directory, temporary_name.as_str(), directory, name)
                .map_err(|_| ManifestPublicationFailureV1::BeforeCommit)
        });
    if let Err(error) = precommit {
        let _ = rustix::fs::unlinkat(
            directory,
            temporary_name.as_str(),
            rustix::fs::AtFlags::empty(),
        );
        return Err(error);
    }
    directory
        .sync_all()
        .map_err(|_| ManifestPublicationFailureV1::AfterRename)?;
    let readback = secure_read_relative(directory, name, max_bytes)
        .map_err(|_| ManifestPublicationFailureV1::AfterRename)?
        .ok_or(ManifestPublicationFailureV1::AfterRename)?;
    (readback == bytes)
        .then_some(())
        .ok_or(ManifestPublicationFailureV1::AfterRename)
}

#[cfg(not(unix))]
fn secure_write_manifest_last_relative(
    _directory: &File,
    _name: &str,
    _bytes: &[u8],
    _max_bytes: usize,
) -> Result<(), ManifestPublicationFailureV1> {
    Err(ManifestPublicationFailureV1::BeforeCommit)
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
        path::PathBuf,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use base64::Engine as _;
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
        pollution_path: Mutex<Option<PathBuf>>,
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

        fn clear(&self) {
            self.responses
                .lock()
                .expect("source lock is healthy")
                .clear();
        }

        fn pollute_on_fetch(&self, path: PathBuf) {
            *self
                .pollution_path
                .lock()
                .expect("pollution-path lock is healthy") = Some(path);
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
            if let Some(path) = self
                .pollution_path
                .lock()
                .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?
                .as_ref()
            {
                fs::write(path, b"unexpected same-uid store entry")
                    .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
            }
            if bytes.len() > max_response_bytes {
                return Err(SccpReplayCheckpointSourceErrorV1::Limit);
            }
            Ok(bytes)
        }
    }

    struct EmptyForestLocalAuthority {
        checkpoint_hashes: Mutex<BTreeMap<u64, [u8; 32]>>,
        current_coordinate: Mutex<(u64, [u8; 32])>,
        inventory_current: Mutex<bool>,
        transient_inventory_mismatches: Mutex<usize>,
    }

    impl EmptyForestLocalAuthority {
        fn new(finalized_height: u64, finalized_block_hash: [u8; 32]) -> Self {
            Self {
                checkpoint_hashes: Mutex::new(BTreeMap::from([(
                    finalized_height,
                    finalized_block_hash,
                )])),
                current_coordinate: Mutex::new((finalized_height, finalized_block_hash)),
                inventory_current: Mutex::new(true),
                transient_inventory_mismatches: Mutex::new(0),
            }
        }

        fn set_coordinate(&self, finalized_height: u64, finalized_block_hash: [u8; 32]) {
            self.checkpoint_hashes
                .lock()
                .expect("local-authority checkpoint lock is healthy")
                .insert(finalized_height, finalized_block_hash);
            *self
                .current_coordinate
                .lock()
                .expect("local-authority current-coordinate lock is healthy") =
                (finalized_height, finalized_block_hash);
        }

        fn set_inventory_current(&self, current: bool) {
            *self
                .inventory_current
                .lock()
                .expect("local-authority inventory lock is healthy") = current;
        }

        fn reject_next_inventory_validation(&self) {
            *self
                .transient_inventory_mismatches
                .lock()
                .expect("local-authority transient mismatch lock is healthy") = 1;
        }

        fn rebuild_empty(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
            require_current_coordinate: bool,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            let current = *self
                .current_coordinate
                .lock()
                .map_err(|_| SccpReplayLocalAuthorityErrorV1::Finality)?;
            if finality.network_identity_sha256 != [0x91; 32]
                || (require_current_coordinate
                    && current != (finality.finalized_height, finality.finalized_block_hash))
                || self
                    .checkpoint_hashes
                    .lock()
                    .map_err(|_| SccpReplayLocalAuthorityErrorV1::Finality)?
                    .get(&finality.finalized_height)
                    .copied()
                    != Some(finality.finalized_block_hash)
            {
                return Err(SccpReplayLocalAuthorityErrorV1::Finality);
            }
            self.validate_current_inventory(expected)?;
            let mut archive = SccpReplayArchiveV1::default();
            for (id, (domain, _)) in expected {
                archive
                    .initialize_accumulator(id.clone(), *domain)
                    .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)?;
            }
            Ok(archive)
        }
    }

    impl SccpReplayLocalAuthorityV1 for EmptyForestLocalAuthority {
        fn validate_current_inventory(
            &self,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            let mut transient = self
                .transient_inventory_mismatches
                .lock()
                .map_err(|_| SccpReplayLocalAuthorityErrorV1::CoreMismatch)?;
            if *transient > 0 {
                *transient -= 1;
                return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
            }
            let current = *self
                .inventory_current
                .lock()
                .map_err(|_| SccpReplayLocalAuthorityErrorV1::CoreMismatch)?;
            if current
                && !expected.is_empty()
                && expected
                    .values()
                    .all(|(_, forest)| forest == &SccpReplayForestV1::default())
            {
                Ok(())
            } else {
                Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch)
            }
        }

        fn rebuild_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            self.rebuild_empty(finality, expected, true)
        }

        fn rebuild_persisted_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            self.rebuild_empty(finality, expected, false)
        }
    }

    struct RejectingLocalAuthority;

    impl SccpReplayLocalAuthorityV1 for RejectingLocalAuthority {
        fn validate_current_inventory(
            &self,
            _expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch)
        }

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
        local_authority: Arc<EmptyForestLocalAuthority>,
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
                replicas: replicas.clone(),
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
            let first_bytes = checkpoint_set_bytes(&first_snapshot, &key_pairs, &config.replicas);
            let source = Arc::new(MutableSource::default());
            source.set_all(&config.replicas, &first_bytes);
            let local_authority = Arc::new(EmptyForestLocalAuthority::new(1, [0x41; 32]));
            Self {
                _temporary_root: temporary_root,
                config,
                source,
                local_authority,
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
            self.local_authority.set_coordinate(height, block_hash);
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
                self.local_authority.clone(),
            )
        }
    }

    fn accumulator_id() -> SccpReplayAccumulatorIdV1 {
        SccpReplayAccumulatorIdV1::from_domain(
            SccpRouteKeyV1::new(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                "taira_eth_xor".to_owned(),
                "xor".to_owned(),
                7,
            )
            .expect("valid final-V1 route key"),
            &domain(),
        )
        .expect("valid replay accumulator identity")
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

    fn alternate_accumulator_id() -> SccpReplayAccumulatorIdV1 {
        SccpReplayAccumulatorIdV1::from_domain(
            SccpRouteKeyV1::new(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                "taira_eth_val".to_owned(),
                "val".to_owned(),
                9,
            )
            .expect("valid alternate route key"),
            &alternate_domain(),
        )
        .expect("valid alternate replay accumulator identity")
    }

    fn alternate_domain() -> SccpReplayDomainV1 {
        SccpReplayDomainV1 {
            source_network: SccpNetworkV1::SoraTaira,
            target_network: SccpNetworkV1::EthereumMainnet,
            boundary: SccpReplayBoundaryV1::SoraOutboundLock,
            route_revision: 9,
            route_configuration_hash: [0x45; 32],
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
        checkpoint_set_bytes_for_snapshots(std::slice::from_ref(snapshot), key_pairs, replicas)
    }

    fn checkpoint_set_bytes_for_snapshots(
        snapshots: &[SccpReplayArchiveSnapshotV1],
        key_pairs: &[KeyPair; 3],
        replicas: &[ToriiSccpReplayArchiveReplica; 3],
    ) -> Vec<u8> {
        let mut snapshots = snapshots.iter().collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.accumulator_id.cmp(&right.accumulator_id));
        let entries = snapshots
            .into_iter()
            .map(|snapshot| {
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
                SccpReplayReplicaCheckpointEntryV1 {
                    checkpoint: SccpReplayArchiveSignedCheckpointV1 { body, attestations },
                    snapshot_bytes: norito::encode_canonical(snapshot)
                        .expect("snapshot canonically encodes"),
                }
            })
            .collect();
        norito::encode_canonical(&entries).expect("checkpoint set canonically encodes")
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
    fn atomic_non_membership_read_returns_one_current_authenticated_bundle() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        let key = [0x5a; 32];

        let response = service
            .read_non_membership_witness(&fixture.accumulator_id, key)
            .expect("a current empty replay forest serves non-membership");

        assert_eq!(response.version, 1);
        assert_eq!(response.accumulator_id, fixture.accumulator_id);
        assert_eq!(response.domain, fixture.domain);
        assert_eq!(
            response.domain_hash_hex,
            hex::encode(fixture.accumulator_id.domain_hash)
        );
        assert_eq!(response.replay_key_hex, hex::encode(key));
        assert_eq!(response.checkpoint_height, 1);
        assert_eq!(response.checkpoint_block_hash_hex, hex::encode([0x41; 32]));
        assert_eq!(
            response.checkpoint_set_sha256_hex,
            hex::encode(
                service
                    .checkpoint_set_sha256()
                    .expect("published checkpoint-set digest is readable")
            )
        );

        let witness_bytes = base64::engine::general_purpose::STANDARD
            .decode(&response.replay_witness_b64)
            .expect("response witness is padded base64");
        let witness: SccpSparseMerkleWitnessV1 = norito::decode_canonical_with_limits(
            &witness_bytes,
            norito::canonical_decode_limits(witness_bytes.len()),
        )
        .expect("response witness is canonical Norito");
        response
            .forest
            .verify_key_digest(key, [0; 32], &witness)
            .expect("response forest authenticates its witness");

        let checkpoint_bytes = base64::engine::general_purpose::STANDARD
            .decode(&response.checkpoint_b64)
            .expect("response checkpoint is padded base64");
        let checkpoint: SccpReplayArchiveSignedCheckpointV1 = norito::decode_canonical_with_limits(
            &checkpoint_bytes,
            norito::canonical_decode_limits(checkpoint_bytes.len()),
        )
        .expect("response checkpoint is canonical Norito");
        let authenticated =
            verify_sccp_replay_archive_checkpoint_v1(&replica_policy(&fixture.config), &checkpoint)
                .expect("response checkpoint is authenticated by the pinned replicas");
        assert_eq!(authenticated.accumulator_id, fixture.accumulator_id);
        assert_eq!(authenticated.domain, fixture.domain);
        assert_eq!(authenticated.forest, response.forest);
        assert_eq!(
            authenticated.finality.finalized_height,
            response.checkpoint_height
        );
        assert_eq!(
            hex::encode(authenticated.finality.finalized_block_hash),
            response.checkpoint_block_hash_hex
        );
    }

    #[test]
    fn multi_accumulator_checkpoints_bind_one_coordinate_but_independent_predecessors() {
        let fixture = Fixture::new();
        let alternate_id = alternate_accumulator_id();
        let alternate_domain = alternate_domain();
        let initial = [
            snapshot(
                &fixture.accumulator_id,
                fixture.domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: 1,
                    finalized_block_hash: [0x41; 32],
                    predecessor_snapshot_sha256: [0; 32],
                },
            ),
            snapshot(
                &alternate_id,
                alternate_domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: 1,
                    finalized_block_hash: [0x41; 32],
                    predecessor_snapshot_sha256: [0; 32],
                },
            ),
        ];
        let first_hashes = initial
            .each_ref()
            .map(|item| item.content_sha256().expect("initial snapshot hashes"));
        let initial_bytes = checkpoint_set_bytes_for_snapshots(
            &initial,
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture
            .source
            .set_all(&fixture.config.replicas, &initial_bytes);
        let service = fixture
            .bootstrap()
            .expect("multi-accumulator initial head bootstraps");

        fixture.local_authority.set_coordinate(2, [0x42; 32]);
        let successor = [
            snapshot(
                &fixture.accumulator_id,
                fixture.domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: 2,
                    finalized_block_hash: [0x42; 32],
                    predecessor_snapshot_sha256: first_hashes[0],
                },
            ),
            snapshot(
                &alternate_id,
                alternate_domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: 2,
                    finalized_block_hash: [0x42; 32],
                    predecessor_snapshot_sha256: first_hashes[1],
                },
            ),
        ];
        assert_ne!(
            successor[0].finality.predecessor_snapshot_sha256,
            successor[1].finality.predecessor_snapshot_sha256
        );
        let successor_bytes = checkpoint_set_bytes_for_snapshots(
            &successor,
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture
            .source
            .set_all(&fixture.config.replicas, &successor_bytes);
        service
            .refresh()
            .expect("independent predecessor hashes share one finalized coordinate");

        for accumulator_id in [&fixture.accumulator_id, &alternate_id] {
            let response = service
                .read_non_membership_witness(accumulator_id, [0x5a; 32])
                .expect("every accumulator serves its own authenticated checkpoint");
            assert_eq!(response.checkpoint_height, 2);
            assert_eq!(response.checkpoint_block_hash_hex, hex::encode([0x42; 32]));
        }
    }

    #[test]
    fn atomic_non_membership_read_fails_closed_when_published_head_is_stale() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        fixture.local_authority.set_inventory_current(false);

        assert_eq!(
            service
                .read_non_membership_witness(&fixture.accumulator_id, [0x5a; 32])
                .map(|_| ()),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
        );
        assert_eq!(
            service.forest(&fixture.accumulator_id).map(|_| ()),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service
                .witness(&fixture.accumulator_id, [0x5a; 32])
                .map(|_| ()),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service.checkpoint(&fixture.accumulator_id).map(|_| ()),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service.checkpoint_set_sha256().map(|_| ()),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
        );
    }

    #[test]
    fn atomic_non_membership_read_distinguishes_unknown_and_corrupt_state() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        let mut unknown = fixture.accumulator_id.clone();
        unknown.domain_hash[0] ^= 1;
        assert_eq!(
            service
                .read_non_membership_witness(&unknown, [0x5a; 32])
                .map(|_| ()),
            Err(ToriiSccpReplayEndpointErrorV1::NotFound)
        );

        service
            .published
            .write()
            .expect("published state lock is healthy")
            .checkpoints
            .get_mut(&fixture.accumulator_id)
            .expect("fixture checkpoint is present")
            .attestations[0]
            .signature[0] ^= 1;
        assert_eq!(
            service
                .read_non_membership_witness(&fixture.accumulator_id, [0x5a; 32])
                .map(|_| ()),
            Err(ToriiSccpReplayEndpointErrorV1::Integrity)
        );
    }

    #[test]
    fn unrelated_finality_advancement_does_not_stale_an_unchanged_replay_inventory() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        fixture.local_authority.set_coordinate(2, [0x42; 32]);
        assert!(
            !service
                .refresh_if_stale()
                .expect("unchanged replay inventory needs no fetch")
        );
        service
            .read_non_membership_witness(&fixture.accumulator_id, [0x5a; 32])
            .expect("an older finalized checkpoint remains valid for unchanged replay state");
    }

    #[test]
    fn refresh_if_stale_revalidates_and_publishes_an_authenticated_head() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        fixture.local_authority.reject_next_inventory_validation();
        assert!(
            service
                .refresh_if_stale()
                .expect("a transiently stale head is revalidated and published")
        );
        assert!(
            !service
                .refresh_if_stale()
                .expect("the revalidated replay inventory needs no duplicate fetch")
        );
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
    fn checkpoint_set_count_is_rejected_before_entry_validation() {
        let mut fixture = Fixture::new();
        fixture.config.max_accumulators = 1;
        let one = snapshot(
            &fixture.accumulator_id,
            fixture.domain,
            SccpReplayArchiveFinalityV1 {
                network_identity_sha256: [0x91; 32],
                finalized_height: 1,
                finalized_block_hash: [0x41; 32],
                predecessor_snapshot_sha256: [0; 32],
            },
        );
        let oversized = checkpoint_set_bytes_for_snapshots(
            &[one.clone(), one],
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        assert_eq!(
            validate_candidate(
                &fixture.config,
                &oversized,
                None,
                fixture.local_authority.as_ref(),
            )
            .err(),
            Some(ToriiSccpReplayStartupErrorV1::ResourceLimit)
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
        set[0].checkpoint.attestations[1].signature[0] ^= 1;
        let forged = norito::encode_canonical(&set).expect("forged set remains canonical");
        fixture.source.set_all(&fixture.config.replicas, &forged);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)
        );
    }

    #[test]
    fn bootstrap_rejects_canonical_snapshot_substitution_against_signed_hash() {
        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        let entry = &mut set[0];
        let mut substituted: SccpReplayArchiveSnapshotV1 = norito::decode_canonical_with_limits(
            &entry.snapshot_bytes,
            norito::canonical_decode_limits(entry.snapshot_bytes.len()),
        )
        .expect("fixture snapshot decodes");
        substituted.finality.finalized_block_hash[0] ^= 1;
        entry.snapshot_bytes =
            norito::encode_canonical(&substituted).expect("substitute snapshot encodes");
        assert_ne!(
            sha256(&[entry.snapshot_bytes.as_slice()]),
            entry.checkpoint.body.snapshot_sha256,
            "the test must reach the authenticated raw-snapshot hash check"
        );
        let substituted_set =
            norito::encode_canonical(&set).expect("substitute checkpoint set encodes");
        fixture
            .source
            .set_all(&fixture.config.replicas, &substituted_set);

        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Malformed)
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
    fn canonical_replay_state_ignores_ambient_norito_layout() {
        let fixture = Fixture::new();
        let set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let alternate = norito::to_bytes(&set).expect("alternate-layout checkpoint set encodes");
        assert_ne!(alternate, fixture.first_bytes);

        let service = fixture
            .bootstrap()
            .expect("canonical checkpoint bootstraps under ambient layout");
        drop(service);
        drop(ambient);

        fixture
            .bootstrap()
            .expect("persisted canonical state reloads independently of ambient layout");
    }

    #[test]
    fn fresh_bootstrap_accepts_a_locally_rebuilt_successor_snapshot() {
        let fixture = Fixture::new();
        let (successor, _) = fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        fixture
            .bootstrap()
            .expect("fresh local authority adopts the current signed snapshot");
    }

    #[test]
    fn refresh_accepts_cached_or_strict_successor_heads_and_rejects_forks() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        service
            .refresh()
            .expect("the exact authenticated current head is an idempotent refresh");
        let (successor, successor_hash) =
            fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service.refresh().expect("strict successor refreshes");
        service
            .refresh()
            .expect("the exact successor head is also idempotent");

        let (same_height_fork, _) = fixture.bytes_at(2, [0x52; 32], successor_hash);
        fixture
            .source
            .set_all(&fixture.config.replicas, &same_height_fork);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "an equal-height different block is a fork"
        );
        fixture.local_authority.set_coordinate(2, [0x42; 32]);
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
    fn refresh_accepts_a_complete_head_after_missing_intermediate_snapshots() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (_, unseen_successor_hash) =
            fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        let (current, current_hash) = fixture.bytes_at(3, [0x43; 32], unseen_successor_hash);
        fixture.source.set_all(&fixture.config.replicas, &current);

        service
            .refresh()
            .expect("complete current snapshot is locally rebuilt after an offline interval");
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("current checkpoint is published")
                .body
                .snapshot_sha256,
            current_hash
        );
    }

    #[test]
    fn successor_requires_existing_and_new_accumulator_predecessor_sentinels() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let alternate_id = alternate_accumulator_id();
        let alternate_domain = alternate_domain();
        fixture.local_authority.set_coordinate(2, [0x42; 32]);
        let existing = snapshot(
            &fixture.accumulator_id,
            fixture.domain,
            SccpReplayArchiveFinalityV1 {
                network_identity_sha256: [0x91; 32],
                finalized_height: 2,
                finalized_block_hash: [0x42; 32],
                predecessor_snapshot_sha256: fixture.first_snapshot_sha256,
            },
        );
        let forged_new = snapshot(
            &alternate_id,
            alternate_domain,
            SccpReplayArchiveFinalityV1 {
                network_identity_sha256: [0x91; 32],
                finalized_height: 2,
                finalized_block_hash: [0x42; 32],
                predecessor_snapshot_sha256: [0x99; 32],
            },
        );
        let forged = checkpoint_set_bytes_for_snapshots(
            &[existing.clone(), forged_new],
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture.source.set_all(&fixture.config.replicas, &forged);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "a newly introduced accumulator must carry the zero predecessor sentinel"
        );

        let valid_new = snapshot(
            &alternate_id,
            alternate_domain,
            SccpReplayArchiveFinalityV1 {
                network_identity_sha256: [0x91; 32],
                finalized_height: 2,
                finalized_block_hash: [0x42; 32],
                predecessor_snapshot_sha256: [0; 32],
            },
        );
        let valid = checkpoint_set_bytes_for_snapshots(
            &[existing, valid_new],
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture.source.set_all(&fixture.config.replicas, &valid);
        service
            .refresh()
            .expect("a new accumulator with the zero sentinel is accepted");
        service
            .read_non_membership_witness(&alternate_id, [0x5a; 32])
            .expect("the newly introduced accumulator is published");
    }

    #[test]
    fn process_lock_rejects_a_second_writer_but_restart_reuses_the_head() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let original_checkpoint_set_sha256 = service
            .checkpoint_set_sha256()
            .expect("published checkpoint-set digest is readable");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "a second writer cannot race the retained manifest"
        );
        drop(service);
        let restarted = fixture
            .bootstrap()
            .expect("a restart re-authenticates and adopts the cached signed head");
        assert_eq!(
            restarted
                .checkpoint_set_sha256()
                .expect("restarted checkpoint-set digest is readable"),
            original_checkpoint_set_sha256
        );
        assert_eq!(
            restarted
                .checkpoint(&fixture.accumulator_id)
                .expect("restarted service retains the authenticated checkpoint")
                .body
                .snapshot_sha256,
            fixture.first_snapshot_sha256
        );
    }

    #[test]
    fn restart_reuses_a_current_persisted_inventory_after_unrelated_blocks() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        drop(service);

        fixture.local_authority.set_coordinate(2, [0x42; 32]);
        fixture.source.clear();
        let restarted = fixture
            .bootstrap()
            .expect("a Kura-authenticated current cache does not depend on replica availability");
        let response = restarted
            .read_non_membership_witness(&fixture.accumulator_id, [0x5a; 32])
            .expect("cached replay inventory remains readable after unrelated blocks");
        assert_eq!(response.checkpoint_height, 1);
        assert_eq!(response.checkpoint_block_hash_hex, hex::encode([0x41; 32]));
    }

    #[test]
    fn refresh_prunes_every_superseded_content_addressed_generation() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let mut predecessor = fixture.first_snapshot_sha256;

        for height in 2_u64..=8 {
            let block_hash = [u8::try_from(0x40 + height).expect("fixture height fits"); 32];
            let (successor, snapshot_sha256) = fixture.bytes_at(height, block_hash, predecessor);
            fixture.source.set_all(&fixture.config.replicas, &successor);
            service.refresh().expect("successor checkpoint refreshes");
            predecessor = snapshot_sha256;
            let checkpoint = service
                .checkpoint(&fixture.accumulator_id)
                .expect("current checkpoint is readable");
            let checkpoint_bytes =
                norito::encode_canonical(&checkpoint).expect("current checkpoint encodes");
            let checkpoint_sha256 = sha256(&[&checkpoint_bytes]);

            let names = fs::read_dir(&fixture.config.state_dir)
                .expect("state directory is readable")
                .map(|entry| {
                    entry
                        .expect("state entry is readable")
                        .file_name()
                        .into_string()
                        .expect("state entry name is UTF-8")
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                names,
                BTreeSet::from([
                    PROCESS_LOCK_FILENAME_V1.to_owned(),
                    HEAD_MANIFEST_FILENAME_V1.to_owned(),
                    snapshot_filename(snapshot_sha256),
                    checkpoint_filename(checkpoint_sha256),
                ])
            );
        }
    }

    #[test]
    fn refresh_reconciles_a_durable_head_that_precedes_the_memory_swap() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let previous = service
            .store
            .load_head(&fixture.config)
            .expect("durable head loads")
            .expect("durable head exists");
        let (successor, successor_snapshot_sha256) =
            fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        let candidate = validate_candidate(
            &fixture.config,
            &successor,
            Some(&previous),
            fixture.local_authority.as_ref(),
        )
        .expect("successor candidate validates");
        service
            .store
            .write_candidate_artifacts(&fixture.config, &candidate)
            .expect("candidate artifacts are durable");
        service
            .store
            .publish_candidate_manifest(&fixture.config, &candidate)
            .expect("candidate manifest is durable");
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("the old in-memory checkpoint remains readable")
                .body
                .snapshot_sha256,
            fixture.first_snapshot_sha256
        );

        service
            .refresh()
            .expect("refresh reconciles disk ahead of memory and remains restartable");
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("the reconciled checkpoint is readable")
                .body
                .snapshot_sha256,
            successor_snapshot_sha256
        );
    }

    #[test]
    fn postcommit_gc_failure_does_not_disable_a_coherent_head() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (successor, _) = fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        fixture
            .source
            .pollute_on_fetch(fixture.config.state_dir.join("unexpected-entry"));

        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "postcommit garbage collection reports its maintenance failure"
        );
        let response = service
            .read_non_membership_witness(&fixture.accumulator_id, [0x5a; 32])
            .expect("durable disk and memory heads remain safely readable");
        assert_eq!(response.checkpoint_height, 2);
        assert_eq!(response.checkpoint_block_hash_hex, hex::encode([0x42; 32]));
    }

    #[test]
    fn restart_prefers_the_authenticated_current_cache_over_remote_divergence() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (successor, successor_snapshot_sha256) =
            fixture.bytes_at(2, [0x42; 32], fixture.first_snapshot_sha256);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service.refresh().expect("strict successor refreshes");
        drop(service);

        let mut substituted_domain = fixture.domain;
        substituted_domain.route_configuration_hash[0] ^= 1;
        let (divergent, _) = fixture.bytes_at_with_domain(
            2,
            [0x42; 32],
            fixture.first_snapshot_sha256,
            substituted_domain,
        );
        fixture.source.set_all(&fixture.config.replicas, &divergent);
        let restarted = fixture
            .bootstrap()
            .expect("a divergent remote response cannot displace a current cache");
        assert_eq!(
            restarted
                .checkpoint(&fixture.accumulator_id)
                .expect("cached successor checkpoint remains current")
                .body
                .snapshot_sha256,
            successor_snapshot_sha256
        );
        drop(restarted);

        fixture
            .source
            .set_all(&fixture.config.replicas, &fixture.first_bytes);
        let restarted = fixture
            .bootstrap()
            .expect("a rollback response is ignored while the cache is current");
        assert_eq!(
            restarted
                .checkpoint(&fixture.accumulator_id)
                .expect("cached successor checkpoint remains current")
                .body
                .snapshot_sha256,
            successor_snapshot_sha256
        );
        drop(restarted);

        fixture.source.set_all(&fixture.config.replicas, &successor);
        let restarted = fixture
            .bootstrap()
            .expect("the exact durable successor remains recoverable");
        assert_eq!(
            restarted
                .checkpoint(&fixture.accumulator_id)
                .expect("restarted service retains the durable checkpoint")
                .body
                .snapshot_sha256,
            successor_snapshot_sha256
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
