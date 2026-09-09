//! Fail-closed Torii bootstrap and read provider for SCCP replay archives.
//!
//! Archive replicas are availability services, not consensus authorities.
//! Torii accepts a checkpoint only when all three configured HTTPS origins
//! return byte-identical canonical data, every pinned Ed25519 signature
//! verifies, every snapshot rebuilds, monotonic retained-leaf continuity
//! holds, and local Core/Kura authority reproduces the complete replay-forest
//! projection. The
//! durable head retains exactly its current generation and one authenticated
//! recovery generation; older content-addressed artifacts are pruned only
//! after the replacement head is durable.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read as _, Seek as _, Write as _},
    num::NonZeroUsize,
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use iroha_config::parameters::actual::{ToriiSccpReplayArchive, ToriiSccpReplayArchiveReplica};
use iroha_core::{
    bridge::rebuild_sccp_replay_archive_from_kura_v1,
    kura::Kura,
    state::{State as CoreState, WorldReadOnly as _},
};
use iroha_data_model::bridge::{
    SccpLaneIdV1, SccpNetworkV1, SccpReplayAccumulatorIdV1, SccpReplayActorV1,
    SccpReplayBoundaryV1, SccpReplayDomainV1, SccpReplayForestV1, SccpRouteKeyV1,
    SccpSparseMerkleWitnessV1,
};
use iroha_sccp::{
    SccpReplayArchiveCheckpointBodyV1, SccpReplayArchiveCheckpointSetEntryV1,
    SccpReplayArchiveDecodeLimitsV1, SccpReplayArchiveHeadFinalityV1,
    SccpReplayArchiveProviderErrorV1, SccpReplayArchiveProviderV1,
    SccpReplayArchiveReplicaBindingV1, SccpReplayArchiveReplicaPolicyV1,
    SccpReplayArchiveSignedCheckpointSetV1, SccpReplayArchiveSignedCheckpointV1,
    SccpReplayArchiveSnapshotV1, SccpReplayArchiveV1, SccpReplayRootResponseV1,
    SccpReplayWitnessResponseV1, decode_sccp_replay_archive_snapshot_v1,
    sccp_replay_archive_checkpoint_set_frame_sha256_v1,
    sccp_replay_archive_network_identity_sha256_v1, verify_sccp_replay_archive_checkpoint_set_v1,
    verify_sccp_replay_archive_checkpoint_v1,
};
use mv::storage::StorageReadOnly as _;
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};
use sorafs_car::{
    CarBuildPlan, CarWriter, FilePlan, compute_chunk_plan_digest_sha3, compute_por_root,
    sorafs_chunker::ChunkProfile,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, ManifestV1, PinPolicy,
    PinPolicyConstraints as SorafsPinPolicyConstraints, StorageClass, decode_manifest_v1_canonical,
    validate_manifest as validate_sorafs_manifest,
};

/// Canonical media type served by independent replay replicas.
pub const SCCP_REPLAY_CHECKPOINT_SET_MEDIA_TYPE_V1: &str = "application/x-iroha-norito";
/// Fixed relative endpoint fetched from each configured replica origin.
pub const SCCP_REPLAY_CHECKPOINT_SET_PATH_V1: &str = "v1/sccp/replay/checkpoint-set-v1";
/// Canonical public path name for the SORA outbound-lock accumulator.
pub const SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1: &str = "sora-outbound-lock";
/// Canonical public path name for the SORA inbound-release accumulator.
pub const SCCP_REPLAY_SORA_INBOUND_RELEASE_PATH_V1: &str = "sora-inbound-release";

const CHECKPOINT_SET_VERSION_V1: u8 = 1;
const HEAD_MANIFEST_VERSION_V1: u8 = 1;
const HEAD_MANIFEST_FILENAME_V1: &str = "head-v1.norito";
const REPLICA_FETCH_TEMP_FILENAME_V1: &str = "replica-checkpoint-set-v1.norito";
#[cfg(unix)]
const PROCESS_LOCK_FILENAME_V1: &str = "archive-v1.lock";
const SORAFS_INVENTORY_METADATA_KEY_V1: &str = "sccp-replay-inventory-sha256-v1";
const MAX_PERSISTED_CHECKPOINT_BYTES_V1: usize = 4 * 1024 * 1024;
#[cfg(unix)]
const SECURE_TEMP_RETRIES_V1: usize = 32;

/// Payload-free failure from a non-canonical replay accumulator or replay-key
/// path. The rejected path is deliberately not retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayPathErrorV1 {
    /// A segment was not the unique canonical final-V1 representation.
    Malformed,
}

impl core::fmt::Display for SccpReplayPathErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("malformed SCCP replay path")
    }
}

impl std::error::Error for SccpReplayPathErrorV1 {}

/// Canonical public route coordinate used to select a SORA replay accumulator.
///
/// The authenticated inventory supplies the full domain hash; URL text alone
/// cannot establish that consensus identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SccpReplayAccumulatorPathV1 {
    /// Exact governed external-to-SORA route coordinate.
    pub route_key: SccpRouteKeyV1,
    /// One of the two public SORA replay boundaries.
    pub boundary: SccpReplayBoundaryV1,
}

/// Decode the exact five-segment public identity of one authoritative SORA
/// replay accumulator.
///
/// The route registry is normalized to its canonical external-to-SORA lane.
/// External-contract replay forests are intentionally not addressable through
/// this public API.
pub fn decode_sccp_replay_accumulator_path_v1(
    boundary: &str,
    external_network: &str,
    route_id: &str,
    asset_key: &str,
    revision: &str,
) -> Result<SccpReplayAccumulatorPathV1, SccpReplayPathErrorV1> {
    let boundary = match boundary {
        SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1 => SccpReplayBoundaryV1::SoraOutboundLock,
        SCCP_REPLAY_SORA_INBOUND_RELEASE_PATH_V1 => SccpReplayBoundaryV1::SoraInboundRelease,
        _ => return Err(SccpReplayPathErrorV1::Malformed),
    };
    let external_network = SccpNetworkV1::from_profile_key(external_network)
        .filter(|network| network.is_external())
        .ok_or(SccpReplayPathErrorV1::Malformed)?;
    let revision_value = revision
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0 && value.to_string() == revision)
        .ok_or(SccpReplayPathErrorV1::Malformed)?;
    let route_key = SccpRouteKeyV1::new(
        SccpLaneIdV1 {
            source: external_network,
            target: SccpNetworkV1::SoraTaira,
        },
        route_id.to_owned(),
        asset_key.to_owned(),
        revision_value,
    )
    .map_err(|_| SccpReplayPathErrorV1::Malformed)?;
    let accumulator_id = SccpReplayAccumulatorPathV1 {
        route_key,
        boundary,
    };
    let encoded = encode_sccp_replay_accumulator_path_v1(&accumulator_id)?;
    if encoded
        != [
            boundary_path_name(boundary).to_owned(),
            external_network.profile_key().to_owned(),
            route_id.to_owned(),
            asset_key.to_owned(),
            revision.to_owned(),
        ]
    {
        return Err(SccpReplayPathErrorV1::Malformed);
    }
    Ok(accumulator_id)
}

/// Encode the unique five-segment public identity of one authoritative SORA
/// replay accumulator.
pub fn encode_sccp_replay_accumulator_path_v1(
    accumulator_id: &SccpReplayAccumulatorPathV1,
) -> Result<[String; 5], SccpReplayPathErrorV1> {
    accumulator_id
        .route_key
        .validate()
        .map_err(|_| SccpReplayPathErrorV1::Malformed)?;
    let boundary = boundary_path_name(accumulator_id.boundary);
    if boundary.is_empty() {
        return Err(SccpReplayPathErrorV1::Malformed);
    }
    let lane = accumulator_id.route_key.lane_id;
    if lane.target != SccpNetworkV1::SoraTaira || !lane.source.is_external() {
        return Err(SccpReplayPathErrorV1::Malformed);
    }
    Ok([
        boundary.to_owned(),
        lane.source.profile_key().to_owned(),
        accumulator_id.route_key.route_id.clone(),
        accumulator_id.route_key.asset_key.clone(),
        accumulator_id.route_key.revision.to_string(),
    ])
}

/// Decode one exact lowercase 64-hex-character replay-key path segment. The
/// all-zero key is canonical and denotes an ordinary non-membership query.
pub fn decode_sccp_replay_key_path_v1(replay_key: &str) -> Result<[u8; 32], SccpReplayPathErrorV1> {
    if replay_key.len() != 64 || !replay_key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SccpReplayPathErrorV1::Malformed);
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(replay_key, &mut decoded).map_err(|_| SccpReplayPathErrorV1::Malformed)?;
    if hex::encode(decoded) != replay_key {
        return Err(SccpReplayPathErrorV1::Malformed);
    }
    Ok(decoded)
}

const fn boundary_path_name(boundary: SccpReplayBoundaryV1) -> &'static str {
    match boundary {
        SccpReplayBoundaryV1::SoraOutboundLock => SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
        SccpReplayBoundaryV1::SoraInboundRelease => SCCP_REPLAY_SORA_INBOUND_RELEASE_PATH_V1,
        SccpReplayBoundaryV1::EvmSourceBurn
        | SccpReplayBoundaryV1::EvmDestinationMint
        | SccpReplayBoundaryV1::TronSourceBurn
        | SccpReplayBoundaryV1::TronDestinationMint
        | SccpReplayBoundaryV1::TonBridgeInboundMint
        | SccpReplayBoundaryV1::TonBridgeOutboundBurn
        | SccpReplayBoundaryV1::TonMasterMint
        | SccpReplayBoundaryV1::TonMasterBurn
        | SccpReplayBoundaryV1::TonWalletMintCredit
        | SccpReplayBoundaryV1::TonWalletBurnAuthorization
        | SccpReplayBoundaryV1::TonWalletBurnLock
        | SccpReplayBoundaryV1::TonWalletBurnRefund => "",
    }
}

/// One complete signed checkpoint and its exact canonical snapshot bytes.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sccp_replay::SccpReplayReplicaCheckpointEntryV1")]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayReplicaCheckpointEntryV1 {
    /// Exactly-three-signature checkpoint statement.
    pub checkpoint: SccpReplayArchiveSignedCheckpointV1,
    /// Canonical Norito snapshot whose content hash is signed by the checkpoint.
    pub snapshot_bytes: Vec<u8>,
}

/// Exact checkpoint set returned independently by all three replicas.
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sccp_replay::SccpReplayReplicaCheckpointSetV1")]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayReplicaCheckpointSetV1 {
    /// Schema version; final V1 accepts exactly one.
    pub version: u8,
    /// Three-replica signature over the complete ordered inventory, including
    /// the valid empty-inventory case.
    pub signed_set: SccpReplayArchiveSignedCheckpointSetV1,
    /// Exact canonical SoraFS manifest named by `signed_set`.
    pub sorafs_manifest_bytes: Vec<u8>,
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
    /// Stream one exact response without following redirects into an already
    /// authenticated owner-only descriptor.
    fn fetch_to(
        &self,
        replica: &ToriiSccpReplayArchiveReplica,
        max_response_bytes: usize,
        timeout: Duration,
        destination: &mut dyn std::io::Write,
    ) -> Result<usize, SccpReplayCheckpointSourceErrorV1>;
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
    fn fetch_to(
        &self,
        replica: &ToriiSccpReplayArchiveReplica,
        max_response_bytes: usize,
        _timeout: Duration,
        destination: &mut dyn std::io::Write,
    ) -> Result<usize, SccpReplayCheckpointSourceErrorV1> {
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
        let bytes_written = std::io::copy(&mut (&mut response).take(read_limit), destination)
            .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
        if bytes_written == 0 || bytes_written > max_response_bytes_u64 {
            return Err(SccpReplayCheckpointSourceErrorV1::Limit);
        }
        usize::try_from(bytes_written).map_err(|_| SccpReplayCheckpointSourceErrorV1::Limit)
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

    /// Authenticate a new remote candidate at the exact current committed coordinate.
    ///
    /// This is separate from serving a previously admitted inventory after
    /// unrelated blocks. A new candidate cannot claim that a later-created
    /// route already existed at an older finalized height.
    fn verify_candidate(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1>;

    /// Rebuild a securely persisted checkpoint whose replay inventory remains current.
    ///
    /// Implementations must authenticate the historical finalized coordinate
    /// and recheck the current inventory after the rebuild.
    fn rebuild_persisted_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        self.rebuild_and_verify(finality, expected)
    }

    /// Revalidate a previously rebuilt head against the current local replay
    /// projection before it is served. Implementations may
    /// override this with a cheaper current-head check, but must authenticate
    /// both the finalized coordinate and the complete accumulator inventory.
    fn verify_current(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        self.rebuild_and_verify(finality, expected).map(|_| ())
    }
}

struct CoreKuraSccpReplayLocalAuthorityV1 {
    state: Arc<CoreState>,
    kura: Arc<Kura>,
}

impl CoreKuraSccpReplayLocalAuthorityV1 {
    fn verify_current_projection(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
        require_current_coordinate: bool,
    ) -> Result<NonZeroUsize, SccpReplayLocalAuthorityErrorV1> {
        let state = self.state.query_view();
        let height =
            validate_checkpoint_against_core_view(&state, finality, require_current_coordinate)?;
        if self
            .kura
            .get_durable_block_hash(height)
            .map(|hash| *hash.as_ref())
            != Some(finality.finalized_block_hash)
        {
            return Err(SccpReplayLocalAuthorityErrorV1::Finality);
        }
        if &authoritative_core_replay_inventory(&state)? != expected {
            return Err(SccpReplayLocalAuthorityErrorV1::CoreMismatch);
        }
        Ok(height)
    }
}

impl SccpReplayLocalAuthorityV1 for CoreKuraSccpReplayLocalAuthorityV1 {
    fn verify_candidate(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        self.verify_current_projection(finality, expected, true)
            .map(|_| ())
    }

    fn rebuild_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        let height = self.verify_current_projection(finality, expected, true)?;
        let archive = rebuild_sccp_replay_archive_from_kura_v1(&self.kura, height, expected)
            .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)?;
        self.verify_current_projection(finality, expected, false)?;
        Ok(archive)
    }

    fn rebuild_persisted_and_verify(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
        let height = self.verify_current_projection(finality, expected, false)?;
        let archive = rebuild_sccp_replay_archive_from_kura_v1(&self.kura, height, expected)
            .map_err(|_| SccpReplayLocalAuthorityErrorV1::Rebuild)?;
        self.verify_current_projection(finality, expected, false)?;
        Ok(archive)
    }

    fn verify_current(
        &self,
        finality: iroha_sccp::SccpReplayArchiveFinalityV1,
        expected: &BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
        self.verify_current_projection(finality, expected, false)
            .map(|_| ())
    }
}

fn validate_checkpoint_against_core_view(
    state: &impl iroha_core::state::StateReadOnly,
    finality: iroha_sccp::SccpReplayArchiveFinalityV1,
    require_current_coordinate: bool,
) -> Result<NonZeroUsize, SccpReplayLocalAuthorityErrorV1> {
    let checkpoint_height = usize::try_from(finality.finalized_height)
        .ok()
        .and_then(NonZeroUsize::new)
        .filter(|height| {
            if require_current_coordinate {
                height.get() == state.block_hashes().len()
            } else {
                height.get() <= state.block_hashes().len()
            }
        })
        .ok_or(SccpReplayLocalAuthorityErrorV1::Finality)?;
    if finality.network_identity_sha256
        != sccp_replay_archive_network_identity_sha256_v1(state.network_id())
        || state
            .block_hashes()
            .get(checkpoint_height.get() - 1)
            .map(|hash| *hash.as_ref())
            != Some(finality.finalized_block_hash)
    {
        return Err(SccpReplayLocalAuthorityErrorV1::Finality);
    }
    Ok(checkpoint_height)
}

fn authoritative_core_replay_inventory(
    state: &impl iroha_core::state::StateReadOnly,
) -> Result<
    BTreeMap<SccpReplayAccumulatorIdV1, (SccpReplayDomainV1, SccpReplayForestV1)>,
    SccpReplayLocalAuthorityErrorV1,
> {
    let registry = state.sccp_registry();
    let world = state.world();
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
    /// Checkpoint-set or snapshot framing was malformed or exceeded limits.
    Malformed,
    /// One or more pinned signatures failed authentication.
    ReplicaAuthentication,
    /// A cached, regressed, forked, or non-monotonic head was supplied.
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

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sccp_replay::PersistedReplayHeadEntryV1")]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct PersistedReplayHeadEntryV1 {
    accumulator_id: SccpReplayAccumulatorIdV1,
    snapshot_sha256: [u8; 32],
    checkpoint_agreement_digest: [u8; 32],
    checkpoint_sha256: [u8; 32],
}

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sccp_replay::PersistedReplayGenerationV1")]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct PersistedReplayGenerationV1 {
    checkpoint_set_sha256: [u8; 32],
    signed_set: SccpReplayArchiveSignedCheckpointSetV1,
    sorafs_manifest_sha256: [u8; 32],
    entries: Vec<PersistedReplayHeadEntryV1>,
}

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii::sccp_replay::PersistedReplayHeadV1")]
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
struct PersistedReplayHeadV1 {
    version: u8,
    // The atomic manifest carries both generations so a crash cannot expose a
    // current head after separately losing its sole recovery generation.
    current: PersistedReplayGenerationV1,
    recovery: Option<PersistedReplayGenerationV1>,
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
    head: PersistedReplayHeadV1,
    entries: Vec<ValidatedCheckpointEntryV1>,
}

struct PublishedReplayStateV1 {
    archive: SccpReplayArchiveV1,
    checkpoints: BTreeMap<SccpReplayAccumulatorIdV1, SccpReplayArchiveSignedCheckpointV1>,
    signed_set: SccpReplayArchiveSignedCheckpointSetV1,
    checkpoint_set_sha256: [u8; 32],
}

struct CandidateReplayStateV1 {
    published: PublishedReplayStateV1,
    manifest: PersistedReplayHeadV1,
    sorafs_manifest_bytes: Vec<u8>,
    entries: Vec<ValidatedCheckpointEntryV1>,
}

#[derive(Clone, Copy)]
enum CandidateLocalValidationV1 {
    FullKuraRebuild,
    CurrentProjection,
}

fn restore_persisted_if_current(
    persisted: &PersistedReplayHeadStateV1,
    local_authority: &dyn SccpReplayLocalAuthorityV1,
) -> Result<Option<PublishedReplayStateV1>, ToriiSccpReplayStartupErrorV1> {
    let generation = &persisted.head.current;
    let coordinate = generation.signed_set.body.finality;
    let finality = iroha_sccp::SccpReplayArchiveFinalityV1 {
        network_identity_sha256: coordinate.network_identity_sha256,
        finalized_height: coordinate.finalized_height,
        finalized_block_hash: coordinate.finalized_block_hash,
    };
    let expected = persisted
        .entries
        .iter()
        .map(|entry| {
            (
                entry.snapshot.accumulator_id.clone(),
                (entry.snapshot.domain, entry.snapshot.forest.clone()),
            )
        })
        .collect();
    match local_authority.verify_current(finality, &expected) {
        Ok(()) => {}
        Err(
            SccpReplayLocalAuthorityErrorV1::Finality
            | SccpReplayLocalAuthorityErrorV1::CoreMismatch,
        ) => return Ok(None),
        Err(SccpReplayLocalAuthorityErrorV1::Rebuild) => {
            return Err(ToriiSccpReplayStartupErrorV1::LocalAuthority);
        }
    }
    let archive = local_authority
        .rebuild_persisted_and_verify(finality, &expected)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
    Ok(Some(PublishedReplayStateV1 {
        archive,
        checkpoints: persisted
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.snapshot.accumulator_id.clone(),
                    entry.checkpoint.clone(),
                )
            })
            .collect(),
        signed_set: generation.signed_set.clone(),
        checkpoint_set_sha256: generation.checkpoint_set_sha256,
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
    published_available: AtomicBool,
    published: RwLock<Arc<PublishedReplayStateV1>>,
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
        let previous = store.load_head(&config)?;
        if let Some(published) = previous
            .as_ref()
            .map(|previous| restore_persisted_if_current(previous, local_authority.as_ref()))
            .transpose()?
            .flatten()
        {
            return Ok(Arc::new(Self {
                config,
                source,
                local_authority,
                store,
                update_lock: Mutex::new(()),
                published_available: AtomicBool::new(true),
                published: RwLock::new(Arc::new(published)),
            }));
        }
        let bytes = fetch_exact_three(&config, source.as_ref(), &store)?;
        let candidate = validate_candidate(
            &config,
            bytes,
            previous.as_ref(),
            local_authority.as_ref(),
            CandidateLocalValidationV1::FullKuraRebuild,
        )?;
        store.persist_candidate(&config, &candidate)?;
        Ok(Arc::new(Self {
            config,
            source,
            local_authority,
            store,
            update_lock: Mutex::new(()),
            published_available: AtomicBool::new(true),
            published: RwLock::new(Arc::new(candidate.published)),
        }))
    }

    /// Fetch and atomically publish the current or a newer three-replica head.
    pub fn refresh(&self) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        self.refresh_inner()
    }

    fn refresh_inner(&self) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        let _guard = self
            .update_lock
            .lock()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let previous = self
            .store
            .load_head(&self.config)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        let visible_digest = self
            .published
            .read()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
            .checkpoint_set_sha256;
        if !self.published_available.load(Ordering::Acquire)
            || visible_digest != previous.head.current.checkpoint_set_sha256
        {
            self.published_available.store(false, Ordering::Release);
            if let Some(restored) =
                restore_persisted_if_current(&previous, self.local_authority.as_ref())?
            {
                *self
                    .published
                    .write()
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)? = Arc::new(restored);
                self.published_available.store(true, Ordering::Release);
            }
        }
        let bytes = fetch_exact_three(&self.config, self.source.as_ref(), &self.store)?;
        let candidate = validate_candidate(
            &self.config,
            bytes,
            Some(&previous),
            self.local_authority.as_ref(),
            CandidateLocalValidationV1::CurrentProjection,
        )?;
        // Once manifest publication begins, any ambiguous failure disables
        // serving until the durable head is reauthenticated on the next retry.
        self.published_available.store(false, Ordering::Release);
        let manifest_bytes = self
            .store
            .publish_candidate_head(&self.config, &candidate)?;
        let mut published = self
            .published
            .write()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        *published = Arc::new(candidate.published);
        self.published_available.store(true, Ordering::Release);
        drop(published);
        self.store
            .prune_to_head(&self.config, &candidate.manifest, &manifest_bytes)
    }

    /// Refresh only when the authenticated published replay inventory is stale.
    pub fn refresh_if_stale(&self) -> Result<bool, ToriiSccpReplayStartupErrorV1> {
        if self.current_published().is_ok() {
            return Ok(false);
        }
        self.refresh()?;
        Ok(true)
    }

    /// Configured delay between bounded refresh attempts.
    #[must_use]
    pub const fn refresh_interval(&self) -> Duration {
        self.config.refresh_interval
    }

    /// Digest of the exact currently visible three-replica checkpoint set.
    pub fn checkpoint_set_sha256(&self) -> Result<[u8; 32], ToriiSccpReplayEndpointErrorV1> {
        Ok(self.current_published()?.checkpoint_set_sha256)
    }

    /// Resolve a public path only within the locally authenticated complete inventory.
    pub fn accumulator_id_for_path(
        &self,
        path: &SccpReplayAccumulatorPathV1,
    ) -> Result<SccpReplayAccumulatorIdV1, ToriiSccpReplayEndpointErrorV1> {
        encode_sccp_replay_accumulator_path_v1(path)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::NotFound)?;
        let published = self.current_published()?;
        let mut matching = published
            .checkpoints
            .keys()
            .filter(|id| id.route_key == path.route_key && id.boundary == path.boundary);
        let id = matching
            .next()
            .cloned()
            .ok_or(ToriiSccpReplayEndpointErrorV1::NotFound)?;
        if matching.next().is_some() {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }
        Ok(id)
    }

    /// Return one signed root only after reauthenticating the complete
    /// checkpoint inventory against the current local Core/Kura head.
    pub fn root_response(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayRootResponseV1, ToriiSccpReplayEndpointErrorV1> {
        let published = self.current_published()?;
        let checkpoint = published
            .checkpoints
            .get(accumulator_id)
            .cloned()
            .ok_or(ToriiSccpReplayEndpointErrorV1::NotFound)?;
        verify_sccp_replay_archive_checkpoint_v1(&replica_policy(&self.config), &checkpoint)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        let (domain, forest) = SccpReplayArchiveV1::forest(&published.archive, accumulator_id)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        if domain != checkpoint.body.domain || forest != &checkpoint.body.forest {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }
        Ok(SccpReplayRootResponseV1 {
            version: 1,
            checkpoint_set_sha256: published.checkpoint_set_sha256,
            signed_set: published.signed_set.clone(),
            checkpoint,
        })
    }

    /// Return one canonical witness and its exact signed root from the same
    /// locally revalidated published head.
    pub fn witness_response(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        replay_key: [u8; 32],
    ) -> Result<SccpReplayWitnessResponseV1, ToriiSccpReplayEndpointErrorV1> {
        let published = self.current_published()?;
        let checkpoint = published
            .checkpoints
            .get(accumulator_id)
            .cloned()
            .ok_or(ToriiSccpReplayEndpointErrorV1::NotFound)?;
        verify_sccp_replay_archive_checkpoint_v1(&replica_policy(&self.config), &checkpoint)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        let (domain, forest) = SccpReplayArchiveV1::forest(&published.archive, accumulator_id)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        if domain != checkpoint.body.domain || forest != &checkpoint.body.forest {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }
        let witness = SccpReplayArchiveV1::witness(&published.archive, accumulator_id, replay_key)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        forest
            .verify_key_digest(replay_key, witness.prior_record_digest, &witness)
            .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        Ok(SccpReplayWitnessResponseV1 {
            version: 1,
            root: SccpReplayRootResponseV1 {
                version: 1,
                checkpoint_set_sha256: published.checkpoint_set_sha256,
                signed_set: published.signed_set.clone(),
                checkpoint,
            },
            replay_key,
            witness,
        })
    }

    fn current_published(
        &self,
    ) -> Result<Arc<PublishedReplayStateV1>, ToriiSccpReplayEndpointErrorV1> {
        if !self.published_available.load(Ordering::Acquire) {
            return Err(ToriiSccpReplayEndpointErrorV1::Unavailable);
        }
        let published = {
            let guard = self
                .published
                .read()
                .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
            Arc::clone(&guard)
        };
        let signed_body = verify_sccp_replay_archive_checkpoint_set_v1(
            &replica_policy(&self.config),
            &published.signed_set,
        )
        .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
        if signed_body.entries.len() != published.checkpoints.len() {
            return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
        }
        let mut expected = BTreeMap::new();
        for inventory_entry in &signed_body.entries {
            let checkpoint = published
                .checkpoints
                .get(&inventory_entry.accumulator_id)
                .ok_or(ToriiSccpReplayEndpointErrorV1::Integrity)?;
            verify_sccp_replay_archive_checkpoint_v1(&replica_policy(&self.config), checkpoint)
                .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?;
            if !signed_body
                .finality
                .matches_checkpoint(checkpoint.body.finality)
                || checkpoint.body.snapshot_sha256 != inventory_entry.snapshot_sha256
                || checkpoint
                    .body
                    .agreement_digest()
                    .map_err(|_| ToriiSccpReplayEndpointErrorV1::Integrity)?
                    != inventory_entry.checkpoint_agreement_digest
                || expected
                    .insert(
                        checkpoint.body.accumulator_id.clone(),
                        (checkpoint.body.domain, checkpoint.body.forest.clone()),
                    )
                    .is_some()
            {
                return Err(ToriiSccpReplayEndpointErrorV1::Integrity);
            }
        }
        let finality = iroha_sccp::SccpReplayArchiveFinalityV1 {
            network_identity_sha256: signed_body.finality.network_identity_sha256,
            finalized_height: signed_body.finality.finalized_height,
            finalized_block_hash: signed_body.finality.finalized_block_hash,
        };
        self.local_authority
            .verify_current(finality, &expected)
            .map_err(|error| match error {
                SccpReplayLocalAuthorityErrorV1::Finality
                | SccpReplayLocalAuthorityErrorV1::CoreMismatch => {
                    ToriiSccpReplayEndpointErrorV1::Unavailable
                }
                SccpReplayLocalAuthorityErrorV1::Rebuild => {
                    ToriiSccpReplayEndpointErrorV1::Integrity
                }
            })?;
        Ok(published)
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
        || config.refresh_interval < limits::REFRESH_INTERVAL_MIN
        || config.refresh_interval > limits::REFRESH_INTERVAL_HARD
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
        let published = self.current_published().map_err(endpoint_provider_error)?;
        SccpReplayArchiveProviderV1::forest(&published.archive, accumulator_id)
    }

    fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveProviderErrorV1> {
        let published = self.current_published().map_err(endpoint_provider_error)?;
        SccpReplayArchiveProviderV1::witness(&published.archive, accumulator_id, key)
    }

    fn checkpoint(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveProviderErrorV1> {
        self.current_published()
            .map_err(endpoint_provider_error)?
            .checkpoints
            .get(accumulator_id)
            .cloned()
            .ok_or(SccpReplayArchiveProviderErrorV1::NotFound)
    }
}

fn endpoint_provider_error(
    error: ToriiSccpReplayEndpointErrorV1,
) -> SccpReplayArchiveProviderErrorV1 {
    match error {
        ToriiSccpReplayEndpointErrorV1::Disabled | ToriiSccpReplayEndpointErrorV1::Unavailable => {
            SccpReplayArchiveProviderErrorV1::Unavailable
        }
        ToriiSccpReplayEndpointErrorV1::NotFound => SccpReplayArchiveProviderErrorV1::NotFound,
        ToriiSccpReplayEndpointErrorV1::Integrity => SccpReplayArchiveProviderErrorV1::Integrity,
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
    store: &SecureReplayStoreV1,
) -> Result<Vec<u8>, ToriiSccpReplayStartupErrorV1> {
    let files = (0..config.replicas.len())
        .map(|_| store.create_anonymous_fetch_file())
        .collect::<Result<Vec<_>, _>>()?;
    let fetched = std::thread::scope(|scope| {
        config
            .replicas
            .iter()
            .zip(files)
            .map(|replica| {
                let (replica, mut file) = replica;
                scope.spawn(move || {
                    let length = iroha_core::panic_hook::catch_unwind_suppressed(|| {
                        source.fetch_to(
                            replica,
                            config.max_response_bytes,
                            config.request_timeout,
                            &mut file,
                        )
                    })
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?;
                    file.flush()
                        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
                    validate_anonymous_fetch_file(&file, length)?;
                    Ok((file, length))
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Transport)?
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
    })?;

    let mut fetched = fetched.into_iter();
    let (mut first, expected_len) = fetched
        .next()
        .ok_or(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement)?;
    first
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let mut agreed = Vec::new();
    agreed
        .try_reserve_exact(expected_len)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    (&mut first)
        .take(
            u64::try_from(expected_len)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
                .checked_add(1)
                .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?,
        )
        .read_to_end(&mut agreed)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if agreed.len() != expected_len {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    for (mut file, length) in fetched {
        if length != expected_len {
            return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
        }
        file.seek(std::io::SeekFrom::Start(0))
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let mut offset = 0_usize;
        let mut chunk = [0_u8; 64 * 1024];
        while offset < agreed.len() {
            let width = (agreed.len() - offset).min(chunk.len());
            file.read_exact(&mut chunk[..width])
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if chunk[..width] != agreed[offset..offset + width] {
                return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
            }
            offset += width;
        }
        let mut trailing = [0_u8; 1];
        if file
            .read(&mut trailing)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?
            != 0
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
    }
    Ok(agreed)
}

fn validate_candidate(
    config: &ToriiSccpReplayArchive,
    bytes: Vec<u8>,
    previous: Option<&PersistedReplayHeadStateV1>,
    local_authority: &dyn SccpReplayLocalAuthorityV1,
    local_validation: CandidateLocalValidationV1,
) -> Result<CandidateReplayStateV1, ToriiSccpReplayStartupErrorV1> {
    if bytes.is_empty() || bytes.len() > config.max_response_bytes {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let checkpoint_set_sha256 = sccp_replay_archive_checkpoint_set_frame_sha256_v1(&bytes);
    let set: SccpReplayReplicaCheckpointSetV1 =
        norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    if norito::encode_canonical(&set).ok().as_deref() != Some(bytes.as_slice())
        || set.version != CHECKPOINT_SET_VERSION_V1
        || set.entries.len() > config.max_accumulators
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    drop(bytes);
    let policy = replica_policy(config);
    let signed_set = set.signed_set;
    let signed_body = verify_sccp_replay_archive_checkpoint_set_v1(&policy, &signed_set)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)?;
    if usize::try_from(signed_body.entry_count).ok() != Some(set.entries.len())
        || signed_body.entries.len() != set.entries.len()
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let limits = SccpReplayArchiveDecodeLimitsV1 {
        max_snapshot_bytes: config.max_snapshot_bytes,
        max_snapshot_leaves: config.max_snapshot_leaves,
    };
    let mut entries = Vec::with_capacity(set.entries.len());
    let mut inventory_entries = Vec::with_capacity(set.entries.len());
    let mut expected = BTreeMap::new();
    let mut checkpoints = BTreeMap::new();
    let mut previous_id = None;
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
        if !signed_body
            .finality
            .matches_checkpoint(validated.snapshot.finality)
        {
            return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
        }
        inventory_entries.push(
            SccpReplayArchiveCheckpointSetEntryV1::from_checkpoint(
                &validated.checkpoint,
                u64::try_from(validated.snapshot_bytes.len())
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?,
            )
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?,
        );
        entries.push(validated);
    }
    if inventory_entries != signed_body.entries {
        return Err(ToriiSccpReplayStartupErrorV1::ReplicaDisagreement);
    }
    validate_sorafs_checkpoint_manifest(
        &set.sorafs_manifest_bytes,
        &signed_body,
        entries.iter().map(|entry| entry.snapshot_bytes.as_slice()),
    )?;
    validate_continuity(
        previous.map(|previous| (&previous.head.current, previous.entries.as_slice())),
        &entries,
        &signed_set,
    )?;
    let finality = iroha_sccp::SccpReplayArchiveFinalityV1 {
        network_identity_sha256: signed_body.finality.network_identity_sha256,
        finalized_height: signed_body.finality.finalized_height,
        finalized_block_hash: signed_body.finality.finalized_block_hash,
    };
    let archive = match local_validation {
        CandidateLocalValidationV1::FullKuraRebuild => local_authority
            .rebuild_and_verify(finality, &expected)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?,
        CandidateLocalValidationV1::CurrentProjection => {
            let is_retained_exact_frame = previous.is_some_and(|previous| {
                previous.head.current.checkpoint_set_sha256 == checkpoint_set_sha256
            });
            if is_retained_exact_frame {
                local_authority.verify_current(finality, &expected)
            } else {
                local_authority.verify_candidate(finality, &expected)
            }
            .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
            let mut archive = SccpReplayArchiveV1::default();
            for entry in &entries {
                archive
                    .restore_snapshot(entry.snapshot.clone(), limits)
                    .map_err(|_| ToriiSccpReplayStartupErrorV1::LocalAuthority)?;
            }
            archive
        }
    };
    let manifest_entries = entries
        .iter()
        .map(|entry| PersistedReplayHeadEntryV1 {
            accumulator_id: entry.snapshot.accumulator_id.clone(),
            snapshot_sha256: entry.checkpoint.body.snapshot_sha256,
            checkpoint_agreement_digest: entry.checkpoint_agreement_digest,
            checkpoint_sha256: entry.checkpoint_sha256,
        })
        .collect();
    let current = PersistedReplayGenerationV1 {
        checkpoint_set_sha256,
        signed_set: signed_set.clone(),
        sorafs_manifest_sha256: signed_body.sorafs_manifest.manifest_sha256,
        entries: manifest_entries,
    };
    let recovery = previous.and_then(|previous| {
        if previous.head.current == current {
            previous.head.recovery.clone()
        } else {
            Some(previous.head.current.clone())
        }
    });
    Ok(CandidateReplayStateV1 {
        published: PublishedReplayStateV1 {
            archive,
            checkpoints,
            signed_set,
            checkpoint_set_sha256,
        },
        manifest: PersistedReplayHeadV1 {
            version: HEAD_MANIFEST_VERSION_V1,
            current,
            recovery,
        },
        sorafs_manifest_bytes: set.sorafs_manifest_bytes,
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

fn canonical_sorafs_checkpoint_manifest<I, B>(
    finality: SccpReplayArchiveHeadFinalityV1,
    inventory_sha256: [u8; 32],
    snapshot_total_bytes: u64,
    snapshot_bytes: I,
) -> Result<ManifestV1, ToriiSccpReplayStartupErrorV1>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let expected_len = usize::try_from(snapshot_total_bytes)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(expected_len)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    for snapshot in snapshot_bytes {
        let snapshot = snapshot.as_ref();
        payload
            .len()
            .checked_add(snapshot.len())
            .filter(|length| *length <= expected_len)
            .ok_or(ToriiSccpReplayStartupErrorV1::Malformed)?;
        payload.extend_from_slice(snapshot);
    }
    if payload.len() != expected_len {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let plan = if payload.is_empty() {
        CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: blake3::hash(&payload),
            content_length: 0,
            chunks: Vec::new(),
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 0,
                size: 0,
            }],
        }
    } else {
        CarBuildPlan::single_file(&payload).map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?
    };
    let writer =
        CarWriter::new(&plan, &payload).map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let stats = writer
        .write_to(std::io::sink())
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let [root_cid] = stats.root_cids.as_slice() else {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    };
    let por_root =
        compute_por_root(&payload, &plan).map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    ManifestBuilder::new()
        .root_cid(root_cid.clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(por_root)
        .content_length(snapshot_total_bytes)
        .car_digest(stats.car_archive_digest.into())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 3,
            storage_class: StorageClass::Hot,
            retention_epoch: finality.finalized_height,
        })
        .add_metadata(
            SORAFS_INVENTORY_METADATA_KEY_V1,
            hex::encode(inventory_sha256),
        )
        .build()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)
}

fn validate_sorafs_checkpoint_manifest<I, B>(
    bytes: &[u8],
    body: &iroha_sccp::SccpReplayArchiveCheckpointSetBodyV1,
    snapshot_bytes: I,
) -> Result<(), ToriiSccpReplayStartupErrorV1>
where
    I: IntoIterator<Item = B>,
    B: AsRef<[u8]>,
{
    let binding = body.sorafs_manifest;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).ok() != Some(binding.manifest_size_bytes)
        || sha256(&[bytes]) != binding.manifest_sha256
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    let manifest = decode_manifest_v1_canonical(bytes)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let constraints = SorafsPinPolicyConstraints {
        min_replicas_floor: 3,
        ..SorafsPinPolicyConstraints::default()
    };
    validate_sorafs_manifest(&manifest, &constraints)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Malformed)?;
    let expected = canonical_sorafs_checkpoint_manifest(
        body.finality,
        body.inventory_sha256,
        binding.snapshot_total_bytes,
        snapshot_bytes,
    )?;
    if manifest != expected
        || manifest.root_cid.as_slice() != binding.manifest_root_cid
        || manifest.content_length != binding.snapshot_total_bytes
    {
        return Err(ToriiSccpReplayStartupErrorV1::Malformed);
    }
    Ok(())
}

fn validate_continuity(
    previous: Option<(&PersistedReplayGenerationV1, &[ValidatedCheckpointEntryV1])>,
    entries: &[ValidatedCheckpointEntryV1],
    signed_set: &SccpReplayArchiveSignedCheckpointSetV1,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let Some((previous_generation, previous_entries)) = previous else {
        // The local Core/Kura rebuild below authenticates the complete current
        // inventory. A fresh archive replica may therefore begin at any
        // retained finalized head; requiring a genesis predecessor here would
        // make first startup after pruning impossible.
        return Ok(());
    };
    if previous_generation.signed_set == *signed_set
        && previous_entries.len() == entries.len()
        && previous_entries
            .iter()
            .zip(entries)
            .all(|(prior, current)| {
                prior.snapshot_bytes == current.snapshot_bytes
                    && prior.checkpoint_bytes == current.checkpoint_bytes
            })
    {
        // Restart and polling are idempotent for the exact already-persisted
        // signed head. The local authority is still re-run before publication.
        return Ok(());
    }
    let finality = signed_set.body.finality;
    let previous_finality = previous_generation.signed_set.body.finality;
    if finality.network_identity_sha256 != previous_finality.network_identity_sha256
        || finality.finalized_height <= previous_finality.finalized_height
        || finality.finalized_block_hash == previous_finality.finalized_block_hash
    {
        return Err(ToriiSccpReplayStartupErrorV1::Continuity);
    }
    let current = entries
        .iter()
        .map(|entry| (entry.snapshot.accumulator_id.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for prior in previous_entries {
        let Some(next) = current.get(&prior.snapshot.accumulator_id) else {
            if prior.snapshot.forest != SccpReplayForestV1::default()
                || !prior.snapshot.leaves.is_empty()
            {
                return Err(ToriiSccpReplayStartupErrorV1::Continuity);
            }
            // Core permits deletion only for quiescent routes. Dropping an
            // authenticated empty accumulator mirrors that registry removal;
            // any occupied history remains permanently non-removable.
            continue;
        };
        if next.snapshot.domain != prior.snapshot.domain
            || next.snapshot.forest.leaf_count < prior.snapshot.forest.leaf_count
            || next.snapshot.forest.update_sequence < prior.snapshot.forest.update_sequence
            || !snapshot_contains_prior_leaves(&prior.snapshot, &next.snapshot)
        {
            return Err(ToriiSccpReplayStartupErrorV1::Continuity);
        }
    }
    // A replica may legitimately skip intermediate signed snapshots. Exact
    // predecessor equality is therefore neither required for existing
    // accumulators nor assumed for an accumulator first observed locally.
    // Monotonic forest state plus the current Core/Kura rebuild above closes
    // rollback and fork acceptance without imposing archive-history liveness.
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
            .checked_mul(1_024)
            .and_then(|bytes| bytes.checked_add(8_192))
            .unwrap_or(usize::MAX)
            .min(config.max_response_bytes)
    }

    fn create_anonymous_fetch_file(&self) -> Result<File, ToriiSccpReplayStartupErrorV1> {
        create_anonymous_fetch_file(&self.directory)
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
            secure_prune_headless_replay_store(
                &self.directory,
                config,
                Self::manifest_limit(config),
            )?;
            return Ok(None);
        };
        let head: PersistedReplayHeadV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if norito::encode_canonical(&head).ok().as_deref() != Some(bytes.as_slice())
            || head.version != HEAD_MANIFEST_VERSION_V1
            || head
                .recovery
                .as_ref()
                .is_some_and(|recovery| recovery == &head.current)
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let entries = self.load_generation(config, &head.current)?;
        if let Some(recovery) = &head.recovery {
            let recovery_entries = self.load_generation(config, recovery)?;
            validate_continuity(
                Some((recovery, recovery_entries.as_slice())),
                &entries,
                &head.current.signed_set,
            )
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        }
        self.prune_to_head(config, &head, &bytes)?;
        Ok(Some(PersistedReplayHeadStateV1 { head, entries }))
    }

    fn load_generation(
        &self,
        config: &ToriiSccpReplayArchive,
        generation: &PersistedReplayGenerationV1,
    ) -> Result<Vec<ValidatedCheckpointEntryV1>, ToriiSccpReplayStartupErrorV1> {
        if generation.checkpoint_set_sha256 == [0; 32]
            || generation.entries.len() > config.max_accumulators
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let policy = replica_policy(config);
        let signed_body =
            verify_sccp_replay_archive_checkpoint_set_v1(&policy, &generation.signed_set)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if usize::try_from(signed_body.entry_count).ok() != Some(generation.entries.len())
            || signed_body.entries.len() != generation.entries.len()
            || signed_body.sorafs_manifest.manifest_sha256 != generation.sorafs_manifest_sha256
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let sorafs_manifest_bytes = secure_read_relative(
            &self.directory,
            &sorafs_manifest_filename(generation.sorafs_manifest_sha256),
            sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES,
        )?
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        let limits = SccpReplayArchiveDecodeLimitsV1 {
            max_snapshot_bytes: config.max_snapshot_bytes,
            max_snapshot_leaves: config.max_snapshot_leaves,
        };
        let mut entries = Vec::with_capacity(generation.entries.len());
        let mut wire_entries = Vec::with_capacity(generation.entries.len());
        let mut previous_id = None;
        for entry_manifest in &generation.entries {
            if previous_id
                .as_ref()
                .is_some_and(|prior| prior >= &entry_manifest.accumulator_id)
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            previous_id = Some(entry_manifest.accumulator_id.clone());
            let snapshot_name = snapshot_filename(entry_manifest.snapshot_sha256);
            let checkpoint_name = checkpoint_filename(entry_manifest.checkpoint_sha256);
            let snapshot_bytes =
                secure_read_relative(&self.directory, &snapshot_name, config.max_snapshot_bytes)?
                    .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            let checkpoint_bytes = secure_read_relative(
                &self.directory,
                &checkpoint_name,
                MAX_PERSISTED_CHECKPOINT_BYTES_V1,
            )?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
            if sha256(&[&snapshot_bytes]) != entry_manifest.snapshot_sha256
                || sha256(&[&checkpoint_bytes]) != entry_manifest.checkpoint_sha256
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            let checkpoint: SccpReplayArchiveSignedCheckpointV1 =
                norito::decode_canonical_with_limits(
                    &checkpoint_bytes,
                    norito::canonical_decode_limits(checkpoint_bytes.len()),
                )
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if norito::encode_canonical(&checkpoint).ok().as_deref()
                != Some(checkpoint_bytes.as_slice())
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            let entry = validate_entry(checkpoint.clone(), snapshot_bytes.clone(), &policy, limits)
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
            if entry.snapshot.accumulator_id != entry_manifest.accumulator_id
                || entry.checkpoint_agreement_digest != entry_manifest.checkpoint_agreement_digest
                || entry.checkpoint_sha256 != entry_manifest.checkpoint_sha256
                || entry.checkpoint.body.snapshot_sha256 != entry_manifest.snapshot_sha256
                || !signed_body
                    .finality
                    .matches_checkpoint(entry.snapshot.finality)
            {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            wire_entries.push(SccpReplayReplicaCheckpointEntryV1 {
                checkpoint,
                snapshot_bytes,
            });
            entries.push(entry);
        }
        let inventory_entries = entries
            .iter()
            .map(|entry| {
                SccpReplayArchiveCheckpointSetEntryV1::from_checkpoint(
                    &entry.checkpoint,
                    u64::try_from(entry.snapshot_bytes.len())
                        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
                )
                .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if inventory_entries != signed_body.entries {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        validate_sorafs_checkpoint_manifest(
            &sorafs_manifest_bytes,
            &signed_body,
            entries.iter().map(|entry| entry.snapshot_bytes.as_slice()),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let wire = SccpReplayReplicaCheckpointSetV1 {
            version: CHECKPOINT_SET_VERSION_V1,
            signed_set: generation.signed_set.clone(),
            sorafs_manifest_bytes,
            entries: wire_entries,
        };
        let wire_bytes = norito::encode_canonical(&wire)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        if sccp_replay_archive_checkpoint_set_frame_sha256_v1(&wire_bytes)
            != generation.checkpoint_set_sha256
        {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        Ok(entries)
    }

    fn persist_candidate(
        &self,
        config: &ToriiSccpReplayArchive,
        candidate: &CandidateReplayStateV1,
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        // Immutable artifacts and the current+recovery manifest are fully
        // durable before any name belonging only to an older generation is
        // considered for deletion. A GC error therefore leaves a valid head
        // and can be retried by `load_head` without rolling it back.
        let manifest_bytes = self.publish_candidate_head(config, candidate)?;
        self.prune_to_head(config, &candidate.manifest, &manifest_bytes)
    }

    fn publish_candidate_head(
        &self,
        config: &ToriiSccpReplayArchive,
        candidate: &CandidateReplayStateV1,
    ) -> Result<Vec<u8>, ToriiSccpReplayStartupErrorV1> {
        secure_write_immutable_relative(
            &self.directory,
            &sorafs_manifest_filename(candidate.manifest.current.sorafs_manifest_sha256),
            &candidate.sorafs_manifest_bytes,
            sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES,
        )?;
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
        let manifest_bytes = norito::encode_canonical(&candidate.manifest)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        secure_write_manifest_last_relative(
            &self.directory,
            HEAD_MANIFEST_FILENAME_V1,
            &manifest_bytes,
            Self::manifest_limit(config),
        )?;
        Ok(manifest_bytes)
    }

    fn prune_to_head(
        &self,
        config: &ToriiSccpReplayArchive,
        head: &PersistedReplayHeadV1,
        head_bytes: &[u8],
    ) -> Result<(), ToriiSccpReplayStartupErrorV1> {
        secure_prune_replay_store(
            &self.directory,
            config,
            head,
            head_bytes,
            Self::manifest_limit(config),
        )
    }
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum ReplayStoreArtifactKindV1 {
    Snapshot,
    Checkpoint,
    SorafsManifest,
}

#[cfg(unix)]
fn replay_store_artifact_kind(name: &str) -> Option<ReplayStoreArtifactKindV1> {
    let (prefix, kind) = if name.starts_with("snapshot-") {
        ("snapshot-", ReplayStoreArtifactKindV1::Snapshot)
    } else if name.starts_with("checkpoint-") {
        ("checkpoint-", ReplayStoreArtifactKindV1::Checkpoint)
    } else if name.starts_with("sorafs-manifest-") {
        (
            "sorafs-manifest-",
            ReplayStoreArtifactKindV1::SorafsManifest,
        )
    } else {
        return None;
    };
    let digest = name.strip_prefix(prefix)?.strip_suffix(".norito")?;
    (digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    .then_some(kind)
}

#[cfg(unix)]
fn replay_store_generated_base_name(name: &str, suffix: &str) -> Option<String> {
    let body = name.strip_prefix('.')?.strip_suffix(suffix)?;
    let (base, nonce) = body.rsplit_once('.')?;
    if nonce.len() != 32
        || !nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || (base != HEAD_MANIFEST_FILENAME_V1
            && base != REPLICA_FETCH_TEMP_FILENAME_V1
            && replay_store_artifact_kind(base).is_none())
    {
        return None;
    }
    Some(base.to_owned())
}

#[cfg(unix)]
fn replay_store_temporary_base_name(name: &str) -> Option<String> {
    replay_store_generated_base_name(name, ".tmp")
}

#[cfg(unix)]
fn replay_store_quarantine_base_name(name: &str) -> Option<String> {
    replay_store_generated_base_name(name, ".gc")
}

#[cfg(unix)]
fn replay_store_artifact_limit(
    config: &ToriiSccpReplayArchive,
    base: &str,
    manifest_limit: usize,
) -> Option<usize> {
    if base == HEAD_MANIFEST_FILENAME_V1 {
        return Some(manifest_limit);
    }
    if base == REPLICA_FETCH_TEMP_FILENAME_V1 {
        return Some(config.max_response_bytes);
    }
    match replay_store_artifact_kind(base)? {
        ReplayStoreArtifactKindV1::Snapshot => Some(config.max_snapshot_bytes),
        ReplayStoreArtifactKindV1::Checkpoint => Some(MAX_PERSISTED_CHECKPOINT_BYTES_V1),
        ReplayStoreArtifactKindV1::SorafsManifest => {
            Some(sorafs_manifest::MAX_MANIFEST_ENCODED_BYTES)
        }
    }
}

#[cfg(unix)]
fn replay_store_retained_names(head: &PersistedReplayHeadV1) -> BTreeSet<String> {
    let mut retained = BTreeSet::new();
    for generation in core::iter::once(&head.current).chain(head.recovery.iter()) {
        retained.insert(sorafs_manifest_filename(generation.sorafs_manifest_sha256));
        for entry in &generation.entries {
            retained.insert(snapshot_filename(entry.snapshot_sha256));
            retained.insert(checkpoint_filename(entry.checkpoint_sha256));
        }
    }
    retained
}

#[cfg(unix)]
fn replay_store_directory_names(
    directory: &File,
    max_entries: usize,
) -> Result<Vec<String>, ToriiSccpReplayStartupErrorV1> {
    let entries = rustix::fs::Dir::read_from(directory)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let raw = entry.file_name().to_bytes();
        if matches!(raw, b"." | b"..") {
            continue;
        }
        if names.len() >= max_entries {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        let name =
            core::str::from_utf8(raw).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        names.push(name.to_owned());
    }
    names.sort_unstable();
    Ok(names)
}

#[cfg(unix)]
fn replay_store_scan_limit(max_accumulators: usize) -> Option<usize> {
    // Before post-publication GC there may be three complete generations:
    // current, recovery, and the newly published candidate. At most one
    // interrupted temporary and one quarantine name can coexist with the
    // durable head and process lock in a single-writer store.
    max_accumulators
        .checked_mul(6)
        .and_then(|artifacts| artifacts.checked_add(7))
}

#[cfg(unix)]
fn secure_prune_replay_store_name(
    directory: &File,
    source_name: &str,
    base_name: &str,
    max_bytes: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    let before = rustix::fs::statat(
        directory,
        source_name,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let max_bytes =
        u64::try_from(max_bytes).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::RegularFile
        || before.st_uid != rustix::process::geteuid().as_raw()
        || before.st_mode & 0o777 != 0o600
        || before.st_nlink != 1
        || before.st_size < 0
        || u64::try_from(before.st_size)
            .ok()
            .is_none_or(|size| size > max_bytes)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    let opened = File::from(
        rustix::fs::openat(
            directory,
            source_name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
    );
    let metadata = opened
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || metadata.dev() != u64::try_from(before.st_dev).unwrap_or(u64::MAX)
        || metadata.ino() != u64::try_from(before.st_ino).unwrap_or(u64::MAX)
        || metadata.len() != u64::try_from(before.st_size).unwrap_or(u64::MAX)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }

    let quarantine_name = (0..SECURE_TEMP_RETRIES_V1)
        .find_map(|_| {
            let nonce: [u8; 16] = rand::random();
            let candidate = format!(".{base_name}.{}.gc", hex::encode(nonce));
            match rustix::fs::renameat_with(
                directory,
                source_name,
                directory,
                candidate.as_str(),
                rustix::fs::RenameFlags::NOREPLACE,
            ) {
                Ok(()) => Some(Ok(candidate)),
                Err(rustix::io::Errno::EXIST) => None,
                Err(_) => Some(Err(ToriiSccpReplayStartupErrorV1::Persistence)),
            }
        })
        .transpose()?
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
    rebind_published_file(
        directory,
        &quarantine_name,
        &opened,
        usize::try_from(metadata.len()).map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?,
    )?;
    if !matches!(
        rustix::fs::statat(
            directory,
            source_name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ),
        Err(rustix::io::Errno::NOENT)
    ) {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    rustix::fs::unlinkat(
        directory,
        quarantine_name.as_str(),
        rustix::fs::AtFlags::empty(),
    )
    .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !matches!(
        rustix::fs::statat(
            directory,
            quarantine_name.as_str(),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ),
        Err(rustix::io::Errno::NOENT)
    ) {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    directory
        .sync_all()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)
}

#[cfg(unix)]
fn secure_prune_replay_store(
    directory: &File,
    config: &ToriiSccpReplayArchive,
    head: &PersistedReplayHeadV1,
    head_bytes: &[u8],
    manifest_limit: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let retained = replay_store_retained_names(head);
    let scan_limit = replay_store_scan_limit(config.max_accumulators)
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
    for name in replay_store_directory_names(directory, scan_limit)? {
        if matches!(
            name.as_str(),
            HEAD_MANIFEST_FILENAME_V1 | PROCESS_LOCK_FILENAME_V1
        ) || retained.contains(&name)
        {
            continue;
        }
        let base = if replay_store_artifact_kind(&name).is_some() {
            name.clone()
        } else if let Some(base) = replay_store_temporary_base_name(&name) {
            base
        } else if let Some(base) = replay_store_quarantine_base_name(&name) {
            if retained.contains(&base) {
                return Err(ToriiSccpReplayStartupErrorV1::Persistence);
            }
            base
        } else {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        };
        let limit = replay_store_artifact_limit(config, &base, manifest_limit)
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        secure_prune_replay_store_name(directory, &name, &base, limit)?;
    }

    let expected = retained
        .iter()
        .cloned()
        .chain([
            HEAD_MANIFEST_FILENAME_V1.to_owned(),
            PROCESS_LOCK_FILENAME_V1.to_owned(),
        ])
        .collect::<BTreeSet<_>>();
    let actual = replay_store_directory_names(directory, scan_limit)?
        .into_iter()
        .collect::<BTreeSet<_>>();
    if actual != expected
        || secure_read_relative(directory, HEAD_MANIFEST_FILENAME_V1, manifest_limit)?.as_deref()
            != Some(head_bytes)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    directory
        .sync_all()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)
}

#[cfg(unix)]
fn secure_prune_headless_replay_store(
    directory: &File,
    config: &ToriiSccpReplayArchive,
    manifest_limit: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    let scan_limit = replay_store_scan_limit(config.max_accumulators)
        .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
    for name in replay_store_directory_names(directory, scan_limit)? {
        if name == PROCESS_LOCK_FILENAME_V1 {
            continue;
        }
        let base = if replay_store_artifact_kind(&name).is_some() {
            name.clone()
        } else if let Some(base) = replay_store_temporary_base_name(&name) {
            base
        } else if let Some(base) = replay_store_quarantine_base_name(&name) {
            base
        } else {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        };
        let limit = replay_store_artifact_limit(config, &base, manifest_limit)
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        secure_prune_replay_store_name(directory, &name, &base, limit)?;
    }
    if replay_store_directory_names(directory, scan_limit)?
        != vec![PROCESS_LOCK_FILENAME_V1.to_owned()]
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    directory
        .sync_all()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)
}

#[cfg(not(unix))]
fn secure_prune_replay_store(
    _directory: &File,
    _config: &ToriiSccpReplayArchive,
    _head: &PersistedReplayHeadV1,
    _head_bytes: &[u8],
    _manifest_limit: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(not(unix))]
fn secure_prune_headless_replay_store(
    _directory: &File,
    _config: &ToriiSccpReplayArchive,
    _manifest_limit: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

fn snapshot_filename(digest: [u8; 32]) -> String {
    format!("snapshot-{}.norito", hex::encode(digest))
}

fn checkpoint_filename(digest: [u8; 32]) -> String {
    format!("checkpoint-{}.norito", hex::encode(digest))
}

fn sorafs_manifest_filename(digest: [u8; 32]) -> String {
    format!("sorafs-manifest-{}.norito", hex::encode(digest))
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
            || (is_final && (after.st_uid != effective_uid || after.st_mode & 0o777 != 0o700))
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
        || named.st_uid != rustix::process::geteuid().as_raw()
        || named.st_mode & 0o777 != 0o600
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
        || opened.mode() & 0o777 != 0o600
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
    let opened_after = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if rustix::fs::FileType::from_raw_mode(after.st_mode) != rustix::fs::FileType::RegularFile
        || after.st_uid != rustix::process::geteuid().as_raw()
        || after.st_mode & 0o777 != 0o600
        || after.st_nlink != 1
        || !opened_after.is_file()
        || opened_after.uid() != rustix::process::geteuid().as_raw()
        || opened_after.mode() & 0o777 != 0o600
        || opened_after.nlink() != 1
        || after.st_dev != before.st_dev
        || after.st_ino != before.st_ino
        || after.st_size != before.st_size
        || after.st_mtime != before.st_mtime
        || after.st_mtime_nsec != before.st_mtime_nsec
        || after.st_ctime != before.st_ctime
        || after.st_ctime_nsec != before.st_ctime_nsec
        || u64::try_from(after.st_dev).ok() != Some(opened_after.dev())
        || u64::try_from(after.st_ino).ok() != Some(opened_after.ino())
        || after.st_size < 0
        || u64::try_from(after.st_size).ok() != Some(opened_after.len())
        || opened_after.len() != opened.len()
        || opened_after.modified().ok() != opened.modified().ok()
        || u64::try_from(bytes.len()).ok() != Some(opened_after.len())
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
        validate_unpublished_file(&temporary, bytes.len())?;
        publish_noreplace(directory, &temporary_name, name)?;
        rebind_published_file(directory, name, &temporary, bytes.len())?;
        directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let readback = secure_read_relative(directory, name, max_bytes)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        if readback != bytes {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        rebind_published_file(directory, name, &temporary, bytes.len())
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
        validate_unpublished_file(&temporary, bytes.len())?;
        rustix::fs::renameat(directory, temporary_name.as_str(), directory, name)
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        rebind_published_file(directory, name, &temporary, bytes.len())?;
        directory
            .sync_all()
            .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
        let readback = secure_read_relative(directory, name, max_bytes)?
            .ok_or(ToriiSccpReplayStartupErrorV1::Persistence)?;
        if readback != bytes {
            return Err(ToriiSccpReplayStartupErrorV1::Persistence);
        }
        rebind_published_file(directory, name, &temporary, bytes.len())
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
fn create_anonymous_fetch_file(directory: &File) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    let (file, name) = create_secure_temporary(directory, REPLICA_FETCH_TEMP_FILENAME_V1)?;
    rustix::fs::unlinkat(directory, name.as_str(), rustix::fs::AtFlags::empty())
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    directory
        .sync_all()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    validate_anonymous_fetch_file(&file, 0)?;
    Ok(file)
}

#[cfg(not(unix))]
fn create_anonymous_fetch_file(_directory: &File) -> Result<File, ToriiSccpReplayStartupErrorV1> {
    Err(ToriiSccpReplayStartupErrorV1::UnsupportedPlatform)
}

#[cfg(unix)]
fn validate_anonymous_fetch_file(
    file: &File,
    expected_len: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 0
        || usize::try_from(metadata.len()).ok() != Some(expected_len)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_anonymous_fetch_file(
    _file: &File,
    _expected_len: usize,
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
            rustix::fs::OFlags::RDWR
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
fn validate_unpublished_file(
    file: &File,
    expected_len: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o777 != 0o600
        || metadata.nlink() != 1
        || usize::try_from(metadata.len()).ok() != Some(expected_len)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(())
}

#[cfg(unix)]
fn rebind_published_file(
    directory: &File,
    name: &str,
    file: &File,
    expected_len: usize,
) -> Result<(), ToriiSccpReplayStartupErrorV1> {
    use std::os::unix::fs::MetadataExt as _;

    let opened = file
        .metadata()
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    let named = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| ToriiSccpReplayStartupErrorV1::Persistence)?;
    if !opened.is_file()
        || opened.uid() != rustix::process::geteuid().as_raw()
        || opened.mode() & 0o777 != 0o600
        || opened.nlink() != 1
        || usize::try_from(opened.len()).ok() != Some(expected_len)
        || rustix::fs::FileType::from_raw_mode(named.st_mode) != rustix::fs::FileType::RegularFile
        || named.st_uid != rustix::process::geteuid().as_raw()
        || named.st_mode & 0o777 != 0o600
        || named.st_nlink != 1
        || u64::try_from(named.st_dev).ok() != Some(opened.dev())
        || u64::try_from(named.st_ino).ok() != Some(opened.ino())
        || usize::try_from(named.st_size).ok() != Some(expected_len)
    {
        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
    }
    Ok(())
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
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::bridge::{
        SccpLaneIdV1, SccpNetworkV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayForestV1,
        SccpRouteKeyV1,
    };
    use iroha_sccp::{
        SccpReplayArchiveCheckpointSetBodyV1, SccpReplayArchiveFinalityV1,
        SccpReplayArchiveReplicaAttestationV1, SccpReplayArchiveSorafsManifestV1,
        sccp_replay_archive_checkpoint_set_inventory_sha256_v1,
        sccp_replay_archive_checkpoint_set_signing_message_v1,
        sccp_replay_archive_checkpoint_signing_message_v1,
    };
    use tempfile::TempDir;
    use url::Url;

    use super::*;

    #[test]
    fn public_replay_paths_are_unique_and_only_expose_sora_boundaries() {
        for boundary in [
            SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
            SCCP_REPLAY_SORA_INBOUND_RELEASE_PATH_V1,
        ] {
            let id = decode_sccp_replay_accumulator_path_v1(
                boundary,
                "ethereum-mainnet",
                "taira_xor",
                "xor",
                "7",
            )
            .expect("canonical SORA accumulator path decodes");
            assert_eq!(
                encode_sccp_replay_accumulator_path_v1(&id),
                Ok([
                    boundary.to_owned(),
                    "ethereum-mainnet".to_owned(),
                    "taira_xor".to_owned(),
                    "xor".to_owned(),
                    "7".to_owned(),
                ])
            );
        }

        for (boundary, network, route, asset, revision) in [
            (
                "evm-source-burn",
                "ethereum-mainnet",
                "taira_xor",
                "xor",
                "7",
            ),
            (
                SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
                "sora-taira",
                "taira_xor",
                "xor",
                "7",
            ),
            (
                SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
                "Ethereum-mainnet",
                "taira_xor",
                "xor",
                "7",
            ),
            (
                SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
                "ethereum-mainnet",
                "Taira_xor",
                "xor",
                "7",
            ),
            (
                SCCP_REPLAY_SORA_OUTBOUND_LOCK_PATH_V1,
                "ethereum-mainnet",
                "taira_xor",
                "xor",
                "07",
            ),
        ] {
            assert_eq!(
                decode_sccp_replay_accumulator_path_v1(boundary, network, route, asset, revision,),
                Err(SccpReplayPathErrorV1::Malformed)
            );
        }

        let external_id = SccpReplayAccumulatorPathV1 {
            route_key: SccpRouteKeyV1::new(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                "taira_xor".to_owned(),
                "xor".to_owned(),
                7,
            )
            .expect("fixture route is canonical"),
            boundary: SccpReplayBoundaryV1::EvmSourceBurn,
        };
        assert_eq!(
            encode_sccp_replay_accumulator_path_v1(&external_id),
            Err(SccpReplayPathErrorV1::Malformed)
        );
    }

    #[test]
    fn replay_key_path_accepts_zero_and_rejects_alternate_hex_encodings() {
        assert_eq!(decode_sccp_replay_key_path_v1(&"0".repeat(64)), Ok([0; 32]));
        assert_eq!(
            decode_sccp_replay_key_path_v1(&"ab".repeat(32)),
            Ok([0xAB; 32])
        );
        for invalid in [
            "0".repeat(63),
            "0".repeat(65),
            format!("0x{}", "0".repeat(64)),
            "AB".repeat(32),
            format!("{}g", "0".repeat(63)),
        ] {
            assert_eq!(
                decode_sccp_replay_key_path_v1(&invalid),
                Err(SccpReplayPathErrorV1::Malformed)
            );
        }
    }

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
        fn fetch_to(
            &self,
            replica: &ToriiSccpReplayArchiveReplica,
            max_response_bytes: usize,
            _timeout: Duration,
            destination: &mut dyn std::io::Write,
        ) -> Result<usize, SccpReplayCheckpointSourceErrorV1> {
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
            destination
                .write_all(&bytes)
                .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
            Ok(bytes.len())
        }
    }

    #[derive(Default)]
    struct EmptyForestLocalAuthority;

    impl SccpReplayLocalAuthorityV1 for EmptyForestLocalAuthority {
        fn verify_candidate(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            self.verify_current(finality, expected)
        }

        fn rebuild_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            if finality.finalized_height == 0
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
        fn verify_candidate(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            self.verify_current(finality, expected)
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

    #[derive(Default)]
    struct CountingLocalAuthority {
        full_rebuilds: AtomicUsize,
        current_projection_checks: AtomicUsize,
    }

    impl SccpReplayLocalAuthorityV1 for CountingLocalAuthority {
        fn verify_candidate(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            self.verify_current(finality, expected)
        }

        fn rebuild_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            self.full_rebuilds.fetch_add(1, Ordering::Relaxed);
            EmptyForestLocalAuthority.rebuild_and_verify(finality, expected)
        }

        fn verify_current(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            self.current_projection_checks
                .fetch_add(1, Ordering::Relaxed);
            EmptyForestLocalAuthority
                .rebuild_and_verify(finality, expected)
                .map(|_| ())
        }
    }

    struct RevocableLocalAuthority {
        current: AtomicBool,
    }

    impl RevocableLocalAuthority {
        fn new() -> Self {
            Self {
                current: AtomicBool::new(true),
            }
        }
    }

    impl SccpReplayLocalAuthorityV1 for RevocableLocalAuthority {
        fn verify_candidate(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            self.verify_current(finality, expected)
        }

        fn rebuild_and_verify(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
            EmptyForestLocalAuthority.rebuild_and_verify(finality, expected)
        }

        fn verify_current(
            &self,
            finality: SccpReplayArchiveFinalityV1,
            expected: &BTreeMap<
                SccpReplayAccumulatorIdV1,
                (SccpReplayDomainV1, SccpReplayForestV1),
            >,
        ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
            if !self.current.load(Ordering::Acquire) {
                return Err(SccpReplayLocalAuthorityErrorV1::Finality);
            }
            EmptyForestLocalAuthority
                .rebuild_and_verify(finality, expected)
                .map(|_| ())
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
                refresh_interval: Duration::from_secs(1),
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
                },
            );
            let first_snapshot_sha256 = first_snapshot
                .content_sha256()
                .expect("snapshot content hash is defined");
            let first_bytes = checkpoint_set_bytes(&first_snapshot, &key_pairs, &config.replicas);
            let source = Arc::new(MutableSource::default());
            source.set_all(&config.replicas, &first_bytes);
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

        fn bytes_at(&self, height: u64, block_hash: [u8; 32]) -> (Vec<u8>, [u8; 32]) {
            self.bytes_at_with_domain(height, block_hash, self.domain)
        }

        fn bytes_at_with_domain(
            &self,
            height: u64,
            block_hash: [u8; 32],
            domain: SccpReplayDomainV1,
        ) -> (Vec<u8>, [u8; 32]) {
            let snapshot = snapshot(
                &self.accumulator_id,
                domain,
                SccpReplayArchiveFinalityV1 {
                    network_identity_sha256: [0x91; 32],
                    finalized_height: height,
                    finalized_block_hash: block_hash,
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
        checkpoint_set_bytes_for_snapshots(
            core::slice::from_ref(snapshot),
            snapshot.finality.into(),
            key_pairs,
            replicas,
        )
    }

    fn checkpoint_set_bytes_for_snapshots(
        snapshots: &[SccpReplayArchiveSnapshotV1],
        finality: SccpReplayArchiveHeadFinalityV1,
        key_pairs: &[KeyPair; 3],
        replicas: &[ToriiSccpReplayArchiveReplica; 3],
    ) -> Vec<u8> {
        let mut snapshots = snapshots.iter().collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.accumulator_id.cmp(&right.accumulator_id));
        let entries: Vec<_> = snapshots
            .into_iter()
            .map(|snapshot| {
                let body = SccpReplayArchiveCheckpointBodyV1::from_snapshot(snapshot)
                    .expect("valid snapshot produces a checkpoint");
                let message = sccp_replay_archive_checkpoint_signing_message_v1(&body)
                    .expect("checkpoint signing message is defined");
                let attestations = sign_attestations(message, key_pairs, replicas);
                SccpReplayReplicaCheckpointEntryV1 {
                    checkpoint: SccpReplayArchiveSignedCheckpointV1 { body, attestations },
                    snapshot_bytes: norito::encode_canonical(snapshot).expect("snapshot encodes"),
                }
            })
            .collect::<Vec<_>>();
        let inventory_entries = entries
            .iter()
            .map(|entry| {
                SccpReplayArchiveCheckpointSetEntryV1::from_checkpoint(
                    &entry.checkpoint,
                    u64::try_from(entry.snapshot_bytes.len()).expect("fixture size fits u64"),
                )
                .expect("valid checkpoint has one inventory entry")
            })
            .collect::<Vec<_>>();
        let inventory_sha256 =
            sccp_replay_archive_checkpoint_set_inventory_sha256_v1(finality, &inventory_entries)
                .expect("fixture inventory hashes");
        let snapshot_total_bytes = inventory_entries
            .iter()
            .map(|entry| entry.snapshot_size_bytes)
            .sum::<u64>();
        let manifest = canonical_sorafs_checkpoint_manifest(
            finality,
            inventory_sha256,
            snapshot_total_bytes,
            entries.iter().map(|entry| entry.snapshot_bytes.as_slice()),
        )
        .expect("fixture SoraFS manifest is derived from the exact snapshots");
        let sorafs_manifest_bytes = manifest.encode().expect("fixture manifest encodes");
        let sorafs_manifest = SccpReplayArchiveSorafsManifestV1 {
            manifest_sha256: sha256(&[&sorafs_manifest_bytes]),
            manifest_root_cid: manifest
                .root_cid
                .as_slice()
                .try_into()
                .expect("canonical SoraFS CID has fixed width"),
            manifest_size_bytes: u64::try_from(sorafs_manifest_bytes.len())
                .expect("fixture manifest size fits u64"),
            snapshot_total_bytes,
        };
        let set_body =
            SccpReplayArchiveCheckpointSetBodyV1::new(finality, inventory_entries, sorafs_manifest)
                .expect("fixture complete inventory is valid");
        let set_message = sccp_replay_archive_checkpoint_set_signing_message_v1(&set_body)
            .expect("checkpoint-set signing message is defined");
        let signed_set = SccpReplayArchiveSignedCheckpointSetV1 {
            body: set_body,
            attestations: sign_attestations(set_message, key_pairs, replicas),
        };
        norito::encode_canonical(&SccpReplayReplicaCheckpointSetV1 {
            version: CHECKPOINT_SET_VERSION_V1,
            signed_set,
            sorafs_manifest_bytes,
            entries,
        })
        .expect("checkpoint set encodes")
    }

    fn sign_attestations(
        message: [u8; 32],
        key_pairs: &[KeyPair; 3],
        replicas: &[ToriiSccpReplayArchiveReplica; 3],
    ) -> [SccpReplayArchiveReplicaAttestationV1; 3] {
        core::array::from_fn(|index| {
            let signature = Signature::try_new(key_pairs[index].private_key(), &message)
                .expect("fixture checkpoint signs");
            SccpReplayArchiveReplicaAttestationV1 {
                replica_id: replicas[index].replica_id,
                signature: signature
                    .payload()
                    .try_into()
                    .expect("Ed25519 signatures are 64 bytes"),
            }
        })
    }

    fn loaded_head(service: &ToriiSccpReplayArchiveServiceV1) -> PersistedReplayHeadStateV1 {
        service
            .store
            .load_head(&service.config)
            .expect("persisted head passes integrity validation")
            .expect("persisted head exists")
    }

    fn validated_candidate(
        fixture: &Fixture,
        service: &ToriiSccpReplayArchiveServiceV1,
        bytes: &[u8],
    ) -> CandidateReplayStateV1 {
        let previous = loaded_head(service);
        validate_candidate(
            &fixture.config,
            bytes.to_vec(),
            Some(&previous),
            &EmptyForestLocalAuthority,
            CandidateLocalValidationV1::CurrentProjection,
        )
        .expect("fixture successor is a valid candidate")
    }

    fn generation_artifact_names(generation: PersistedReplayGenerationV1) -> BTreeSet<String> {
        replay_store_retained_names(&PersistedReplayHeadV1 {
            version: HEAD_MANIFEST_VERSION_V1,
            current: generation,
            recovery: None,
        })
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
        let root_response = service
            .root_response(&fixture.accumulator_id)
            .expect("current signed root response is served");
        assert_eq!(root_response.version, 1);
        assert_eq!(root_response.signed_set.body.entry_count, 1);
        assert_eq!(
            root_response.checkpoint_set_sha256,
            service
                .checkpoint_set_sha256()
                .expect("current checkpoint-set digest is served")
        );
        let witness_response = service
            .witness_response(&fixture.accumulator_id, [0; 32])
            .expect("atomic non-membership response is served");
        assert_eq!(witness_response.version, 1);
        assert_eq!(witness_response.replay_key, [0; 32]);
        assert_eq!(witness_response.witness, witness);
        assert_eq!(witness_response.root, root_response);
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
    fn replica_fetch_uses_owner_only_unlinked_descriptors() {
        use std::os::unix::fs::MetadataExt as _;

        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("secure replay store opens");
        let mut file = store
            .create_anonymous_fetch_file()
            .expect("anonymous replica descriptor is created");
        file.write_all(b"bounded replica frame")
            .expect("anonymous replica descriptor is writable");
        validate_anonymous_fetch_file(&file, b"bounded replica frame".len())
            .expect("anonymous replica descriptor retains its exact invariant");
        let metadata = file.metadata().expect("anonymous descriptor has metadata");
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 0);
        assert_eq!(
            fs::read_dir(&fixture.config.state_dir)
                .expect("state directory is readable")
                .count(),
            1,
            "only the process lock has a name while replica bytes are fetched"
        );
    }

    #[test]
    fn refresh_rebuilds_snapshots_but_does_not_rescan_kura_history() {
        let fixture = Fixture::new();
        let authority = Arc::new(CountingLocalAuthority::default());
        let service = ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
            fixture.config.clone(),
            fixture.source.clone(),
            authority.clone(),
        )
        .expect("initial checkpoint performs one complete Kura rebuild");
        assert_eq!(authority.full_rebuilds.load(Ordering::Relaxed), 1);
        assert_eq!(
            authority.current_projection_checks.load(Ordering::Relaxed),
            0
        );

        let (successor, _) = fixture.bytes_at(2, [0x42; 32]);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service
            .refresh()
            .expect("successor refresh verifies current Core and rebuilds its snapshots");
        assert_eq!(
            authority.full_rebuilds.load(Ordering::Relaxed),
            1,
            "refresh must not replay the complete Kura history"
        );
        assert_eq!(
            authority.current_projection_checks.load(Ordering::Relaxed),
            1
        );
    }

    #[test]
    fn signed_empty_checkpoint_set_is_valid_only_for_an_empty_local_inventory() {
        let fixture = Fixture::new();
        let finality = SccpReplayArchiveHeadFinalityV1 {
            network_identity_sha256: [0x91; 32],
            finalized_height: 1,
            finalized_block_hash: [0x41; 32],
        };
        let bytes = checkpoint_set_bytes_for_snapshots(
            &[],
            finality,
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture.source.set_all(&fixture.config.replicas, &bytes);
        let service = fixture
            .bootstrap()
            .expect("signed empty inventory bootstraps");
        assert_ne!(
            service
                .checkpoint_set_sha256()
                .expect("empty checkpoint set has an authenticated digest"),
            [0; 32]
        );
        assert_eq!(
            service.root_response(&fixture.accumulator_id),
            Err(ToriiSccpReplayEndpointErrorV1::NotFound)
        );
        assert_eq!(
            service.witness_response(&fixture.accumulator_id, [0; 32]),
            Err(ToriiSccpReplayEndpointErrorV1::NotFound)
        );
    }

    #[test]
    fn serving_stops_when_the_locally_authenticated_head_is_no_longer_current() {
        let fixture = Fixture::new();
        let authority = Arc::new(RevocableLocalAuthority::new());
        let service = ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
            fixture.config.clone(),
            fixture.source.clone(),
            authority.clone(),
        )
        .expect("current head bootstraps");
        service
            .forest(&fixture.accumulator_id)
            .expect("current forest is served");

        authority.current.store(false, Ordering::Release);
        assert_eq!(
            service.forest(&fixture.accumulator_id),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service.witness(&fixture.accumulator_id, [0; 32]),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service.checkpoint(&fixture.accumulator_id),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable)
        );
        assert_eq!(
            service.checkpoint_set_sha256(),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
        );
        assert_eq!(
            service.root_response(&fixture.accumulator_id),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
        );
        assert_eq!(
            service.witness_response(&fixture.accumulator_id, [0; 32]),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
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
    fn bootstrap_rejects_a_canonically_encoded_forged_attestation() {
        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        set.entries[0].checkpoint.attestations[1].signature[0] ^= 1;
        let forged = norito::encode_canonical(&set).expect("forged set remains canonical");
        fixture.source.set_all(&fixture.config.replicas, &forged);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::ReplicaAuthentication)
        );
    }

    #[test]
    fn bootstrap_rejects_unsigned_inventory_and_sorafs_manifest_substitution() {
        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        set.signed_set.body.sorafs_manifest.snapshot_total_bytes += 1;
        let forged = norito::encode_canonical(&set).expect("forged set remains canonical");
        fixture.source.set_all(&fixture.config.replicas, &forged);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::ReplicaAuthentication),
            "the complete inventory and SoraFS binding are signed"
        );

        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        set.entries.clear();
        let omitted = norito::encode_canonical(&set).expect("omitted set remains canonical");
        fixture.source.set_all(&fixture.config.replicas, &omitted);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Malformed),
            "an entry cannot be omitted from the signed complete inventory"
        );

        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        set.sorafs_manifest_bytes[0] ^= 1;
        let substituted = norito::encode_canonical(&set).expect("substitution remains canonical");
        fixture
            .source
            .set_all(&fixture.config.replicas, &substituted);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Malformed),
            "the signed SoraFS manifest hash binds the exact bytes"
        );

        let fixture = Fixture::new();
        let mut set: SccpReplayReplicaCheckpointSetV1 = norito::decode_canonical_with_limits(
            &fixture.first_bytes,
            norito::canonical_decode_limits(fixture.first_bytes.len()),
        )
        .expect("fixture checkpoint set decodes");
        let mut manifest = decode_manifest_v1_canonical(&set.sorafs_manifest_bytes)
            .expect("fixture SoraFS manifest decodes");
        manifest.root_cid[4] ^= 1;
        set.sorafs_manifest_bytes = manifest.encode().expect("mutated manifest encodes");
        set.signed_set.body.sorafs_manifest = SccpReplayArchiveSorafsManifestV1 {
            manifest_sha256: sha256(&[&set.sorafs_manifest_bytes]),
            manifest_root_cid: manifest
                .root_cid
                .as_slice()
                .try_into()
                .expect("canonical CID width remains fixed"),
            manifest_size_bytes: u64::try_from(set.sorafs_manifest_bytes.len())
                .expect("fixture manifest length fits u64"),
            snapshot_total_bytes: set.signed_set.body.sorafs_manifest.snapshot_total_bytes,
        };
        let signing_message =
            sccp_replay_archive_checkpoint_set_signing_message_v1(&set.signed_set.body)
                .expect("mutated set body hashes");
        set.signed_set.attestations = sign_attestations(
            signing_message,
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        let substituted =
            norito::encode_canonical(&set).expect("resigned manifest remains canonical");
        fixture
            .source
            .set_all(&fixture.config.replicas, &substituted);
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Malformed),
            "replicas cannot sign an arbitrary SoraFS root unrelated to the snapshots"
        );
    }

    #[test]
    fn first_bootstrap_accepts_a_current_midstream_checkpoint() {
        let fixture = Fixture::new();
        let (midstream, _) = fixture.bytes_at(17, [0x71; 32]);
        fixture.source.set_all(&fixture.config.replicas, &midstream);
        fixture
            .bootstrap()
            .expect("current Core/Kura authority permits a pruned midstream bootstrap");
    }

    #[test]
    fn refresh_is_idempotent_accepts_skipped_heads_and_rejects_forks() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (successor, successor_hash) = fixture.bytes_at(2, [0x42; 32]);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service.refresh().expect("strict successor refreshes");
        service
            .refresh()
            .expect("the exact cached head is an idempotent refresh");

        let (same_height_fork, _) = fixture.bytes_at(2, [0x52; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &same_height_fork);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::Continuity),
            "an equal-height different block is a fork"
        );
        assert_eq!(
            service.checkpoint(&fixture.accumulator_id),
            Err(SccpReplayArchiveProviderErrorV1::Unavailable),
            "a failed replica refresh makes the retained head unavailable"
        );
        fixture.source.set_all(&fixture.config.replicas, &successor);
        service
            .refresh()
            .expect("an exact current replica head restores availability");
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("restored replica availability exposes the retained checkpoint")
                .body
                .snapshot_sha256,
            successor_hash,
            "a rejected fork cannot replace visible replay state"
        );

        let (skipped_intermediate, _) = fixture.bytes_at(4, [0x44; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &skipped_intermediate);
        service
            .refresh()
            .expect("a locally rebuilt head may skip remote archive snapshots");

        let mut substituted_domain = fixture.domain;
        substituted_domain.route_configuration_hash[0] ^= 1;
        let (substituted_domain_head, _) =
            fixture.bytes_at_with_domain(5, [0x45; 32], substituted_domain);
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
    fn retention_keeps_exactly_the_current_and_one_prior_generation() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let first = loaded_head(&service).head.current;

        let (second_bytes, _) = fixture.bytes_at(2, [0x42; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &second_bytes);
        service.refresh().expect("second generation publishes");
        let second = loaded_head(&service);
        assert_eq!(second.head.recovery.as_ref(), Some(&first));

        let (third_bytes, _) = fixture.bytes_at(3, [0x43; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &third_bytes);
        service.refresh().expect("third generation publishes");
        let third = loaded_head(&service);
        assert_eq!(third.head.recovery.as_ref(), Some(&second.head.current));
        assert_ne!(third.head.current, second.head.current);

        let retained = replay_store_retained_names(&third.head);
        let actual = replay_store_directory_names(
            &service.store.directory,
            replay_store_scan_limit(fixture.config.max_accumulators)
                .expect("fixture scan geometry is bounded"),
        )
        .expect("retained directory is enumerable")
        .into_iter()
        .collect::<BTreeSet<_>>();
        let expected = retained
            .iter()
            .cloned()
            .chain([
                HEAD_MANIFEST_FILENAME_V1.to_owned(),
                PROCESS_LOCK_FILENAME_V1.to_owned(),
            ])
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected);
        assert_eq!(
            retained.len(),
            6,
            "one accumulator uses three files per head"
        );
        for obsolete in generation_artifact_names(first).difference(&retained) {
            assert!(
                !fixture.config.state_dir.join(obsolete).exists(),
                "the generation older than the recovery head is pruned"
            );
        }
    }

    #[test]
    fn idempotent_refresh_preserves_the_existing_recovery_generation() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (second_bytes, _) = fixture.bytes_at(2, [0x42; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &second_bytes);
        service.refresh().expect("second generation publishes");
        let before = loaded_head(&service).head;
        service.refresh().expect("exact head refresh is idempotent");
        let after = loaded_head(&service).head;
        assert_eq!(after, before);
    }

    #[test]
    fn scan_geometry_covers_the_hard_accumulator_boundary_without_allocation() {
        use iroha_config::parameters::defaults::torii::sccp_replay_archive as limits;

        let hard = usize::try_from(limits::MAX_ACCUMULATORS_HARD)
            .expect("the configured hard limit fits this platform");
        let expected = hard
            .checked_mul(6)
            .and_then(|value| value.checked_add(7))
            .expect("the hard-limit retention geometry fits usize");
        assert_eq!(replay_store_scan_limit(hard), Some(expected));
        assert!(expected > hard * 4 + 2, "pre-GC third-generation files fit");
        assert_eq!(replay_store_scan_limit(usize::MAX), None);
    }

    #[test]
    fn headless_restart_removes_only_strict_archive_orphans() {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private empty store opens");
        let orphan_bytes = b"orphaned before the first head publication";
        let orphan_name = snapshot_filename(sha256(&[orphan_bytes]));
        secure_write_immutable_relative(
            &store.directory,
            &orphan_name,
            orphan_bytes,
            fixture.config.max_snapshot_bytes,
        )
        .expect("headless candidate artifact is written");
        let (temporary, temporary_name) = create_secure_temporary(&store.directory, &orphan_name)
            .expect("interrupted temporary is created");
        drop(temporary);
        let quarantined_bytes = b"orphaned during garbage collection";
        let quarantined_base = snapshot_filename(sha256(&[quarantined_bytes]));
        secure_write_immutable_relative(
            &store.directory,
            &quarantined_base,
            quarantined_bytes,
            fixture.config.max_snapshot_bytes,
        )
        .expect("second orphan is durably written");
        let quarantined_name = format!(".{quarantined_base}.{}.gc", "ab".repeat(16));
        rustix::fs::renameat(
            &store.directory,
            quarantined_base.as_str(),
            &store.directory,
            quarantined_name.as_str(),
        )
        .expect("test simulates a crash after quarantine rename");

        assert!(
            store
                .load_head(&fixture.config)
                .expect("headless recovery prunes strict service-owned names")
                .is_none()
        );
        assert!(!fixture.config.state_dir.join(orphan_name).exists());
        assert!(!fixture.config.state_dir.join(temporary_name).exists());
        assert!(!fixture.config.state_dir.join(quarantined_name).exists());
        assert_eq!(
            replay_store_directory_names(
                &store.directory,
                replay_store_scan_limit(fixture.config.max_accumulators)
                    .expect("fixture scan geometry is bounded"),
            )
            .expect("recovered store is enumerable"),
            vec![PROCESS_LOCK_FILENAME_V1.to_owned()]
        );
    }

    #[test]
    fn restart_prunes_orphans_from_both_sides_of_manifest_publication() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let initial = loaded_head(&service).head;

        let orphan_bytes = b"unreferenced candidate snapshot";
        let orphan_name = snapshot_filename(sha256(&[orphan_bytes]));
        secure_write_immutable_relative(
            &service.store.directory,
            &orphan_name,
            orphan_bytes,
            fixture.config.max_snapshot_bytes,
        )
        .expect("pre-manifest artifact is durably written");
        let reloaded = loaded_head(&service);
        assert_eq!(reloaded.head, initial);
        assert!(
            !fixture.config.state_dir.join(&orphan_name).exists(),
            "restart under the old head removes a pre-publication orphan"
        );

        let (second_bytes, _) = fixture.bytes_at(2, [0x42; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &second_bytes);
        service.refresh().expect("second generation publishes");
        let first_generation = loaded_head(&service)
            .head
            .recovery
            .expect("first generation is retained for recovery");
        let first_names = generation_artifact_names(first_generation);

        let (third_bytes, _) = fixture.bytes_at(3, [0x43; 32]);
        let candidate = validated_candidate(&fixture, &service, &third_bytes);
        service
            .store
            .publish_candidate_head(&fixture.config, &candidate)
            .expect("new head is durable before garbage collection");
        assert!(
            first_names
                .iter()
                .any(|name| fixture.config.state_dir.join(name).exists()),
            "a crash immediately after manifest publication may leave older files"
        );

        let recovered = loaded_head(&service);
        assert_eq!(recovered.head, candidate.manifest);
        let retained = replay_store_retained_names(&recovered.head);
        for obsolete in first_names.difference(&retained) {
            assert!(
                !fixture.config.state_dir.join(obsolete).exists(),
                "restart completes post-manifest pruning"
            );
        }
    }

    #[test]
    fn shared_content_addressed_artifacts_are_retained_once() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let mut head = loaded_head(&service).head;
        let mut recovery = head.current.clone();
        recovery.checkpoint_set_sha256[0] ^= 1;
        head.recovery = Some(recovery);
        let head_bytes = norito::encode_canonical(&head).expect("synthetic retention head encodes");
        secure_write_manifest_last_relative(
            &service.store.directory,
            HEAD_MANIFEST_FILENAME_V1,
            &head_bytes,
            SecureReplayStoreV1::manifest_limit(&fixture.config),
        )
        .expect("synthetic retention head publishes");
        service
            .store
            .prune_to_head(&fixture.config, &head, &head_bytes)
            .expect("shared names are not mistaken for obsolete files");
        assert_eq!(
            replay_store_retained_names(&head).len(),
            3,
            "the set union retains one copy of each shared artifact"
        );
    }

    #[test]
    fn swapped_obsolete_name_fails_closed_and_gc_is_retryable() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let (second_bytes, second_snapshot) = fixture.bytes_at(2, [0x42; 32]);
        fixture
            .source
            .set_all(&fixture.config.replicas, &second_bytes);
        service.refresh().expect("second generation publishes");
        let before = loaded_head(&service);
        let obsolete = generation_artifact_names(
            before
                .head
                .recovery
                .clone()
                .expect("first generation is retained"),
        );

        let (third_bytes, _) = fixture.bytes_at(3, [0x43; 32]);
        let candidate = validated_candidate(&fixture, &service, &third_bytes);
        let head_bytes = service
            .store
            .publish_candidate_head(&fixture.config, &candidate)
            .expect("new head is durable before garbage collection");
        let retained = replay_store_retained_names(&candidate.manifest);
        let swapped_name = obsolete
            .difference(&retained)
            .next()
            .expect("one first-generation artifact becomes obsolete")
            .clone();
        let swapped_path = fixture.config.state_dir.join(&swapped_name);
        fs::remove_file(&swapped_path).expect("obsolete artifact is removed for race fixture");
        let outside = fixture
            .config
            .state_dir
            .parent()
            .expect("state directory has a parent")
            .join("gc-symlink-target");
        fs::write(&outside, b"must not be removed or truncated").expect("target is created");
        symlink(&outside, &swapped_path).expect("obsolete name is substituted");

        assert_eq!(
            service
                .store
                .prune_to_head(&fixture.config, &candidate.manifest, &head_bytes,),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "garbage collection never follows or unlinks a substituted symlink"
        );
        assert_eq!(
            fs::read(&outside).expect("outside target remains readable"),
            b"must not be removed or truncated"
        );
        assert_eq!(
            service
                .checkpoint(&fixture.accumulator_id)
                .expect("failed GC leaves the old in-memory head visible")
                .body
                .snapshot_sha256,
            second_snapshot
        );
        assert_eq!(
            secure_read_relative(
                &service.store.directory,
                HEAD_MANIFEST_FILENAME_V1,
                SecureReplayStoreV1::manifest_limit(&fixture.config),
            )
            .expect("new disk head remains readable")
            .as_deref(),
            Some(head_bytes.as_slice()),
            "a GC failure cannot roll back or corrupt the durable head"
        );

        fs::remove_file(&swapped_path).expect("hostile symlink is removed by the test owner");
        let recovered = loaded_head(&service);
        assert_eq!(recovered.head, candidate.manifest);
        assert!(
            outside.is_file(),
            "retry still never follows the outside target"
        );
    }

    #[test]
    fn retained_artifacts_require_the_same_exact_mode_as_garbage_collection() {
        use std::os::unix::fs::PermissionsExt as _;

        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let head = loaded_head(&service).head;
        let snapshot = head.current.entries[0].snapshot_sha256;
        fs::set_permissions(
            fixture.config.state_dir.join(snapshot_filename(snapshot)),
            fs::Permissions::from_mode(0o400),
        )
        .expect("test narrows the retained file mode");
        assert_eq!(
            service.store.load_head(&fixture.config).map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "load and GC both require exact owner-read/write mode"
        );
    }

    #[test]
    fn process_lock_rejects_concurrency_but_restart_accepts_the_same_head() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        assert_eq!(
            fixture.bootstrap().map(|_| ()),
            Err(ToriiSccpReplayStartupErrorV1::Persistence),
            "a second writer cannot race the retained manifest"
        );
        drop(service);
        let restarted = fixture
            .bootstrap()
            .expect("restart reauthenticates and accepts the exact persisted head");
        assert_eq!(
            restarted
                .checkpoint_set_sha256()
                .expect("restarted head is readable"),
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(&fixture.first_bytes)
        );
    }

    #[test]
    fn refresh_allows_only_an_authenticated_empty_accumulator_to_be_removed() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("initial checkpoint bootstraps");
        let finality = SccpReplayArchiveHeadFinalityV1 {
            network_identity_sha256: [0x91; 32],
            finalized_height: 2,
            finalized_block_hash: [0x72; 32],
        };
        let empty = checkpoint_set_bytes_for_snapshots(
            &[],
            finality,
            &fixture.key_pairs,
            &fixture.config.replicas,
        );
        fixture.source.set_all(&fixture.config.replicas, &empty);
        service
            .refresh()
            .expect("a deleted quiescent route drops its empty accumulator");
        assert_eq!(
            service.root_response(&fixture.accumulator_id),
            Err(ToriiSccpReplayEndpointErrorV1::NotFound)
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
    #[test]
    fn public_path_resolves_the_authenticated_complete_domain() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        let mut path = SccpReplayAccumulatorPathV1 {
            route_key: fixture.accumulator_id.route_key.clone(),
            boundary: fixture.accumulator_id.boundary,
        };
        assert_eq!(
            service.accumulator_id_for_path(&path),
            Ok(fixture.accumulator_id.clone())
        );
        path.route_key.route_id = "absent_route".to_owned();
        assert_eq!(
            service.accumulator_id_for_path(&path),
            Err(ToriiSccpReplayEndpointErrorV1::NotFound)
        );
    }

    #[test]
    fn refresh_if_stale_retains_an_authenticated_current_generation() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        fixture
            .source
            .set_all(&fixture.config.replicas, b"untrusted replacement");
        assert_eq!(service.refresh_if_stale(), Ok(false));
        assert_eq!(
            service.checkpoint_set_sha256(),
            Ok(sccp_replay_archive_checkpoint_set_frame_sha256_v1(
                &fixture.first_bytes
            ))
        );
    }

    #[test]
    fn published_snapshot_arc_releases_the_swap_lock_before_read_work() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("valid exact-three bootstrap");
        let published = service
            .current_published()
            .expect("authenticated generation is readable");
        let digest = published.checkpoint_set_sha256;

        let publication_guard = service
            .published
            .try_write()
            .expect("an owned read generation does not retain the publication lock");

        assert_eq!(published.checkpoint_set_sha256, digest);
        assert_eq!(publication_guard.checkpoint_set_sha256, digest);
    }

    #[test]
    fn restart_prefers_authenticated_persisted_state_over_remote_divergence() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("first head is admitted");
        let digest = service.checkpoint_set_sha256().expect("head is readable");
        drop(service);
        fixture
            .source
            .set_all(&fixture.config.replicas, b"untrusted replacement");
        let restarted = fixture
            .bootstrap()
            .expect("persisted current head is independently rebuilt");
        assert_eq!(restarted.checkpoint_set_sha256(), Ok(digest));
    }

    #[test]
    fn replica_fetch_contains_every_child_panic() {
        struct PanickingSource;
        impl SccpReplayCheckpointSourceV1 for PanickingSource {
            fn fetch_to(
                &self,
                _replica: &ToriiSccpReplayArchiveReplica,
                _max_response_bytes: usize,
                _timeout: Duration,
                _destination: &mut dyn std::io::Write,
            ) -> Result<usize, SccpReplayCheckpointSourceErrorV1> {
                panic!("untrusted transport panic")
            }
        }
        let fixture = Fixture::new();
        let store =
            SecureReplayStoreV1::open(&fixture.config.state_dir).expect("private store opens");
        assert_eq!(
            fetch_exact_three(&fixture.config, &PanickingSource, &store),
            Err(ToriiSccpReplayStartupErrorV1::Transport)
        );
        assert!(!iroha_core::panic_hook::is_suppressed());
    }

    #[test]
    fn failed_remote_refresh_preserves_a_current_authenticated_head() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("first head is admitted");
        let digest = service.checkpoint_set_sha256().expect("head is readable");
        fixture
            .source
            .set_all(&fixture.config.replicas, b"untrusted replacement");
        assert!(service.refresh().is_err());
        assert_eq!(service.checkpoint_set_sha256(), Ok(digest));
    }

    #[test]
    fn retry_reauthenticates_the_unchanged_durable_head_after_publication_failure() {
        let fixture = Fixture::new();
        let service = fixture.bootstrap().expect("first head is admitted");
        let digest = service.checkpoint_set_sha256().expect("head is readable");
        // A failure before manifest replacement disables reads without changing either digest.
        service.published_available.store(false, Ordering::Release);
        fixture
            .source
            .set_all(&fixture.config.replicas, b"untrusted replacement");
        assert!(service.refresh().is_err());
        assert_eq!(service.checkpoint_set_sha256(), Ok(digest));
    }
    #[test]
    fn new_remote_candidates_require_exact_tip_admission_while_retained_frames_do_not() {
        struct HistoricalReadAuthority;
        impl SccpReplayLocalAuthorityV1 for HistoricalReadAuthority {
            fn rebuild_and_verify(
                &self,
                finality: SccpReplayArchiveFinalityV1,
                expected: &BTreeMap<
                    SccpReplayAccumulatorIdV1,
                    (SccpReplayDomainV1, SccpReplayForestV1),
                >,
            ) -> Result<SccpReplayArchiveV1, SccpReplayLocalAuthorityErrorV1> {
                EmptyForestLocalAuthority.rebuild_and_verify(finality, expected)
            }
            fn verify_candidate(
                &self,
                _finality: SccpReplayArchiveFinalityV1,
                _expected: &BTreeMap<
                    SccpReplayAccumulatorIdV1,
                    (SccpReplayDomainV1, SccpReplayForestV1),
                >,
            ) -> Result<(), SccpReplayLocalAuthorityErrorV1> {
                Err(SccpReplayLocalAuthorityErrorV1::Finality)
            }
        }
        let fixture = Fixture::new();
        let service = ToriiSccpReplayArchiveServiceV1::bootstrap_with_components(
            fixture.config.clone(),
            fixture.source.clone(),
            Arc::new(HistoricalReadAuthority),
        )
        .expect("initial Kura rebuild admits the head");
        service
            .refresh()
            .expect("byte-identical securely retained frame remains admissible");
        let before = service
            .checkpoint_set_sha256()
            .expect("retained head is readable");
        let (successor, _) = fixture.bytes_at(2, [0x42; 32]);
        fixture.source.set_all(&fixture.config.replicas, &successor);
        assert_eq!(
            service.refresh(),
            Err(ToriiSccpReplayStartupErrorV1::LocalAuthority)
        );
        assert_eq!(service.checkpoint_set_sha256(), Ok(before));
    }
}
