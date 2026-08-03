//! Bounded, authenticated transfer of certified merge-ledger sidecars.
//!
//! Global blocks carry only a compact [`CertifiedMergeLedgerReference`].  A
//! validator that does not yet have the referenced full entry asks one of the
//! exact merge-QC signers for it.  Transfer sessions are deliberately
//! byte-ephemeral: semantic stream floors, cursors, and pending identities are
//! crash-safe, while incomplete payload bytes remain in memory. Only a
//! completely reassembled, canonical, reference-matching entry may be handed
//! to Kura's atomic pending-sidecar store.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, CertifiedMergeLedgerReference, consensus_v2::MAX_VALIDATORS_PER_HEIGHT},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLedgerEntry},
    peer::PeerId,
};
#[cfg(test)]
use iroha_p2p::network::{NetworkReplyFlushAckTestFixture, NetworkReplyRouteTestFixture};
use iroha_p2p::{
    Post, Priority,
    network::{
        NetworkReplyFlushIdentity, NetworkReplyRoute, NetworkReplyRouteSourceUpdate,
        NetworkReplyRoutes, NetworkReplySourceKey,
        message::{ClassifyTopic as _, Topic},
    },
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

#[cfg(test)]
use crate::sumeragi::v2_core::{
    production_reliable_flush_trace_refines_outbound_ownership_kernel,
    production_reliable_flush_two_phase_link_kernel,
};
use crate::{
    merge::MergeLedgerCandidate,
    sumeragi::{
        v2_core::{
            CanonicalIdentityProjection, IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_PEER,
            IDENTITY_DOMAIN_PROCESS_LOCAL, IDENTITY_KIND_MERGE_ENTRY,
            IDENTITY_KIND_NETWORK_RESPONSE, IDENTITY_KIND_PEER, IDENTITY_KIND_REFERENCE_DIGEST,
            IDENTITY_KIND_REPLY_DELIVERY_ROUTE, IDENTITY_KIND_REPLY_PAYLOAD,
            IDENTITY_KIND_REPLY_SOURCE_KEY, IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
            IDENTITY_KIND_SIDECAR_CHUNK, IDENTITY_KIND_SIDECAR_PAYLOAD,
            IDENTITY_KIND_SIDECAR_REQUEST, IDENTITY_KIND_SIDECAR_RESPONSE,
            IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE, IDENTITY_KIND_SIDECAR_SIBLING_STATE,
            IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE, IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE,
            ProductionReliableFlushApplicationProjection, ProductionReliableFlushTraceProjection,
            check_production_reliable_flush_application_transition,
            check_production_reliable_flush_link_transition,
            check_production_reliable_flush_worker_transition,
        },
        v2_lane_work::DurableMergeSidecarRolloverAuthority,
    },
};

/// Current certified merge-sidecar transfer protocol version.
pub const CERTIFIED_MERGE_SIDECAR_VERSION_V1: u8 = 1;
/// Maximum payload carried by one sidecar chunk.
pub const MAX_CERTIFIED_MERGE_CHUNK_BYTES: usize = 64 * 1024;
/// Maximum chunks required by a protocol-sized full entry.
pub const MAX_CERTIFIED_MERGE_CHUNKS: usize =
    MAX_MERGE_LEDGER_ENTRY_BYTES.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);

/// Durable requester-issued incarnation of one semantic request stream.
///
/// Epochs are globally monotonic at one requester and are never reused, even
/// across crashes or height rollover. Sequence and cumulative-close values are
/// meaningful only inside the exact `(requester, responder, stream_epoch)`
/// tuple.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[repr(transparent)]
pub struct CertifiedMergeSidecarStreamEpochV1(pub NonZeroU64);

impl CertifiedMergeSidecarStreamEpochV1 {
    /// Return the non-zero wire value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Non-zero occurrence coordinate within one certified merge-sidecar stream.
///
/// Sequence values are meaningful only inside the exact `(requester,
/// responder, service_generation, stream_epoch)` tuple. Cumulative close
/// floors and stream high-water counters remain plain `u64` values because
/// zero represents an empty prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[repr(transparent)]
pub struct CertifiedMergeSidecarSemanticSequenceV1(pub NonZeroU64);

impl CertifiedMergeSidecarSemanticSequenceV1 {
    /// Return the non-zero wire value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Durable responder-owned generation of the bounded sidecar service state.
///
/// A responder advances this global fence only while atomically installing a
/// certified changed roster. Ordinary rehydration requires terminal
/// per-requester state; a durable exact-output handoff or lifecycle restart may
/// instead supersede active responder state after predecessor writers become
/// unreachable. Delayed messages from the prior roster generation can
/// therefore be rejected without retaining an unbounded collection of peer
/// tombstones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[repr(transparent)]
pub struct CertifiedMergeSidecarServiceGenerationV1(pub NonZeroU64);

impl CertifiedMergeSidecarServiceGenerationV1 {
    /// Initial generation used by a fresh responder and an uninformed client.
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    /// Return the non-zero wire value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// Typed identity of one canonical responder roster.
pub(crate) type MergeSidecarRosterDigest = HashOf<Vec<PeerId>>;

/// Hash a roster after canonical peer ordering and duplicate removal.
///
/// Callers must derive the accompanying capacity from the same unique roster,
/// never from currently connected peers.
pub(crate) fn canonical_merge_sidecar_roster_digest(roster: &[PeerId]) -> MergeSidecarRosterDigest {
    let ordered = roster.iter().cloned().collect::<BTreeSet<_>>();
    HashOf::new(&ordered.into_iter().collect::<Vec<_>>())
}

#[cfg(test)]
fn unbound_test_merge_sidecar_roster_digest() -> MergeSidecarRosterDigest {
    HashOf::new(&Vec::<PeerId>::new())
}

const REFERENCE_DIGEST_DOMAIN: &[u8] = b"iroha:merge:sidecar-reference:v1\0";
const REQUEST_ID_DOMAIN: &[u8] = b"iroha:merge:sidecar-request:v1\0";
const CLOSE_ID_DOMAIN: &[u8] = b"iroha:merge:sidecar-close:v1\0";
const SERVICE_GENERATION_HINT_ID_DOMAIN: &[u8] =
    b"iroha:merge:sidecar-service-generation-hint:v1\0";
const SIGNING_CONTEXT_DOMAIN: &[u8] = b"iroha:merge:signing-context:v2\0";

const RESERVED_DECIDED_INBOUND_SESSIONS: usize = 1;
const RESERVED_DECIDED_INBOUND_BYTES: usize = MAX_MERGE_LEDGER_ENTRY_BYTES;
const RESERVED_DECIDED_DEFERRED_BLOCKS: usize = 1;
/// Maximum live requester-side semantic-stream working set.
///
/// The table is validator-scoped rather than connection-scoped. Requester
/// streams use globally monotonic local epochs.
const MAX_CERTIFIED_MERGE_SEMANTIC_PEERS: usize = MAX_VALIDATORS_PER_HEIGHT;
/// Maximum responder-side semantic-stream working set.
///
/// Production reserves one complete committee in addition to the current
/// roster.  The extra corridor lets an exact predecessor committee finish
/// historical recovery without consuming any current-roster slot.  Adapter
/// admission separately caps identities outside the current frozen roster at
/// [`MAX_VALIDATORS_PER_HEIGHT`]; the transport keeps the aggregate hard bound
/// and durable lifecycle geometry.
const MAX_CERTIFIED_MERGE_SERVER_STREAMS: usize = 2 * MAX_VALIDATORS_PER_HEIGHT;

#[cfg(test)]
const DEFAULT_REPLY_SOURCE_CAPACITY: usize = 8;
const CHUNK_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"iroha:merge-sidecar:chunk-payload:v1";
const RELIABLE_FLUSH_SIBLING_STATE_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-sibling-state:v1\0";
const RELIABLE_FLUSH_SHARED_TRANSFER_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-shared-transfer:v1\0";
const RELIABLE_FLUSH_TARGET_GATE_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-target-gate:v1\0";
const RELIABLE_FLUSH_TARGET_OUTBOUND_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-target-outbound:v1\0";

const SIGNING_GUARD_VERSION: u8 = 2;
const SIGNING_GUARD_DIR: &str = "merge-signing-guard-v2";
const LEGACY_SIGNING_GUARD_DIRS: &[&str] = &["merge-signing-guard-v1"];
const SIGNING_GUARD_RECORD_EXT: &str = "norito";
const SIGNING_GUARD_TEMP_EXT: &str = "norito.tmp";
const SIGNING_GUARD_HIGH_WATER_FILE: &str = "committed-high-water.norito";
const SIGNING_GUARD_HIGH_WATER_TEMP: &str = "committed-high-water.norito.tmp";
const LIFECYCLE_JOURNAL_VERSION_V3: u8 = 3;
const LIFECYCLE_JOURNAL_DIR: &str = "sumeragi_v2_merge_sidecar_lifecycle_v3";
const LEGACY_LIFECYCLE_JOURNAL_DIRS: &[&str] = &[
    "sumeragi_v2_merge_sidecar_lifecycle_v1",
    "sumeragi_v2_merge_sidecar_lifecycle_v2",
];
const LIFECYCLE_JOURNAL_SLOT_FILES: [&str; 2] = ["state-0.norito", "state-1.norito"];
const LIFECYCLE_JOURNAL_TEMP: &str = "state.norito.tmp";
const LIFECYCLE_ROOT_HIGH_WATER_FILE: &str =
    "sumeragi_v2_merge_sidecar_lifecycle_v3_root_high_water.norito";
const LIFECYCLE_ROOT_HIGH_WATER_TEMP: &str =
    "sumeragi_v2_merge_sidecar_lifecycle_v3_root_high_water.norito.tmp";
const LIFECYCLE_ROOT_HIGH_WATER_MAX_BYTES: usize = 4 * 1024;
const LIFECYCLE_JOURNAL_BASE_BYTES: usize = 64 * 1024;
const LIFECYCLE_JOURNAL_GATE_BYTES: usize = 16 * 1024;
const LIFECYCLE_JOURNAL_STREAM_BYTES: usize = 2 * 1024;
#[cfg(test)]
const MAX_INBOUND_SESSIONS: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY.get();
#[cfg(test)]
const MAX_INBOUND_SESSIONS_PER_PEER: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER.get();
#[cfg(test)]
const MAX_OUTBOUND_SESSIONS_PER_SOURCE: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE
        .get();
#[cfg(test)]
const MAX_OUTBOUND_BYTES_PER_SOURCE: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE.get();
#[cfg(test)]
const MAX_SERVER_REQUEST_GATES_PER_SOURCE: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE
        .get();
#[cfg(test)]
const REQUEST_TIMEOUT: Duration =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIDECAR_REQUEST_TIMEOUT;
#[cfg(test)]
const MAX_SIGNING_GUARD_RECORDS: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_CAPACITY.get();
#[cfg(test)]
const MAX_SIGNING_GUARD_RECORD_BYTES: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_RECORD_BYTES.get();
#[cfg(test)]
const MAX_SIGNING_GUARD_TOTAL_BYTES: usize =
    iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_TOTAL_BYTES.get();

/// Fingerprinted runtime geometry for certified merge-sidecar transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MergeSidecarLimits {
    inbound_session_capacity: usize,
    inbound_sessions_per_peer: usize,
    inbound_assembly_bytes: usize,
    inbound_assembly_bytes_per_peer: usize,
    deferred_block_capacity: usize,
    future_block_distance: u64,
    request_timeout: Duration,
    outbound_sessions_per_source: usize,
    outbound_bytes_per_source: usize,
    server_request_gates_per_source: usize,
}

impl MergeSidecarLimits {
    /// Construct a geometry which retains disjoint decided and ordinary
    /// full-entry corridors and cannot weaken per-source ownership.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        inbound_session_capacity: NonZeroUsize,
        inbound_sessions_per_peer: NonZeroUsize,
        inbound_assembly_bytes: NonZeroUsize,
        inbound_assembly_bytes_per_peer: NonZeroUsize,
        deferred_block_capacity: NonZeroUsize,
        future_block_distance: NonZeroU64,
        request_timeout: Duration,
        outbound_sessions_per_source: NonZeroUsize,
        outbound_bytes_per_source: NonZeroUsize,
        server_request_gates_per_source: NonZeroUsize,
    ) -> Result<Self, MergeSidecarError> {
        let inbound_session_capacity = inbound_session_capacity.get();
        let inbound_sessions_per_peer = inbound_sessions_per_peer.get();
        let inbound_assembly_bytes = inbound_assembly_bytes.get();
        let inbound_assembly_bytes_per_peer = inbound_assembly_bytes_per_peer.get();
        let deferred_block_capacity = deferred_block_capacity.get();
        let outbound_sessions_per_source = outbound_sessions_per_source.get();
        let outbound_bytes_per_source = outbound_bytes_per_source.get();
        let server_request_gates_per_source = server_request_gates_per_source.get();
        let minimum_inbound_bytes =
            MAX_MERGE_LEDGER_ENTRY_BYTES
                .checked_mul(2)
                .ok_or(MergeSidecarError::Capacity(
                    "merge-sidecar inbound reserved-byte geometry",
                ))?;
        if inbound_session_capacity <= RESERVED_DECIDED_INBOUND_SESSIONS
            || inbound_sessions_per_peer <= RESERVED_DECIDED_INBOUND_SESSIONS
            || inbound_sessions_per_peer > inbound_session_capacity
            || deferred_block_capacity <= RESERVED_DECIDED_DEFERRED_BLOCKS
            || inbound_assembly_bytes < minimum_inbound_bytes
            || inbound_assembly_bytes_per_peer < minimum_inbound_bytes
            || inbound_assembly_bytes_per_peer > inbound_assembly_bytes
            || outbound_bytes_per_source < MAX_MERGE_LEDGER_ENTRY_BYTES
            || server_request_gates_per_source < outbound_sessions_per_source
            || request_timeout.is_zero()
        {
            return Err(MergeSidecarError::Capacity(
                "invalid merge-sidecar runtime geometry",
            ));
        }
        Ok(Self {
            inbound_session_capacity,
            inbound_sessions_per_peer,
            inbound_assembly_bytes,
            inbound_assembly_bytes_per_peer,
            deferred_block_capacity,
            future_block_distance: future_block_distance.get(),
            request_timeout,
            outbound_sessions_per_source,
            outbound_bytes_per_source,
            server_request_gates_per_source,
        })
    }

    #[cfg(test)]
    pub(crate) fn defaults() -> Self {
        use iroha_config::parameters::defaults::sumeragi as defaults;

        Self::new(
            defaults::V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY,
            defaults::V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER,
            defaults::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES,
            defaults::V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_PER_PEER,
            defaults::V2_MERGE_SIDECAR_DEFERRED_BLOCK_CAPACITY,
            defaults::V2_MERGE_SIDECAR_FUTURE_BLOCK_DISTANCE,
            defaults::V2_MERGE_SIDECAR_REQUEST_TIMEOUT,
            defaults::V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE,
            defaults::V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE,
            defaults::V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE,
        )
        .expect("default merge-sidecar limits are valid")
    }
}

/// Fingerprinted disk geometry for the durable merge-signing guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MergeSigningGuardLimits {
    max_records: usize,
    max_record_bytes: usize,
    max_total_bytes: usize,
}

impl MergeSigningGuardLimits {
    /// Construct a journal geometry capable of atomically retaining at least
    /// one protocol-sized candidate decision and its metadata.
    pub(crate) fn new(
        max_records: NonZeroUsize,
        max_record_bytes: NonZeroUsize,
        max_total_bytes: NonZeroUsize,
    ) -> Result<Self, MergeSidecarError> {
        let max_record_bytes = max_record_bytes.get();
        let max_total_bytes = max_total_bytes.get();
        let metadata_headroom =
            iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES;
        let minimum_record_bytes = MAX_MERGE_LEDGER_ENTRY_BYTES
            .checked_add(metadata_headroom)
            .ok_or(MergeSidecarError::Capacity(
                "merge-signing record byte geometry",
            ))?;
        let minimum_total_bytes =
            max_record_bytes
                .checked_add(metadata_headroom)
                .ok_or(MergeSidecarError::Capacity(
                    "merge-signing aggregate byte geometry",
                ))?;
        if max_record_bytes < minimum_record_bytes || max_total_bytes < minimum_total_bytes {
            return Err(MergeSidecarError::Capacity(
                "invalid merge-signing guard runtime geometry",
            ));
        }
        Ok(Self {
            max_records: max_records.get(),
            max_record_bytes,
            max_total_bytes,
        })
    }

    #[cfg(test)]
    pub(crate) fn defaults() -> Self {
        use iroha_config::parameters::defaults::sumeragi as defaults;

        Self::new(
            defaults::V2_MERGE_SIGNING_GUARD_RECORD_CAPACITY,
            defaults::V2_MERGE_SIGNING_GUARD_RECORD_BYTES,
            defaults::V2_MERGE_SIGNING_GUARD_TOTAL_BYTES,
        )
        .expect("default merge-signing limits are valid")
    }
}

fn retry_timeout(base: Duration, attempts: u32) -> Duration {
    let backoff_shift = attempts.saturating_sub(1).min(4);
    base.saturating_mul(1_u32 << backoff_shift)
}

/// Point-to-point request for one exact certified merge sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarRequestV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Responder-owned lifecycle generation expected by the requester.
    pub service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable incarnation of the requester-to-responder semantic stream.
    pub stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Monotonic semantic sequence in the requester-to-responder stream.
    pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    /// Cumulative authenticated close floor for the same semantic stream.
    pub closed_through: u64,
    /// Canonical identity of this exact immutable request occurrence.
    ///
    /// The cumulative `closed_through` floor is intentionally excluded so it
    /// may advance monotonically on the same occurrence without changing the
    /// request identity or rematerializing a response.
    pub request_id: Hash,
    /// Canonical hash of the requested full entry.
    pub entry_hash: HashOf<MergeLedgerEntry>,
    /// Exact canonical byte length committed by the compact reference.
    pub encoded_len: u64,
    /// Merge epoch committed by the compact reference.
    pub epoch_id: u64,
    /// Digest of the complete compact reference.
    pub reference_digest: Hash,
    /// Authenticated peer expected on the P2P envelope.
    pub requester: PeerId,
    /// Exact merge-QC signer selected to answer this request.
    pub responder: PeerId,
}

impl CertifiedMergeSidecarRequestV1 {
    /// Derive the canonical identity of this immutable semantic projection.
    #[must_use]
    pub fn canonical_request_id(&self) -> Hash {
        let version = [self.version];
        let service_generation = self.service_generation.get().to_le_bytes();
        let stream_epoch = self.stream_epoch.get().to_le_bytes();
        let semantic_sequence = self.semantic_sequence.get().to_le_bytes();
        let encoded_len = self.encoded_len.to_le_bytes();
        let epoch_id = self.epoch_id.to_le_bytes();
        let requester = self.requester.encode();
        let responder = self.responder.encode();
        Hash::new_from_chunks(&[
            REQUEST_ID_DOMAIN,
            &version,
            &service_generation,
            &stream_epoch,
            &semantic_sequence,
            self.entry_hash.as_ref().as_ref(),
            &encoded_len,
            &epoch_id,
            self.reference_digest.as_ref(),
            requester.as_slice(),
            responder.as_slice(),
        ])
    }

    fn bind_canonical_request_id(&mut self) {
        self.request_id = self.canonical_request_id();
    }

    fn same_occurrence_except_close_floor(&self, other: &Self) -> bool {
        self.version == other.version
            && self.service_generation == other.service_generation
            && self.stream_epoch == other.stream_epoch
            && self.semantic_sequence == other.semantic_sequence
            && self.request_id == other.request_id
            && self.entry_hash == other.entry_hash
            && self.encoded_len == other.encoded_len
            && self.epoch_id == other.epoch_id
            && self.reference_digest == other.reference_digest
            && self.requester == other.requester
            && self.responder == other.responder
    }
}

/// Cumulative authenticated release of completed semantic request occurrences.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarCloseV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Responder-owned lifecycle generation expected by the requester.
    pub service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable incarnation of the requester-to-responder semantic stream.
    pub stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Highest contiguous semantic sequence which the requester has terminated.
    pub closed_through: u64,
    /// Canonical identity of this cumulative close witness.
    pub close_id: Hash,
    /// Authenticated peer which issued the request stream.
    pub requester: PeerId,
    /// Exact responder whose retained occurrences may be released.
    pub responder: PeerId,
}

impl CertifiedMergeSidecarCloseV1 {
    /// Derive the canonical identity of this cumulative close witness.
    #[must_use]
    pub(crate) fn canonical_close_id(&self) -> Hash {
        let version = [self.version];
        let service_generation = self.service_generation.get().to_le_bytes();
        let stream_epoch = self.stream_epoch.get().to_le_bytes();
        let closed_through = self.closed_through.to_le_bytes();
        let requester = self.requester.encode();
        let responder = self.responder.encode();
        Hash::new_from_chunks(&[
            CLOSE_ID_DOMAIN,
            &version,
            &service_generation,
            &stream_epoch,
            &closed_through,
            requester.as_slice(),
            responder.as_slice(),
        ])
    }

    fn bind_canonical_close_id(&mut self) {
        self.close_id = self.canonical_close_id();
    }
}

/// Idempotent responder acknowledgement for one cumulative close witness.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarCloseAckV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Responder-owned lifecycle generation in which the close was applied.
    pub service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable incarnation of the acknowledged semantic stream.
    pub stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Cumulative close floor durably observed by the responder.
    pub closed_through: u64,
    /// Canonical identity copied from the acknowledged close witness.
    pub close_id: Hash,
    /// Authenticated peer which issued the request stream.
    pub requester: PeerId,
    /// Exact responder which applied the cumulative close floor.
    pub responder: PeerId,
}

impl CertifiedMergeSidecarCloseAckV1 {
    pub(crate) fn canonical_close_id(&self) -> Hash {
        CertifiedMergeSidecarCloseV1 {
            version: self.version,
            service_generation: self.service_generation,
            stream_epoch: self.stream_epoch,
            closed_through: self.closed_through,
            close_id: self.close_id,
            requester: self.requester.clone(),
            responder: self.responder.clone(),
        }
        .canonical_close_id()
    }
}

/// Authenticated responder fence returned for a stale service generation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarGenerationHintV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Generation carried by the request or close which triggered this hint.
    pub observed_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Responder's current durable service generation.
    pub current_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Canonical hash of the exact request or close observed by the responder.
    pub observed_message_hash: Hash,
    /// Canonical identity of this exact generation hint.
    pub hint_id: Hash,
    /// Authenticated peer which issued the stale control or request.
    pub requester: PeerId,
    /// Exact responder which owns `current_generation`.
    pub responder: PeerId,
}

impl CertifiedMergeSidecarGenerationHintV1 {
    /// Derive the canonical identity of this responder-generation witness.
    #[must_use]
    pub(crate) fn canonical_hint_id(&self) -> Hash {
        let version = [self.version];
        let observed_generation = self.observed_generation.get().to_le_bytes();
        let current_generation = self.current_generation.get().to_le_bytes();
        let requester = self.requester.encode();
        let responder = self.responder.encode();
        Hash::new_from_chunks(&[
            SERVICE_GENERATION_HINT_ID_DOMAIN,
            &version,
            &observed_generation,
            &current_generation,
            self.observed_message_hash.as_ref(),
            requester.as_slice(),
            responder.as_slice(),
        ])
    }

    fn bind_canonical_hint_id(&mut self) {
        self.hint_id = self.canonical_hint_id();
    }
}

/// One fixed-boundary chunk of a certified merge sidecar response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarChunkV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Responder-owned lifecycle generation copied from the request.
    pub service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable incarnation copied verbatim from the request.
    pub stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Monotonic semantic sequence copied verbatim from the request.
    pub semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    /// Canonical request identity copied verbatim from the request.
    pub request_id: Hash,
    /// Canonical hash of the requested full entry.
    pub entry_hash: HashOf<MergeLedgerEntry>,
    /// Exact canonical byte length of the full entry.
    pub encoded_len: u64,
    /// Merge epoch committed by the compact reference.
    pub epoch_id: u64,
    /// Digest of the complete compact reference.
    pub reference_digest: Hash,
    /// Authenticated requester identity copied from the request.
    pub requester: PeerId,
    /// Authenticated responder identity; must match the P2P envelope sender.
    pub responder: PeerId,
    /// Zero-based fixed-boundary chunk index.
    pub chunk_index: u32,
    /// Exact number of chunks needed for `encoded_len`.
    pub chunk_count: u32,
    /// Chunk payload. Non-final chunks are exactly 64 KiB.
    pub bytes: Vec<u8>,
}

/// Wire messages used by the certified merge-sidecar protocol.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum CertifiedMergeSidecarMessage {
    /// Request an exact full entry from one merge-QC signer.
    Request(CertifiedMergeSidecarRequestV1),
    /// Release a contiguous prefix of terminated request occurrences.
    Close(CertifiedMergeSidecarCloseV1),
    /// Acknowledge durable application of a cumulative close floor.
    CloseAck(CertifiedMergeSidecarCloseAckV1),
    /// Advertise the responder's durable generation after a stale request.
    GenerationHint(CertifiedMergeSidecarGenerationHintV1),
    /// Return one bounded chunk of the requested entry.
    Chunk(CertifiedMergeSidecarChunkV1),
}

/// Lossless process-local projection of one admitted response-chunk flush.
///
/// This projection deliberately retains native peer, route-source, topic, and
/// canonical hash types. It contains no capability constructor and no wire
/// codec. The materialized response bytes remain shared by the transport;
/// only their canonical identities and the exact chunk payload digest are
/// repeated per authenticated source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CertifiedMergeSidecarChunkFlushProjection {
    /// Semantic target named by the response post and opaque reply route.
    pub(crate) semantic_target: PeerId,
    /// Authenticated transport source which owns this independent attempt.
    pub(crate) authenticated_source: PeerId,
    /// Opaque process-local identity of the authenticated source owner.
    pub(crate) source_key_identity: Hash,
    /// Opaque process-local identity of the exact admitted delivery route.
    pub(crate) delivery_route_identity: Hash,
    /// Opaque process-local identity of the actor-minted writer completion.
    pub(crate) writer_occurrence_identity: Hash,
    /// Actor-global ordinal of the exact authenticated connection tenure.
    pub(crate) connection_tenure_ordinal: u128,
    /// Actor-global ordinal of the exact local delivery occurrence.
    pub(crate) delivery_ordinal: u128,
    /// Actor-budget-local ticket identifier.
    pub(crate) ticket_id: u64,
    /// One-based actor service rank at admission.
    pub(crate) ticket_rank: usize,
    /// Canonical reliable-progress topic bound into the actor ticket.
    pub(crate) ticket_topic: Topic,
    /// Adaptive reply-writer timeout generation bound into the actor ticket.
    pub(crate) reply_writer_timeout_attempt: u8,
    /// Digest of the canonical priority, semantic target, and encoded response.
    pub(crate) canonical_request_digest: Hash,
    /// Exact encrypted-stream queue charge assigned by the network actor.
    pub(crate) stream_wire_bytes: usize,
    /// Semantic sidecar request nonce.
    pub(crate) request_id: Hash,
    /// Responder-owned durable service generation.
    pub(crate) service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable incarnation of the semantic request stream.
    pub(crate) stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Monotonic requester-to-responder semantic request sequence.
    pub(crate) semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    /// Canonical hash of the requested full merge-ledger entry.
    pub(crate) entry_hash: HashOf<MergeLedgerEntry>,
    /// Exact canonical byte length of the full entry.
    pub(crate) encoded_len: u64,
    /// Merge epoch committed by the compact reference.
    pub(crate) epoch_id: u64,
    /// Digest of the complete compact reference.
    pub(crate) reference_digest: Hash,
    /// Semantic requester copied from the request.
    pub(crate) requester: PeerId,
    /// Exact response signer.
    pub(crate) responder: PeerId,
    /// Canonical identity of the complete network response envelope payload.
    pub(crate) canonical_response_hash: HashOf<crate::NetworkMessage>,
    /// Canonical identity of the sidecar response enum.
    pub(crate) sidecar_response_hash: HashOf<CertifiedMergeSidecarMessage>,
    /// Canonical identity of the exact fixed-boundary chunk.
    pub(crate) chunk_hash: HashOf<CertifiedMergeSidecarChunkV1>,
    /// Domain-separated digest of only the exact chunk payload bytes.
    pub(crate) payload_digest: Hash,
    /// Zero-based fixed-boundary chunk index.
    pub(crate) chunk_index: u32,
    /// Exact number of chunks in the materialized response.
    pub(crate) chunk_count: u32,
    /// Exact per-source output-message cursor before actor admission.
    pub(crate) message_cursor_before: usize,
    /// Exact per-source output-message cursor after actor admission.
    pub(crate) message_cursor_after: usize,
    /// Exact per-source sidecar chunk cursor before writer flush.
    pub(crate) chunk_cursor_before: usize,
    /// Exact per-source sidecar chunk cursor after writer flush.
    pub(crate) chunk_cursor_after: usize,
}

/// Process-local evidence for one response chunk awaiting its peer-writer flush.
///
/// The value is never serialized. It binds the immutable projection to the
/// opaque actor-owned ticket and route occurrence which admitted the exact
/// canonical post. Possessing this value is not evidence of delivery by
/// itself: exact worker output may pass it to
/// [`MergeSidecarTransport::acknowledge_outbound_chunk`] only after the same
/// acknowledgement reports a successful full write and flush.
#[derive(Clone, Debug)]
pub(crate) struct CertifiedMergeSidecarChunkAdmission {
    projection: CertifiedMergeSidecarChunkFlushProjection,
    source_key: NetworkReplySourceKey,
    flush_identity: NetworkReplyFlushIdentity,
    confirmed_worker_trace: Option<ProductionReliableFlushTraceProjection>,
}

impl CertifiedMergeSidecarChunkAdmission {
    /// Bind an exact sidecar output cursor to its actor-owned flush identity.
    pub(crate) fn from_admitted_reply(
        post: &Post<crate::NetworkMessage>,
        reply_route: &NetworkReplyRoute,
        message_cursor_before: usize,
        message_cursor_after: usize,
        flush_identity: &NetworkReplyFlushIdentity,
    ) -> Result<Self, MergeSidecarError> {
        let crate::NetworkMessage::CertifiedMergeSidecar(message) = &post.data else {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "canonical response is not a certified merge-sidecar message",
            ));
        };
        let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "canonical response is not a certified merge-sidecar chunk",
            ));
        };
        let Some(expected_message_cursor_after) = message_cursor_before.checked_add(1) else {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "output message cursor overflowed",
            ));
        };
        if message_cursor_before != 0 || message_cursor_after != expected_message_cursor_after {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "single-response output message cursor did not advance exactly once",
            ));
        }
        let chunk_cursor_before = usize::try_from(chunk.chunk_index).map_err(|_| {
            MergeSidecarError::FlushIdentityMismatch("chunk index is not representable")
        })?;
        let chunk_cursor_after =
            chunk_cursor_before
                .checked_add(1)
                .ok_or(MergeSidecarError::FlushIdentityMismatch(
                    "chunk cursor overflowed",
                ))?;
        let chunk_count = usize::try_from(chunk.chunk_count).map_err(|_| {
            MergeSidecarError::FlushIdentityMismatch("chunk count is not representable")
        })?;
        if chunk_count == 0 || chunk_cursor_after > chunk_count {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "chunk cursor is outside the certified response",
            ));
        }
        if post.peer_id != chunk.requester
            || reply_route.semantic_target() != &chunk.requester
            || !flush_identity.is_bound_to_delivery(reply_route)
            || !flush_identity.is_bound_to_canonical_reply(post)
        {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "actor flush identity is not bound to the exact response occurrence",
            ));
        }

        let source_key = flush_identity.source_key();
        let projection = CertifiedMergeSidecarChunkFlushProjection {
            semantic_target: flush_identity.semantic_target().clone(),
            authenticated_source: flush_identity.authenticated_source_peer().clone(),
            source_key_identity: source_key.process_local_identity_hash(),
            delivery_route_identity: flush_identity.process_local_route_identity_hash(),
            writer_occurrence_identity: flush_identity
                .process_local_writer_occurrence_identity_hash(),
            connection_tenure_ordinal: flush_identity.connection_tenure_ordinal(),
            delivery_ordinal: flush_identity.delivery_ordinal(),
            ticket_id: flush_identity.ticket_id(),
            ticket_rank: flush_identity.ticket_rank(),
            ticket_topic: flush_identity.ticket_topic(),
            reply_writer_timeout_attempt: flush_identity.reply_writer_timeout_attempt(),
            canonical_request_digest: flush_identity.canonical_request_digest(),
            stream_wire_bytes: flush_identity.ticket_stream_wire_bytes(),
            request_id: chunk.request_id,
            service_generation: chunk.service_generation,
            stream_epoch: chunk.stream_epoch,
            semantic_sequence: chunk.semantic_sequence,
            entry_hash: chunk.entry_hash,
            encoded_len: chunk.encoded_len,
            epoch_id: chunk.epoch_id,
            reference_digest: chunk.reference_digest,
            requester: chunk.requester.clone(),
            responder: chunk.responder.clone(),
            canonical_response_hash: HashOf::new(&post.data),
            sidecar_response_hash: HashOf::new(message.as_ref()),
            chunk_hash: HashOf::new(chunk),
            payload_digest: Hash::new_from_chunks(&[
                CHUNK_PAYLOAD_DIGEST_DOMAIN,
                chunk.bytes.as_slice(),
            ]),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
            message_cursor_before,
            message_cursor_after,
            chunk_cursor_before,
            chunk_cursor_after,
        };
        Ok(Self {
            projection,
            source_key,
            flush_identity: flush_identity.clone(),
            confirmed_worker_trace: None,
        })
    }

    /// Retain the exact successful worker transition for lane-side linkage.
    ///
    /// The worker may bind this process-local witness once, and only after the
    /// same pure kernel accepts a successful writer flush. Lane application
    /// subsequently checks the retained occurrence before claiming the writer
    /// completion and again against the fully observed post-state.
    pub(crate) fn bind_confirmed_worker_trace(
        &mut self,
        trace: ProductionReliableFlushTraceProjection,
    ) -> Result<(), MergeSidecarError> {
        if self.confirmed_worker_trace.is_some() {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "successful worker flush trace was already bound",
            ));
        }
        let occurrence = reliable_flush_application_occurrence_projection(self)?;
        if trace.status != 2 {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "worker flush trace is not the accepted transition for this occurrence",
            ));
        }
        let checked_worker = check_production_reliable_flush_worker_transition(trace).ok_or(
            MergeSidecarError::FlushIdentityMismatch(
                "worker flush trace is not the accepted transition for this occurrence",
            ),
        )?;
        let checked_link = check_production_reliable_flush_link_transition(trace, occurrence)
            .ok_or(MergeSidecarError::FlushIdentityMismatch(
                "worker flush trace is not the accepted transition for this occurrence",
            ))?;
        let trace = checked_worker.into_projection();
        let (linked_worker, linked_occurrence) = checked_link.into_projection();
        if linked_worker != trace || linked_occurrence != occurrence {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "checked worker flush token changed its exact occurrence",
            ));
        }
        self.confirmed_worker_trace = Some(trace);
        Ok(())
    }

    /// Immutable exact identity projection consumed by the shared pure kernel.
    #[must_use]
    pub(crate) fn projection(&self) -> &CertifiedMergeSidecarChunkFlushProjection {
        &self.projection
    }

    /// Whether `ack_identity` is the exact actor completion queued here.
    #[must_use]
    pub(crate) fn matches_ack_identity(&self, ack_identity: &NetworkReplyFlushIdentity) -> bool {
        self.flush_identity
            .same_writer_flush_occurrence(ack_identity)
            && self.projection_matches_identity(ack_identity)
    }

    fn projection_matches_identity(&self, identity: &NetworkReplyFlushIdentity) -> bool {
        let projection = &self.projection;
        projection.semantic_target == *identity.semantic_target()
            && projection.authenticated_source == *identity.authenticated_source_peer()
            && self.source_key == identity.source_key()
            && projection.source_key_identity == self.source_key.process_local_identity_hash()
            && projection.source_key_identity == identity.source_key().process_local_identity_hash()
            && projection.delivery_route_identity == identity.process_local_route_identity_hash()
            && projection.writer_occurrence_identity
                == identity.process_local_writer_occurrence_identity_hash()
            && projection.connection_tenure_ordinal == identity.connection_tenure_ordinal()
            && projection.delivery_ordinal == identity.delivery_ordinal()
            && projection.ticket_id == identity.ticket_id()
            && projection.ticket_rank == identity.ticket_rank()
            && projection.ticket_topic == identity.ticket_topic()
            && projection.reply_writer_timeout_attempt == identity.reply_writer_timeout_attempt()
            && projection.canonical_request_digest == identity.canonical_request_digest()
            && projection.stream_wire_bytes == identity.ticket_stream_wire_bytes()
    }

    /// Whether the retained actor identity is bound to this live attempt tenure.
    #[must_use]
    pub(crate) fn is_bound_to_attempt(&self, route: &NetworkReplyRoute) -> bool {
        self.flush_identity.is_bound_to_tenure(route)
            && self.projection.semantic_target == *route.semantic_target()
            && self.source_key == route.source_key()
    }

    /// Whether this terminal flush belongs to the same semantic source attempt.
    ///
    /// Unlike [`Self::is_bound_to_attempt`], this deliberately ignores the
    /// connection tenure. A reconnect can replace writer authority while the
    /// old writer's successful flush acknowledgement is still crossing the
    /// runner boundary; that terminal receipt must advance the shared source
    /// cursor exactly once.
    #[must_use]
    pub(crate) fn is_bound_to_source(&self, route: &NetworkReplyRoute) -> bool {
        self.is_bound_to_attempt(route)
            || (self.projection.semantic_target == *route.semantic_target()
                && self.source_key == route.source_key())
    }

    /// Whether one cached materialized carrier is the exact admitted response.
    pub(crate) fn matches_materialized_chunk(
        &self,
        message: &Arc<CertifiedMergeSidecarMessage>,
    ) -> bool {
        let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {
            return false;
        };
        let Ok(chunk_cursor_before) = usize::try_from(chunk.chunk_index) else {
            return false;
        };
        let Some(chunk_cursor_after) = chunk_cursor_before.checked_add(1) else {
            return false;
        };
        let data = crate::NetworkMessage::CertifiedMergeSidecar(Arc::clone(message));
        let post = Post {
            data: data.clone(),
            peer_id: chunk.requester.clone(),
            priority: Priority::High,
        };
        let projection = &self.projection;
        projection.semantic_target == chunk.requester
            && projection.request_id == chunk.request_id
            && projection.service_generation == chunk.service_generation
            && projection.stream_epoch == chunk.stream_epoch
            && projection.semantic_sequence == chunk.semantic_sequence
            && projection.entry_hash == chunk.entry_hash
            && projection.encoded_len == chunk.encoded_len
            && projection.epoch_id == chunk.epoch_id
            && projection.reference_digest == chunk.reference_digest
            && projection.requester == chunk.requester
            && projection.responder == chunk.responder
            && projection.canonical_response_hash == HashOf::new(&data)
            && projection.sidecar_response_hash == HashOf::new(message.as_ref())
            && projection.chunk_hash == HashOf::new(chunk)
            && projection.payload_digest
                == Hash::new_from_chunks(&[CHUNK_PAYLOAD_DIGEST_DOMAIN, chunk.bytes.as_slice()])
            && projection.chunk_index == chunk.chunk_index
            && projection.chunk_count == chunk.chunk_count
            && projection.chunk_cursor_before == chunk_cursor_before
            && projection.chunk_cursor_after == chunk_cursor_after
            && projection.message_cursor_before.checked_add(1)
                == Some(projection.message_cursor_after)
            && projection.ticket_topic == post.data.topic()
            && self.flush_identity.is_bound_to_canonical_reply(&post)
    }
}

/// Digest the exact compact reference carried by a global block.
#[must_use]
pub fn certified_merge_reference_digest(reference: &CertifiedMergeLedgerReference) -> Hash {
    let bytes = norito::to_bytes(reference)
        .expect("certified merge reference must have a canonical Norito encoding");
    Hash::new_from_chunks(&[REFERENCE_DIGEST_DOMAIN, bytes.as_slice()])
}

/// Return the exact peers selected by a canonical merge-QC signer bitmap.
///
/// This helper validates bitmap length and padding independently of block
/// admission so transport authorization never relies on an unchecked sender
/// claim.
pub fn certified_merge_sidecar_holders(
    reference: &CertifiedMergeLedgerReference,
) -> Result<Vec<PeerId>, MergeSidecarError> {
    if reference.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
        return Err(MergeSidecarError::UnsupportedVersion(reference.version));
    }
    let roster = &reference.merge_qc.validator_set;
    if roster.is_empty() {
        return Err(MergeSidecarError::MalformedReference("empty validator set"));
    }
    if reference.merge_qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
        return Err(MergeSidecarError::MalformedReference(
            "unsupported validator-set hash version",
        ));
    }
    if reference.merge_qc.validator_set_hash != HashOf::new(roster) {
        return Err(MergeSidecarError::MalformedReference(
            "validator-set hash does not match roster",
        ));
    }
    let mut roster_unique = BTreeSet::new();
    if roster.iter().any(|peer| !roster_unique.insert(peer)) {
        return Err(MergeSidecarError::MalformedReference(
            "validator set contains duplicate peers",
        ));
    }
    let expected_len = roster.len().div_ceil(8);
    if reference.merge_qc.signers_bitmap.len() != expected_len {
        return Err(MergeSidecarError::MalformedReference(
            "signer bitmap length does not match validator set",
        ));
    }
    if roster.len() % 8 != 0 {
        let used_bits = roster.len() % 8;
        let padding_mask = !((1_u8 << used_bits) - 1);
        if reference.merge_qc.signers_bitmap[expected_len - 1] & padding_mask != 0 {
            return Err(MergeSidecarError::MalformedReference(
                "signer bitmap has non-zero padding",
            ));
        }
    }
    let mut holders = Vec::new();
    for (byte_index, byte) in reference
        .merge_qc
        .signers_bitmap
        .iter()
        .copied()
        .enumerate()
    {
        for bit in 0_u8..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let index = byte_index * 8 + usize::from(bit);
            let Some(peer) = roster.get(index) else {
                return Err(MergeSidecarError::MalformedReference(
                    "signer bitmap selects an out-of-bounds validator",
                ));
            };
            holders.push(peer.clone());
        }
    }
    if holders.is_empty() {
        return Err(MergeSidecarError::MalformedReference(
            "signer bitmap selects no holders",
        ));
    }
    Ok(holders)
}

/// Decode exact canonical bytes and prove that they match the block-carried reference.
pub fn decode_certified_merge_sidecar(
    reference: &CertifiedMergeLedgerReference,
    bytes: &[u8],
) -> Result<MergeLedgerEntry, MergeSidecarError> {
    let expected_len = usize::try_from(reference.encoded_len)
        .map_err(|_| MergeSidecarError::InvalidEncodedLength(reference.encoded_len))?;
    if expected_len == 0
        || expected_len > MAX_MERGE_LEDGER_ENTRY_BYTES
        || bytes.len() != expected_len
    {
        return Err(MergeSidecarError::LengthMismatch {
            expected: expected_len,
            actual: bytes.len(),
        });
    }
    let entry = norito::decode_from_bytes::<MergeLedgerEntry>(bytes)
        .map_err(|error| MergeSidecarError::Decode(error.to_string()))?;
    if !entry.has_current_version() {
        return Err(MergeSidecarError::Decode(format!(
            "unsupported merge ledger entry version {}",
            entry.version
        )));
    }
    if entry.canonical_bytes() != bytes {
        return Err(MergeSidecarError::NonCanonicalEncoding);
    }
    if !reference.matches_entry(&entry) {
        return Err(MergeSidecarError::ReferenceMismatch);
    }
    Ok(entry)
}

/// Certified merge-sidecar protocol rejection.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MergeSidecarError {
    /// Unsupported wire version.
    #[error("unsupported certified merge-sidecar version {0}")]
    UnsupportedVersion(u8),
    /// A compact reference is structurally unsafe for holder selection.
    #[error("malformed certified merge reference: {0}")]
    MalformedReference(&'static str),
    /// The committed full-entry length is outside protocol bounds.
    #[error("invalid certified merge-sidecar encoded length {0}")]
    InvalidEncodedLength(u64),
    /// Message envelope identity differs from its declared identity.
    #[error("certified merge-sidecar peer identity mismatch")]
    PeerIdentityMismatch,
    /// Message was not solicited by an active exact request.
    #[error("unsolicited or stale certified merge-sidecar response")]
    UnsolicitedResponse,
    /// Process-local actor flush identity differs from the queued exact response.
    #[error("certified merge-sidecar reliable flush identity mismatch: {0}")]
    FlushIdentityMismatch(&'static str),
    /// Response did not come from the selected QC signer.
    #[error("certified merge-sidecar response sender is not the selected holder")]
    UnexpectedResponder,
    /// Request identifier differs from the active request.
    #[error("certified merge-sidecar request identifier mismatch")]
    RequestIdMismatch,
    /// A close or acknowledgement did not match its canonical stream witness.
    #[error("certified merge-sidecar close identifier mismatch")]
    CloseIdMismatch,
    /// Response metadata differs from the requested reference.
    #[error("certified merge-sidecar response metadata mismatch")]
    MetadataMismatch,
    /// Chunk count/index/length violates fixed-boundary framing.
    #[error("invalid certified merge-sidecar chunk framing: {0}")]
    InvalidChunk(&'static str),
    /// A chunk index was already received, including identical duplicates.
    #[error("duplicate certified merge-sidecar chunk {0}")]
    DuplicateChunk(u32),
    /// A configured in-memory session or byte cap was reached.
    #[error("certified merge-sidecar transport capacity reached: {0}")]
    Capacity(&'static str),
    /// Deferred block is stale or implausibly far ahead of local state.
    #[error("certified merge-sidecar deferred carrier height is stale or too far ahead")]
    InvalidCarrierHeight,
    /// Reassembled bytes have an unexpected exact length.
    #[error("certified merge-sidecar length mismatch: expected {expected}, got {actual}")]
    LengthMismatch {
        /// Committed exact byte length.
        expected: usize,
        /// Reassembled byte length.
        actual: usize,
    },
    /// Full entry could not be decoded as exact framed Norito.
    #[error("certified merge-sidecar decode failed: {0}")]
    Decode(String),
    /// Decoded bytes are not the unique canonical encoding.
    #[error("certified merge-sidecar bytes are not canonical")]
    NonCanonicalEncoding,
    /// Canonical entry does not match every compact-reference field.
    #[error("certified merge-sidecar does not match the compact reference")]
    ReferenceMismatch,
    /// Durable local signing guard detected an attempted equivocation.
    #[error("merge committee local signing equivocation detected")]
    LocalSigningEquivocation,
    /// Signing-guard persistence failed closed.
    #[error("merge committee signing guard persistence failed: {0}")]
    SigningGuard(String),
    /// Durable semantic request lifecycle persistence failed closed.
    #[error("certified merge-sidecar lifecycle journal failed: {0}")]
    LifecycleJournal(String),
}

#[derive(Clone, Debug)]
struct DeferredCarrier {
    hash: HashOf<BlockHeader>,
    height: u64,
    view: u64,
}

#[derive(Clone, Debug)]
struct RequestAttempt {
    id: Hash,
    message_hash: Hash,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    holder: PeerId,
    last_progress_at: Instant,
    previous_holder_cursor: usize,
    previous_attempts: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InboundPriority {
    Ordinary,
    Decided,
}

#[derive(Debug)]
struct InboundAssembly {
    reference: CertifiedMergeLedgerReference,
    requester: PeerId,
    priority: InboundPriority,
    holders: Vec<PeerId>,
    holder_cursor: usize,
    current: Option<RequestAttempt>,
    chunks: Vec<Option<Vec<u8>>>,
    received_bytes: usize,
    attempts: u32,
    deferred: BTreeMap<HashOf<BlockHeader>, DeferredCarrier>,
    complete_pending_validation: bool,
}

#[derive(Debug)]
struct RequestStreamState {
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    next_sequence: u64,
    closed_through: u64,
    acknowledged_through: u64,
    last_close_sent_at: Option<Instant>,
    last_close_message_hash: Option<Hash>,
    open_sequences: BTreeSet<CertifiedMergeSidecarSemanticSequenceV1>,
}

impl RequestStreamState {
    fn new(stream_epoch: CertifiedMergeSidecarStreamEpochV1) -> Self {
        Self {
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch,
            next_sequence: 0,
            closed_through: 0,
            acknowledged_through: 0,
            last_close_sent_at: None,
            last_close_message_hash: None,
            open_sequences: BTreeSet::new(),
        }
    }

    fn allocate(
        &mut self,
    ) -> Result<(CertifiedMergeSidecarSemanticSequenceV1, u64), MergeSidecarError> {
        let next_sequence =
            self.next_sequence
                .checked_add(1)
                .ok_or(MergeSidecarError::Capacity(
                    "semantic request sequence exhausted",
                ))?;
        let semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
            NonZeroU64::new(next_sequence)
                .expect("a successfully incremented semantic sequence is non-zero"),
        );
        self.next_sequence = next_sequence;
        let inserted = self.open_sequences.insert(semantic_sequence);
        debug_assert!(inserted, "new semantic sequence must be unique");
        Ok((semantic_sequence, self.closed_through))
    }

    fn close(&mut self, semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1) {
        if semantic_sequence.get() > self.next_sequence {
            return;
        }
        self.open_sequences.remove(&semantic_sequence);
        let prior = self.closed_through;
        self.closed_through = self
            .open_sequences
            .first()
            .map_or(self.next_sequence, |first_open| {
                first_open.get().saturating_sub(1)
            });
        if self.closed_through > prior {
            self.last_close_sent_at = None;
            self.last_close_message_hash = None;
        }
    }

    fn close_due(&self, now: Instant, retry_after: Duration) -> bool {
        self.closed_through > self.acknowledged_through
            && self
                .last_close_sent_at
                .is_none_or(|last_sent| now.saturating_duration_since(last_sent) >= retry_after)
    }

    fn emit_close(
        &mut self,
        requester: &PeerId,
        responder: &PeerId,
        now: Instant,
    ) -> CertifiedMergeSidecarCloseV1 {
        debug_assert!(self.closed_through > self.acknowledged_through);
        self.last_close_sent_at = Some(now);
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: self.service_generation,
            stream_epoch: self.stream_epoch,
            closed_through: self.closed_through,
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        close.bind_canonical_close_id();
        self.last_close_message_hash = Some(HashOf::new(&close).into());
        close
    }

    fn acknowledge_close(&mut self, closed_through: u64) -> bool {
        if closed_through <= self.acknowledged_through || closed_through > self.closed_through {
            return false;
        }
        self.acknowledged_through = closed_through;
        if self.acknowledged_through == self.closed_through {
            self.last_close_sent_at = None;
            self.last_close_message_hash = None;
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ServerStreamState {
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    closed_through: u64,
    highest_sequence: u64,
}

type InboundSidecarKey = (HashOf<MergeLedgerEntry>, Hash);
type ServerRequestKey = (PeerId, Hash);
type OutboundAttemptKey = (ServerRequestKey, ServerRequestSource);

/// One authenticated server-stream prefix whose queued output is no longer owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CertifiedMergeSidecarClosedPrefix {
    /// Requester whose semantic occurrences were closed.
    pub(crate) requester: PeerId,
    /// Responder service generation which owned the occurrences.
    pub(crate) service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Durable stream incarnation whose occurrences were closed.
    pub(crate) stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    /// Highest contiguous semantic sequence covered by the close.
    pub(crate) closed_through: u64,
}

impl CertifiedMergeSidecarClosedPrefix {
    pub(crate) fn covers(&self, other: &Self) -> bool {
        self.requester == other.requester
            && (other.service_generation < self.service_generation
                || (other.service_generation == self.service_generation
                    && (other.stream_epoch < self.stream_epoch
                        || (other.stream_epoch == self.stream_epoch
                            && other.closed_through <= self.closed_through))))
    }
}

#[derive(Debug)]
struct OutboundTransfer {
    request: CertifiedMergeSidecarRequestV1,
    response_len: usize,
    /// Fixed-boundary wire chunks materialized once and shared by every source.
    chunks: Vec<Arc<CertifiedMergeSidecarMessage>>,
    attempts: BTreeMap<ServerRequestSource, OutboundAttempt>,
}

#[derive(Debug)]
struct OutboundAttempt {
    reply_route: Option<NetworkReplyRoute>,
    /// First chunk without a successful exact peer-writer flush receipt.
    next_chunk: usize,
    /// Chunk handed downstream while its exact peer-writer flush is pending.
    in_flight_chunk: Option<usize>,
    queued: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ServerRequestSource {
    Synthetic(PeerId),
    Authenticated(NetworkReplySourceKey),
    /// Stable authenticated hub restored without any process-local capability.
    RecoveredAuthenticated(PeerId),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ServerRequestBudgetSource {
    Synthetic(PeerId),
    Authenticated(PeerId),
}

impl ServerRequestSource {
    fn budget_source(&self) -> ServerRequestBudgetSource {
        match self {
            Self::Synthetic(peer) => ServerRequestBudgetSource::Synthetic(peer.clone()),
            Self::Authenticated(source) => {
                ServerRequestBudgetSource::Authenticated(source.authenticated_source_peer().clone())
            }
            Self::RecoveredAuthenticated(peer) => {
                ServerRequestBudgetSource::Authenticated(peer.clone())
            }
        }
    }

    fn shares_budget_with(&self, other: &Self) -> bool {
        self.budget_source() == other.budget_source()
    }
}

#[derive(Clone, Debug)]
struct ServerRequestGate {
    request: CertifiedMergeSidecarRequestV1,
    request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    source_capacity: Option<usize>,
    attempts: BTreeMap<ServerRequestSource, ServerRequestGateAttempt>,
}

#[derive(Clone, Debug)]
struct ServerRequestGateAttempt {
    reply_route: Option<NetworkReplyRoute>,
    materialization_authorized: bool,
    authorized_materialization_route: Option<NetworkReplyRoute>,
    /// This source failed to acquire immutable response output.
    ///
    /// Transient response-capacity pressure keeps the bounded route/cursor
    /// history while allowing the exact authenticated delivery to retry.
    /// Terminal pre-materialization failures retire the whole gate instead.
    materialization_retryable: bool,
    /// First chunk still lacking an exact writer-flush acknowledgement, or a
    /// terminal cursor after this authenticated source completed the transfer.
    ///
    /// Keeping completion distinct from chunk zero prevents any delivery from
    /// replaying a fully acknowledged response. Reconnect replaces only the
    /// tenure-bound writer authority; it retries a pending current chunk and
    /// leaves a completed source terminal.
    cursor: ServerResponseCursor,
    /// Hash-only identity of the current chunk handed to exact actor output.
    ///
    /// The gate retains this bounded witness after shared response bytes or a
    /// retired tenure are released. A successful actor-minted writer receipt
    /// can therefore advance the source exactly once even if it crosses a
    /// prune or reconnect boundary before lane work applies it.
    pending_flush_chunk: Option<ServerPendingChunkIdentity>,
    inserted: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ServerResponseCursor {
    Pending(usize),
    Complete,
}

/// Byte-free identity of one materialized sidecar chunk awaiting writer flush.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ServerPendingChunkIdentity {
    request_id: Hash,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    entry_hash: HashOf<MergeLedgerEntry>,
    encoded_len: u64,
    epoch_id: u64,
    reference_digest: Hash,
    requester: PeerId,
    responder: PeerId,
    canonical_response_hash: HashOf<crate::NetworkMessage>,
    sidecar_response_hash: HashOf<CertifiedMergeSidecarMessage>,
    chunk_hash: HashOf<CertifiedMergeSidecarChunkV1>,
    payload_digest: Hash,
    chunk_index: u32,
    chunk_count: u32,
    topic: Topic,
}

impl ServerPendingChunkIdentity {
    fn from_message(message: &Arc<CertifiedMergeSidecarMessage>) -> Option<Self> {
        let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {
            return None;
        };
        let data = crate::NetworkMessage::CertifiedMergeSidecar(Arc::clone(message));
        Some(Self {
            request_id: chunk.request_id,
            service_generation: chunk.service_generation,
            stream_epoch: chunk.stream_epoch,
            semantic_sequence: chunk.semantic_sequence,
            entry_hash: chunk.entry_hash,
            encoded_len: chunk.encoded_len,
            epoch_id: chunk.epoch_id,
            reference_digest: chunk.reference_digest,
            requester: chunk.requester.clone(),
            responder: chunk.responder.clone(),
            canonical_response_hash: HashOf::new(&data),
            sidecar_response_hash: HashOf::new(message.as_ref()),
            chunk_hash: HashOf::new(chunk),
            payload_digest: Hash::new_from_chunks(&[
                CHUNK_PAYLOAD_DIGEST_DOMAIN,
                chunk.bytes.as_slice(),
            ]),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
            topic: data.topic(),
        })
    }

    fn matches_admission(&self, admission: &CertifiedMergeSidecarChunkAdmission) -> bool {
        let projection = admission.projection();
        self.request_id == projection.request_id
            && self.service_generation == projection.service_generation
            && self.stream_epoch == projection.stream_epoch
            && self.semantic_sequence == projection.semantic_sequence
            && self.entry_hash == projection.entry_hash
            && self.encoded_len == projection.encoded_len
            && self.epoch_id == projection.epoch_id
            && self.reference_digest == projection.reference_digest
            && self.requester == projection.requester
            && self.responder == projection.responder
            && self.canonical_response_hash == projection.canonical_response_hash
            && self.sidecar_response_hash == projection.sidecar_response_hash
            && self.chunk_hash == projection.chunk_hash
            && self.payload_digest == projection.payload_digest
            && self.chunk_index == projection.chunk_index
            && self.chunk_count == projection.chunk_count
            && self.topic == projection.ticket_topic
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSidecarRuntimeGeometryV3 {
    reply_source_capacity: u64,
    semantic_peer_capacity: u64,
    inbound_session_capacity: u64,
    inbound_sessions_per_peer: u64,
    inbound_assembly_bytes: u64,
    inbound_assembly_bytes_per_peer: u64,
    deferred_block_capacity: u64,
    future_block_distance: u64,
    request_timeout_secs: u64,
    request_timeout_nanos: u32,
    outbound_sessions_per_source: u64,
    outbound_bytes_per_source: u64,
    server_request_gates_per_source: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct RequestStreamLifecycleV3 {
    responder: PeerId,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    next_sequence: u64,
    closed_through: u64,
    acknowledged_through: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
enum DurableServerRequestSourceV3 {
    Synthetic(PeerId),
    Authenticated(PeerId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
enum DurableServerResponseCursorV3 {
    Pending(u64),
    Complete,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct ServerPendingChunkLifecycleV3 {
    request_id: Hash,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    entry_hash: HashOf<MergeLedgerEntry>,
    encoded_len: u64,
    epoch_id: u64,
    reference_digest: Hash,
    requester: PeerId,
    responder: PeerId,
    canonical_response_hash: HashOf<crate::NetworkMessage>,
    sidecar_response_hash: HashOf<CertifiedMergeSidecarMessage>,
    chunk_hash: HashOf<CertifiedMergeSidecarChunkV1>,
    payload_digest: Hash,
    chunk_index: u32,
    chunk_count: u32,
}

impl From<&ServerPendingChunkIdentity> for ServerPendingChunkLifecycleV3 {
    fn from(identity: &ServerPendingChunkIdentity) -> Self {
        Self {
            request_id: identity.request_id,
            service_generation: identity.service_generation,
            stream_epoch: identity.stream_epoch,
            semantic_sequence: identity.semantic_sequence,
            entry_hash: identity.entry_hash,
            encoded_len: identity.encoded_len,
            epoch_id: identity.epoch_id,
            reference_digest: identity.reference_digest,
            requester: identity.requester.clone(),
            responder: identity.responder.clone(),
            canonical_response_hash: identity.canonical_response_hash,
            sidecar_response_hash: identity.sidecar_response_hash,
            chunk_hash: identity.chunk_hash,
            payload_digest: identity.payload_digest,
            chunk_index: identity.chunk_index,
            chunk_count: identity.chunk_count,
        }
    }
}

impl From<ServerPendingChunkLifecycleV3> for ServerPendingChunkIdentity {
    fn from(identity: ServerPendingChunkLifecycleV3) -> Self {
        Self {
            request_id: identity.request_id,
            service_generation: identity.service_generation,
            stream_epoch: identity.stream_epoch,
            semantic_sequence: identity.semantic_sequence,
            entry_hash: identity.entry_hash,
            encoded_len: identity.encoded_len,
            epoch_id: identity.epoch_id,
            reference_digest: identity.reference_digest,
            requester: identity.requester,
            responder: identity.responder,
            canonical_response_hash: identity.canonical_response_hash,
            sidecar_response_hash: identity.sidecar_response_hash,
            chunk_hash: identity.chunk_hash,
            payload_digest: identity.payload_digest,
            chunk_index: identity.chunk_index,
            chunk_count: identity.chunk_count,
            topic: Topic::ConsensusChunk,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct ServerRequestAttemptLifecycleV3 {
    source: DurableServerRequestSourceV3,
    cursor: DurableServerResponseCursorV3,
    pending_flush_chunk: Option<ServerPendingChunkLifecycleV3>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct ServerRequestGateLifecycleV3 {
    requester: PeerId,
    request_id: Hash,
    request: CertifiedMergeSidecarRequestV1,
    request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    source_capacity: Option<u64>,
    attempts: Vec<ServerRequestAttemptLifecycleV3>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct ServerStreamLifecycleV3 {
    requester: PeerId,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    closed_through: u64,
    highest_sequence: u64,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct UnsupportedMergeSidecarLifecyclePayloadV1 {
    version: u8,
    geometry: MergeSidecarRuntimeGeometryV3,
    next_stream_epoch: u64,
    server_service_generation: CertifiedMergeSidecarServiceGenerationV1,
    request_streams: Vec<RequestStreamLifecycleV3>,
    server_streams: Vec<ServerStreamLifecycleV3>,
    server_request_gates: Vec<ServerRequestGateLifecycleV3>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct UnsupportedMergeSidecarLifecycleSnapshotV1 {
    payload: UnsupportedMergeSidecarLifecyclePayloadV1,
    payload_hash: HashOf<UnsupportedMergeSidecarLifecyclePayloadV1>,
}

#[cfg(test)]
impl UnsupportedMergeSidecarLifecycleSnapshotV1 {
    fn new(payload: UnsupportedMergeSidecarLifecyclePayloadV1) -> Self {
        let payload_hash = HashOf::new(&payload);
        Self {
            payload,
            payload_hash,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct UnsupportedMergeSidecarLifecyclePayloadV2 {
    version: u8,
    geometry: MergeSidecarLifecycleGeometryV3,
    next_stream_epoch: u64,
    server_service_generation: CertifiedMergeSidecarServiceGenerationV1,
    materialization_requester_cursor: Option<PeerId>,
    request_streams: Vec<RequestStreamLifecycleV3>,
    server_streams: Vec<ServerStreamLifecycleV3>,
    server_request_gates: Vec<ServerRequestGateLifecycleV3>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct UnsupportedMergeSidecarLifecycleSnapshotV2 {
    payload: UnsupportedMergeSidecarLifecyclePayloadV2,
    payload_hash: HashOf<UnsupportedMergeSidecarLifecyclePayloadV2>,
}

#[cfg(test)]
impl UnsupportedMergeSidecarLifecycleSnapshotV2 {
    fn new(payload: UnsupportedMergeSidecarLifecyclePayloadV2) -> Self {
        let payload_hash = HashOf::new(&payload);
        Self {
            payload,
            payload_hash,
        }
    }
}

/// The current format fingerprints the canonical responder roster and the independently
/// bounded stream, logical-gate, and authenticated-attempt tables.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSidecarLifecycleGeometryV3 {
    runtime: MergeSidecarRuntimeGeometryV3,
    server_roster_digest: MergeSidecarRosterDigest,
    server_stream_capacity: u64,
    server_request_gate_capacity: u64,
    server_request_attempt_capacity: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSidecarLifecyclePayloadV3 {
    version: u8,
    /// Monotonic root-level commit generation owned by the journal.
    root_generation: u64,
    geometry: MergeSidecarLifecycleGeometryV3,
    next_stream_epoch: u64,
    server_service_generation: CertifiedMergeSidecarServiceGenerationV1,
    /// Last requester selected by the durable two-level materialization
    /// scheduler. Selection resumes strictly after this requester and wraps.
    materialization_requester_cursor: Option<PeerId>,
    request_streams: Vec<RequestStreamLifecycleV3>,
    server_streams: Vec<ServerStreamLifecycleV3>,
    server_request_gates: Vec<ServerRequestGateLifecycleV3>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSidecarLifecycleSnapshotV3 {
    payload: MergeSidecarLifecyclePayloadV3,
    payload_hash: HashOf<MergeSidecarLifecyclePayloadV3>,
}

impl MergeSidecarLifecycleSnapshotV3 {
    fn new(payload: MergeSidecarLifecyclePayloadV3) -> Self {
        let payload_hash = HashOf::new(&payload);
        Self {
            payload,
            payload_hash,
        }
    }

    fn integrity_is_valid(&self) -> bool {
        self.payload_hash == HashOf::new(&self.payload)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSidecarLifecycleRootHighWaterV3 {
    version: u8,
    root_generation: u64,
    /// `None` only for the durably published generation-zero bootstrap
    /// sentinel. Every committed state generation carries its exact snapshot
    /// hash.
    snapshot_hash: Option<HashOf<MergeSidecarLifecycleSnapshotV3>>,
}

impl MergeSidecarLifecycleRootHighWaterV3 {
    fn bootstrap() -> Self {
        Self {
            version: LIFECYCLE_JOURNAL_VERSION_V3,
            root_generation: 0,
            snapshot_hash: None,
        }
    }

    fn new(snapshot: &MergeSidecarLifecycleSnapshotV3) -> Self {
        debug_assert_ne!(
            snapshot.payload.root_generation, 0,
            "a committed lifecycle snapshot cannot use the bootstrap generation"
        );
        Self {
            version: LIFECYCLE_JOURNAL_VERSION_V3,
            root_generation: snapshot.payload.root_generation,
            snapshot_hash: Some(HashOf::new(snapshot)),
        }
    }

    fn is_bootstrap(&self) -> bool {
        self.version == LIFECYCLE_JOURNAL_VERSION_V3
            && self.root_generation == 0
            && self.snapshot_hash.is_none()
    }

    fn matches(&self, snapshot: &MergeSidecarLifecycleSnapshotV3) -> bool {
        self.version == LIFECYCLE_JOURNAL_VERSION_V3
            && self.root_generation != 0
            && self.root_generation == snapshot.payload.root_generation
            && self.snapshot_hash.as_ref() == Some(&HashOf::new(snapshot))
    }
}

#[cfg(unix)]
type LifecycleArtifactIdentity = (u64, u64);
#[cfg(windows)]
type LifecycleArtifactIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type LifecycleArtifactIdentity = ();

#[cfg(unix)]
type LifecycleArtifactRevision = (u64, i64, i64, i64, i64, u64, u32, u32, u32);
#[cfg(windows)]
type LifecycleArtifactRevision = (u64, u64, u64, u32, Option<u32>);
#[cfg(not(any(unix, windows)))]
type LifecycleArtifactRevision = ();

#[cfg(unix)]
fn lifecycle_artifact_identity(metadata: &fs::Metadata) -> LifecycleArtifactIdentity {
    use std::os::unix::fs::MetadataExt as _;

    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn lifecycle_artifact_identity(metadata: &fs::Metadata) -> LifecycleArtifactIdentity {
    use std::os::windows::fs::MetadataExt as _;

    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn lifecycle_artifact_identity(_metadata: &fs::Metadata) -> LifecycleArtifactIdentity {}

#[cfg(unix)]
fn lifecycle_artifact_revision(metadata: &fs::Metadata) -> LifecycleArtifactRevision {
    use std::os::unix::fs::MetadataExt as _;

    (
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.nlink(),
        metadata.mode(),
        metadata.uid(),
        metadata.gid(),
    )
}

#[cfg(windows)]
fn lifecycle_artifact_revision(metadata: &fs::Metadata) -> LifecycleArtifactRevision {
    use std::os::windows::fs::MetadataExt as _;

    (
        metadata.file_size(),
        metadata.creation_time(),
        metadata.last_write_time(),
        metadata.file_attributes(),
        metadata.number_of_links(),
    )
}

#[cfg(not(any(unix, windows)))]
fn lifecycle_artifact_revision(_metadata: &fs::Metadata) -> LifecycleArtifactRevision {}

#[cfg(unix)]
const fn lifecycle_artifact_identity_available(_identity: LifecycleArtifactIdentity) -> bool {
    true
}

#[cfg(windows)]
const fn lifecycle_artifact_identity_available(identity: LifecycleArtifactIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}

#[cfg(not(any(unix, windows)))]
const fn lifecycle_artifact_identity_available(_identity: LifecycleArtifactIdentity) -> bool {
    false
}

fn lifecycle_artifact_is_single_link(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

#[cfg(windows)]
fn lifecycle_artifact_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn lifecycle_artifact_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn lifecycle_artifact_metadata_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    let identity = lifecycle_artifact_identity(left);
    lifecycle_artifact_identity_available(identity)
        && identity == lifecycle_artifact_identity(right)
        && lifecycle_artifact_revision(left) == lifecycle_artifact_revision(right)
}

fn verify_open_lifecycle_directory(
    path: &Path,
    directory: &File,
) -> Result<LifecycleArtifactIdentity, MergeSidecarError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    let opened = directory
        .metadata()
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    let path_identity = lifecycle_artifact_identity(&path_metadata);
    let opened_identity = lifecycle_artifact_identity(&opened);
    if path_metadata.file_type().is_symlink()
        || opened.file_type().is_symlink()
        || lifecycle_artifact_is_reparse_point(&path_metadata)
        || lifecycle_artifact_is_reparse_point(&opened)
        || !path_metadata.is_dir()
        || !opened.is_dir()
        || !lifecycle_artifact_identity_available(path_identity)
        || !lifecycle_artifact_identity_available(opened_identity)
        || path_identity != opened_identity
    {
        return Err(MergeSidecarError::LifecycleJournal(format!(
            "lifecycle journal directory {} is indirect or changed identity",
            path.display()
        )));
    }
    Ok(opened_identity)
}

#[cfg(any(unix, windows))]
fn open_lifecycle_directory(path: &Path) -> Result<File, MergeSidecarError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(
            (rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::NOFOLLOW).bits() as i32,
        );
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        // `FlushFileBuffers`, which backs `File::sync_all`, requires a
        // write-capable directory handle on Windows.
        options.write(true);
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS);
    }
    let directory = options
        .open(path)
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    verify_open_lifecycle_directory(path, &directory)?;
    Ok(directory)
}

#[cfg(not(any(unix, windows)))]
fn open_lifecycle_directory(_path: &Path) -> Result<File, MergeSidecarError> {
    Err(MergeSidecarError::LifecycleJournal(
        "durable lifecycle directory synchronization is unsupported on this platform".to_owned(),
    ))
}

fn verify_open_lifecycle_regular(
    path: &Path,
    file: &File,
    artifact: &str,
) -> Result<(fs::Metadata, fs::Metadata), MergeSidecarError> {
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    let opened = file
        .metadata()
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    let path_identity = lifecycle_artifact_identity(&path_metadata);
    let opened_identity = lifecycle_artifact_identity(&opened);
    if path_metadata.file_type().is_symlink()
        || opened.file_type().is_symlink()
        || lifecycle_artifact_is_reparse_point(&path_metadata)
        || lifecycle_artifact_is_reparse_point(&opened)
        || !path_metadata.is_file()
        || !opened.is_file()
        || !lifecycle_artifact_identity_available(path_identity)
        || !lifecycle_artifact_identity_available(opened_identity)
    {
        return Err(MergeSidecarError::LifecycleJournal(format!(
            "unsafe lifecycle {artifact} artifact {}",
            path.display()
        )));
    }
    if path_identity != opened_identity {
        return Err(MergeSidecarError::LifecycleJournal(format!(
            "lifecycle {artifact} changed identity while its handle was open"
        )));
    }
    if !lifecycle_artifact_is_single_link(&path_metadata)
        || !lifecycle_artifact_is_single_link(&opened)
    {
        return Err(MergeSidecarError::LifecycleJournal(format!(
            "unsafe lifecycle {artifact} artifact {}",
            path.display()
        )));
    }
    Ok((path_metadata, opened))
}

#[cfg(any(unix, windows))]
fn open_lifecycle_regular(path: &Path, artifact: &str) -> Result<File, MergeSidecarError> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(path)
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
    verify_open_lifecycle_regular(path, &file, artifact)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_lifecycle_regular(_path: &Path, _artifact: &str) -> Result<File, MergeSidecarError> {
    Err(MergeSidecarError::LifecycleJournal(
        "durable lifecycle regular-file identity is unsupported on this platform".to_owned(),
    ))
}

/// Integrity-bound lifecycle state committed by one root-level high-water.
///
/// Successive snapshots alternate between two state slots. A slot is fsynced
/// before the root marker selects it, so an interrupted update either reopens
/// the exact predecessor or the exact successor. Selected artifacts are read
/// through no-follow handles and must retain one direct filesystem identity,
/// revision, and link from open through the bounded read. Known regular temp
/// files are discarded only after one selected pair passes transport-level
/// validation; non-regular artifacts fail closed. One process must own this
/// journal exclusively.
///
/// The root marker is first published as a generation-zero bootstrap sentinel,
/// before the state directory exists. A missing root is therefore always
/// corruption; an interrupted first commit validates its complete state before
/// selecting it.
///
/// The root marker is the local trust anchor, not an external monotonic
/// counter. Rolling back or replacing that marker (including restoring the
/// bootstrap sentinel), or rolling back the entire store root, is outside the
/// rollback guarantee.
#[derive(Debug)]
struct MergeSidecarLifecycleJournal {
    store_root: PathBuf,
    directory: PathBuf,
    max_snapshot_bytes: usize,
    committed: Option<MergeSidecarLifecycleRootHighWaterV3>,
    poisoned: bool,
    #[cfg(test)]
    fail_after_state_replace_before_directory_sync: bool,
    #[cfg(test)]
    fail_after_state_publish: bool,
    #[cfg(test)]
    fail_after_root_replace_before_store_sync: bool,
    #[cfg(test)]
    fail_after_root_publish: bool,
}

impl MergeSidecarLifecycleJournal {
    fn open(
        store_root: &Path,
        max_snapshot_bytes: usize,
    ) -> Result<(Self, Option<MergeSidecarLifecycleSnapshotV3>), MergeSidecarError> {
        for legacy in LEGACY_LIFECYCLE_JOURNAL_DIRS {
            let legacy = store_root.join(legacy);
            match fs::symlink_metadata(&legacy) {
                Ok(_) => {
                    return Err(MergeSidecarError::LifecycleJournal(format!(
                        "unsupported legacy lifecycle journal {}",
                        legacy.display()
                    )));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(MergeSidecarError::LifecycleJournal(error.to_string()));
                }
            }
        }

        let directory = store_root.join(LIFECYCLE_JOURNAL_DIR);
        let directory_exists = match fs::symlink_metadata(&directory) {
            Ok(metadata)
                if metadata.file_type().is_dir()
                    && !metadata.file_type().is_symlink()
                    && !lifecycle_artifact_is_reparse_point(&metadata) =>
            {
                // Validate the live directory handle even when a no-op reopen
                // would otherwise perform no directory fsync. In particular,
                // a Windows junction is a directory but not an admissible
                // lifecycle owner.
                drop(open_lifecycle_directory(&directory)?);
                true
            }
            Ok(_) => {
                return Err(MergeSidecarError::LifecycleJournal(format!(
                    "unsafe lifecycle journal directory {}",
                    directory.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(MergeSidecarError::LifecycleJournal(error.to_string()));
            }
        };

        let mut journal = Self {
            store_root: store_root.to_path_buf(),
            directory,
            max_snapshot_bytes,
            committed: None,
            poisoned: false,
            #[cfg(test)]
            fail_after_state_replace_before_directory_sync: false,
            #[cfg(test)]
            fail_after_state_publish: false,
            #[cfg(test)]
            fail_after_root_replace_before_store_sync: false,
            #[cfg(test)]
            fail_after_root_publish: false,
        };

        let marker_exists = Self::artifact_exists(&journal.root_high_water_path())?;
        match (directory_exists, marker_exists) {
            (false, false) => {
                journal.publish_bootstrap_marker()?;
                fs::create_dir(&journal.directory)
                    .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
                Self::sync_directory(store_root)?;
                Ok((journal, None))
            }
            (false, true) => {
                let marker = journal.decode_root_high_water(&journal.root_high_water_path())?;
                if !marker.is_bootstrap() {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "committed lifecycle root high-water survived without its V3 directory"
                            .to_owned(),
                    ));
                }
                fs::create_dir(&journal.directory)
                    .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
                Self::sync_directory(store_root)?;
                journal.committed = Some(marker);
                Ok((journal, None))
            }
            (true, false) => Err(MergeSidecarError::LifecycleJournal(
                "V3 lifecycle directory survived without its root high-water".to_owned(),
            )),
            (true, true) => {
                let marker = journal.decode_root_high_water(&journal.root_high_water_path())?;
                if marker.is_bootstrap() {
                    journal.committed = Some(marker);
                    let candidate = journal.bootstrap_candidate()?;
                    return Ok((journal, candidate));
                }
                let (snapshot, marker) = journal.load_pair_strict()?;
                journal.committed = Some(marker);
                Ok((journal, Some(snapshot)))
            }
        }
    }

    #[cfg(test)]
    fn state_path(&self) -> PathBuf {
        let generation = self
            .committed
            .as_ref()
            .map_or(1, |marker| marker.root_generation.max(1));
        self.state_path_for_generation(generation)
    }

    fn state_path_for_generation(&self, generation: u64) -> PathBuf {
        let slot = usize::from((generation & 1) != 0);
        self.directory.join(LIFECYCLE_JOURNAL_SLOT_FILES[slot])
    }

    fn temp_path(&self) -> PathBuf {
        self.directory.join(LIFECYCLE_JOURNAL_TEMP)
    }

    fn root_high_water_path(&self) -> PathBuf {
        self.store_root.join(LIFECYCLE_ROOT_HIGH_WATER_FILE)
    }

    fn root_high_water_temp_path(&self) -> PathBuf {
        self.store_root.join(LIFECYCLE_ROOT_HIGH_WATER_TEMP)
    }

    fn publish_bootstrap_marker(&mut self) -> Result<(), MergeSidecarError> {
        Self::remove_regular_artifact(
            &self.root_high_water_temp_path(),
            &self.store_root,
            "root high-water temp",
        )?;
        let marker = MergeSidecarLifecycleRootHighWaterV3::bootstrap();
        let marker_bytes = norito::to_bytes(&marker)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        if marker_bytes.is_empty() || marker_bytes.len() > LIFECYCLE_ROOT_HIGH_WATER_MAX_BYTES {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle bootstrap marker exceeds its geometry".to_owned(),
            ));
        }
        Self::write_new_synced(&self.root_high_water_temp_path(), &marker_bytes)?;
        Self::persist_atomic_replacement(
            &self.root_high_water_temp_path(),
            &self.root_high_water_path(),
        )?;
        Self::sync_directory(&self.store_root)?;
        self.committed = Some(marker);
        Ok(())
    }

    fn bootstrap_candidate(
        &self,
    ) -> Result<Option<MergeSidecarLifecycleSnapshotV3>, MergeSidecarError> {
        self.validate_directory_entries()?;
        let even_slot = self.state_path_for_generation(0);
        if Self::artifact_exists(&even_slot)? {
            return Err(MergeSidecarError::LifecycleJournal(
                "bootstrap lifecycle root retained a non-initial state slot".to_owned(),
            ));
        }
        let initial_path = self.state_path_for_generation(1);
        if !Self::artifact_exists(&initial_path)? {
            return Ok(None);
        }
        let snapshot = self.decode_snapshot(&initial_path)?;
        if snapshot.payload.root_generation != 1 {
            return Err(MergeSidecarError::LifecycleJournal(
                "bootstrap lifecycle candidate is not generation one".to_owned(),
            ));
        }
        Ok(Some(snapshot))
    }

    /// Publish or clean a selected snapshot only after it has passed all
    /// transport-level geometry and semantic validation.
    fn finalize_validated_open(
        &mut self,
        validated: Option<&MergeSidecarLifecycleSnapshotV3>,
    ) -> Result<(), MergeSidecarError> {
        let Some(committed) = self.committed.clone() else {
            return Err(MergeSidecarError::LifecycleJournal(
                "restored lifecycle journal has no committed root".to_owned(),
            ));
        };
        if committed.is_bootstrap() {
            let live = self.decode_root_high_water(&self.root_high_water_path())?;
            if live != committed {
                return Err(MergeSidecarError::LifecycleJournal(
                    "bootstrap lifecycle root changed during validation".to_owned(),
                ));
            }
            match validated {
                Some(snapshot) => {
                    let live_snapshot = self.decode_snapshot(&self.state_path_for_generation(1))?;
                    if snapshot.payload.root_generation != 1 || &live_snapshot != snapshot {
                        return Err(MergeSidecarError::LifecycleJournal(
                            "bootstrap lifecycle candidate changed during validation".to_owned(),
                        ));
                    }
                    self.validate_known_temps()?;
                    // The candidate may have survived a crash after its atomic
                    // slot replacement but before the state-directory fsync.
                    // Make that directory entry durable before the root adopts
                    // it.
                    Self::sync_directory(&self.directory)?;
                    Self::remove_regular_artifact(
                        &self.root_high_water_temp_path(),
                        &self.store_root,
                        "root high-water temp",
                    )?;
                    let marker = MergeSidecarLifecycleRootHighWaterV3::new(snapshot);
                    let marker_bytes = norito::to_bytes(&marker)
                        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
                    if marker_bytes.is_empty()
                        || marker_bytes.len() > LIFECYCLE_ROOT_HIGH_WATER_MAX_BYTES
                    {
                        return Err(MergeSidecarError::LifecycleJournal(
                            "lifecycle root high-water exceeds its geometry".to_owned(),
                        ));
                    }
                    Self::write_new_synced(&self.root_high_water_temp_path(), &marker_bytes)?;
                    Self::persist_atomic_replacement(
                        &self.root_high_water_temp_path(),
                        &self.root_high_water_path(),
                    )?;
                    Self::sync_directory(&self.store_root)?;
                    self.committed = Some(marker);
                    Self::remove_regular_artifact(
                        &self.temp_path(),
                        &self.directory,
                        "state temp",
                    )?;
                }
                None => {
                    if self.bootstrap_candidate()?.is_some() {
                        return Err(MergeSidecarError::LifecycleJournal(
                            "bootstrap lifecycle candidate appeared during validation".to_owned(),
                        ));
                    }
                    self.discard_uncommitted_temps()?;
                }
            }
            return Ok(());
        }
        let Some(validated) = validated else {
            return Err(MergeSidecarError::LifecycleJournal(
                "committed lifecycle root has no validated snapshot".to_owned(),
            ));
        };
        let (live_snapshot, live_marker) = self.load_pair_strict()?;
        if live_marker != committed || &live_snapshot != validated {
            return Err(MergeSidecarError::LifecycleJournal(
                "committed lifecycle pair changed during validation".to_owned(),
            ));
        }
        self.validate_known_temps()?;
        Self::validate_regular_artifact_if_present(
            &self.state_path_for_generation(committed.root_generation ^ 1),
            "uncommitted state slot",
        )?;
        // A prior process may have crashed after atomically replacing either
        // selected artifact but before syncing its parent. Cement the selected
        // state first and then its root before deleting the predecessor or any
        // publication debris. Otherwise a second crash could roll the root
        // directory entry back after startup has already removed the state it
        // names.
        Self::sync_directory(&self.directory)?;
        Self::sync_directory(&self.store_root)?;
        self.discard_uncommitted_temps()?;
        self.discard_inactive_slot()
    }

    fn sync_directory(path: &Path) -> Result<(), MergeSidecarError> {
        let directory = open_lifecycle_directory(path)?;
        let identity = verify_open_lifecycle_directory(path, &directory)?;
        directory
            .sync_all()
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        if verify_open_lifecycle_directory(path, &directory)? != identity {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "lifecycle journal directory {} changed while synchronizing",
                path.display()
            )));
        }
        Ok(())
    }

    fn artifact_exists(path: &Path) -> Result<bool, MergeSidecarError> {
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(MergeSidecarError::LifecycleJournal(error.to_string())),
        }
    }

    fn reject_artifact_if_present(path: &Path, artifact: &str) -> Result<(), MergeSidecarError> {
        if Self::artifact_exists(path)? {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "unsafe lifecycle journal temp artifact: incomplete {artifact} {} requires operator recovery",
                path.display()
            )));
        }
        Ok(())
    }

    fn reject_known_temps(&self) -> Result<(), MergeSidecarError> {
        Self::reject_artifact_if_present(&self.temp_path(), "state temp")?;
        Self::reject_artifact_if_present(&self.root_high_water_temp_path(), "root high-water temp")
    }

    fn remove_regular_artifact(
        path: &Path,
        parent: &Path,
        artifact: &str,
    ) -> Result<(), MergeSidecarError> {
        if !Self::validate_regular_artifact_if_present(path, artifact)? {
            return Ok(());
        }
        fs::remove_file(path)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        Self::sync_directory(parent)
    }

    fn validate_regular_artifact_if_present(
        path: &Path,
        artifact: &str,
    ) -> Result<bool, MergeSidecarError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(MergeSidecarError::LifecycleJournal(error.to_string()));
            }
        };
        if metadata.file_type().is_symlink()
            || lifecycle_artifact_is_reparse_point(&metadata)
            || !metadata.file_type().is_file()
        {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "unsafe lifecycle {artifact} artifact {}",
                path.display()
            )));
        }
        Ok(true)
    }

    fn validate_known_temps(&self) -> Result<(), MergeSidecarError> {
        Self::validate_regular_artifact_if_present(&self.temp_path(), "state temp")?;
        Self::validate_regular_artifact_if_present(
            &self.root_high_water_temp_path(),
            "root high-water temp",
        )?;
        Ok(())
    }

    fn discard_uncommitted_temps(&self) -> Result<(), MergeSidecarError> {
        self.validate_known_temps()?;
        Self::remove_regular_artifact(&self.temp_path(), &self.directory, "state temp")?;
        Self::remove_regular_artifact(
            &self.root_high_water_temp_path(),
            &self.store_root,
            "root high-water temp",
        )
    }

    fn discard_inactive_slot(&self) -> Result<(), MergeSidecarError> {
        let Some(committed) = &self.committed else {
            return Ok(());
        };
        let inactive_generation = committed.root_generation ^ 1;
        Self::remove_regular_artifact(
            &self.state_path_for_generation(inactive_generation),
            &self.directory,
            "uncommitted state slot",
        )
    }

    fn validate_directory_entries(&self) -> Result<(), MergeSidecarError> {
        for entry in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?
        {
            let entry =
                entry.map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !LIFECYCLE_JOURNAL_SLOT_FILES.contains(&name.as_ref())
                && name != LIFECYCLE_JOURNAL_TEMP
            {
                return Err(MergeSidecarError::LifecycleJournal(format!(
                    "unknown artifact in V3 lifecycle directory: {}",
                    entry.path().display()
                )));
            }
        }
        Ok(())
    }

    fn read_bounded_regular(
        path: &Path,
        max_bytes: usize,
        artifact: &str,
    ) -> Result<Vec<u8>, MergeSidecarError> {
        let path_before = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        if path_before.file_type().is_symlink()
            || lifecycle_artifact_is_reparse_point(&path_before)
            || !path_before.file_type().is_file()
            || !lifecycle_artifact_is_single_link(&path_before)
        {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "unsafe lifecycle {artifact} artifact {}",
                path.display()
            )));
        }
        let len = usize::try_from(path_before.len()).map_err(|_| {
            MergeSidecarError::LifecycleJournal(format!(
                "lifecycle {artifact} length is not representable"
            ))
        })?;
        if len == 0 || len > max_bytes {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "lifecycle {artifact} length exceeds its geometry"
            )));
        }
        let mut file = open_lifecycle_regular(path, artifact)?;
        let (path_opened, opened_before) = verify_open_lifecycle_regular(path, &file, artifact)?;
        if !lifecycle_artifact_metadata_unchanged(&path_before, &path_opened)
            || !lifecycle_artifact_metadata_unchanged(&path_before, &opened_before)
        {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "lifecycle {artifact} changed while opening"
            )));
        }

        let read_limit = u64::try_from(max_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::with_capacity(len);
        Read::by_ref(&mut file)
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        let (path_after, opened_after) = verify_open_lifecycle_regular(path, &file, artifact)?;
        let bytes_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if bytes.is_empty()
            || bytes.len() > max_bytes
            || opened_after.len() != bytes_len
            || !lifecycle_artifact_metadata_unchanged(&opened_before, &opened_after)
            || !lifecycle_artifact_metadata_unchanged(&opened_before, &path_after)
        {
            return Err(MergeSidecarError::LifecycleJournal(format!(
                "lifecycle {artifact} changed while reading"
            )));
        }
        Ok(bytes)
    }

    fn decode_snapshot(
        &self,
        path: &Path,
    ) -> Result<MergeSidecarLifecycleSnapshotV3, MergeSidecarError> {
        let bytes = Self::read_bounded_regular(path, self.max_snapshot_bytes, "journal state")?;
        let snapshot = norito::decode_from_bytes::<MergeSidecarLifecycleSnapshotV3>(&bytes)
            .map_err(|_| {
                MergeSidecarError::LifecycleJournal(
                    "unsupported or corrupt V3 lifecycle journal; migration is not supported"
                        .to_owned(),
                )
            })?;
        let canonical = norito::to_bytes(&snapshot)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        if canonical != bytes {
            return Err(MergeSidecarError::LifecycleJournal(
                "V3 lifecycle journal is not canonical Norito".to_owned(),
            ));
        }
        if snapshot.payload.version != LIFECYCLE_JOURNAL_VERSION_V3
            || snapshot.payload.root_generation == 0
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "unsupported lifecycle journal version or zero root generation; migration is not supported"
                    .to_owned(),
            ));
        }
        if !snapshot.integrity_is_valid() {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal payload digest mismatch".to_owned(),
            ));
        }
        Ok(snapshot)
    }

    fn decode_root_high_water(
        &self,
        path: &Path,
    ) -> Result<MergeSidecarLifecycleRootHighWaterV3, MergeSidecarError> {
        let bytes = Self::read_bounded_regular(
            path,
            LIFECYCLE_ROOT_HIGH_WATER_MAX_BYTES,
            "root high-water",
        )?;
        let marker = norito::decode_from_bytes::<MergeSidecarLifecycleRootHighWaterV3>(&bytes)
            .map_err(|_| {
                MergeSidecarError::LifecycleJournal(
                    "unsupported or corrupt lifecycle root high-water".to_owned(),
                )
            })?;
        let canonical = norito::to_bytes(&marker)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        let bootstrap_shape = marker.root_generation == 0 && marker.snapshot_hash.is_none();
        let committed_shape = marker.root_generation != 0 && marker.snapshot_hash.is_some();
        if canonical != bytes
            || marker.version != LIFECYCLE_JOURNAL_VERSION_V3
            || !(bootstrap_shape || committed_shape)
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "non-canonical, unsupported, or malformed lifecycle root high-water".to_owned(),
            ));
        }
        Ok(marker)
    }

    fn load_pair_strict(
        &self,
    ) -> Result<
        (
            MergeSidecarLifecycleSnapshotV3,
            MergeSidecarLifecycleRootHighWaterV3,
        ),
        MergeSidecarError,
    > {
        self.validate_directory_entries()?;
        if !Self::artifact_exists(&self.root_high_water_path())? {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle state and root high-water are not both present".to_owned(),
            ));
        }
        let marker = self.decode_root_high_water(&self.root_high_water_path())?;
        if marker.is_bootstrap() {
            return Err(MergeSidecarError::LifecycleJournal(
                "bootstrap lifecycle root does not select a committed state".to_owned(),
            ));
        }
        let state_path = self.state_path_for_generation(marker.root_generation);
        if !Self::artifact_exists(&state_path)? {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle state and root high-water are not both present".to_owned(),
            ));
        }
        let snapshot = self.decode_snapshot(&state_path)?;
        if !marker.matches(&snapshot) {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle state and root high-water generation/hash mismatch".to_owned(),
            ));
        }
        Ok((snapshot, marker))
    }

    fn load(&self) -> Result<Option<MergeSidecarLifecycleSnapshotV3>, MergeSidecarError> {
        self.reject_known_temps()?;
        if !Self::artifact_exists(&self.root_high_water_path())? {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle state and root high-water are not both present".to_owned(),
            ));
        }
        let marker = self.decode_root_high_water(&self.root_high_water_path())?;
        if self.committed.as_ref() != Some(&marker) {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle root high-water changed outside the live journal".to_owned(),
            ));
        }
        if marker.is_bootstrap() {
            self.validate_directory_entries()?;
            for name in LIFECYCLE_JOURNAL_SLOT_FILES {
                if Self::artifact_exists(&self.directory.join(name))? {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "uncommitted lifecycle state appeared beside the live bootstrap root"
                            .to_owned(),
                    ));
                }
            }
            return Ok(None);
        }
        let (snapshot, marker) = self.load_pair_strict()?;
        if self.committed.as_ref() != Some(&marker) {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle root high-water changed outside the live journal".to_owned(),
            ));
        }
        Ok(Some(snapshot))
    }

    fn write_new_synced(path: &Path, bytes: &[u8]) -> Result<(), MergeSidecarError> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        file.sync_all()
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))
    }

    fn persist_atomic_replacement(
        temporary: &Path,
        destination: &Path,
    ) -> Result<(), MergeSidecarError> {
        // `std::fs::rename` replaces an existing destination on Unix but
        // rejects it on Windows. `TempPath::persist` uses the native atomic
        // replacement operation on both platforms. Preserve a failed temp so
        // startup reconciliation observes the incomplete publication.
        let mut temporary = tempfile::TempPath::try_from_path(temporary)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        temporary.disable_cleanup(true);
        temporary
            .persist(destination)
            .map_err(|error| MergeSidecarError::LifecycleJournal(error.error.to_string()))
    }

    fn live_generation(&self) -> Result<u64, MergeSidecarError> {
        let on_disk = self.load()?;
        match (&self.committed, on_disk.as_ref()) {
            (Some(committed), None) if committed.is_bootstrap() => Ok(0),
            (Some(committed), Some(snapshot)) if committed.matches(snapshot) => {
                Ok(committed.root_generation)
            }
            (None, None) | (None, Some(_)) | (Some(_), None) | (Some(_), Some(_)) => {
                Err(MergeSidecarError::LifecycleJournal(
                    "live lifecycle journal differs from its committed root".to_owned(),
                ))
            }
        }
    }

    fn preflight_next_commit(&mut self) -> Result<(), MergeSidecarError> {
        if self.poisoned {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal requires process restart".to_owned(),
            ));
        }
        let result = (|| {
            self.live_generation()?
                .checked_add(1)
                .filter(|generation| *generation != 0)
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "lifecycle root generation exhausted".to_owned(),
                    )
                })?;
            Ok(())
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn persist_next(
        &mut self,
        mut snapshot: MergeSidecarLifecycleSnapshotV3,
    ) -> Result<MergeSidecarLifecycleSnapshotV3, MergeSidecarError> {
        if self.poisoned {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal requires process restart".to_owned(),
            ));
        }
        let result = (|| {
            let current_generation = self.live_generation()?;
            snapshot.payload.version = LIFECYCLE_JOURNAL_VERSION_V3;
            snapshot.payload.root_generation = current_generation;
            snapshot.payload_hash = HashOf::new(&snapshot.payload);
            if self
                .committed
                .as_ref()
                .is_some_and(|marker| marker.matches(&snapshot))
            {
                return Ok(snapshot);
            }

            let next_generation = current_generation
                .checked_add(1)
                .filter(|generation| *generation != 0)
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "lifecycle root generation exhausted".to_owned(),
                    )
                })?;
            snapshot.payload.root_generation = next_generation;
            snapshot.payload_hash = HashOf::new(&snapshot.payload);
            let snapshot_bytes = norito::to_bytes(&snapshot)
                .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
            if snapshot_bytes.is_empty() || snapshot_bytes.len() > self.max_snapshot_bytes {
                return Err(MergeSidecarError::LifecycleJournal(
                    "lifecycle journal snapshot exceeds its geometry".to_owned(),
                ));
            }
            let marker = MergeSidecarLifecycleRootHighWaterV3::new(&snapshot);
            let marker_bytes = norito::to_bytes(&marker)
                .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
            if marker_bytes.is_empty() || marker_bytes.len() > LIFECYCLE_ROOT_HIGH_WATER_MAX_BYTES {
                return Err(MergeSidecarError::LifecycleJournal(
                    "lifecycle root high-water exceeds its geometry".to_owned(),
                ));
            }

            self.reject_known_temps()?;
            Self::write_new_synced(&self.temp_path(), &snapshot_bytes)?;
            let next_state_path = self.state_path_for_generation(next_generation);
            Self::persist_atomic_replacement(&self.temp_path(), &next_state_path)?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_after_state_replace_before_directory_sync) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "injected failure after lifecycle state replacement but before directory synchronization"
                        .to_owned(),
                ));
            }
            Self::sync_directory(&self.directory)?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_after_state_publish) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "injected failure after lifecycle state publication".to_owned(),
                ));
            }

            Self::write_new_synced(&self.root_high_water_temp_path(), &marker_bytes)?;
            Self::persist_atomic_replacement(
                &self.root_high_water_temp_path(),
                &self.root_high_water_path(),
            )?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_after_root_replace_before_store_sync) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "injected failure after lifecycle root replacement but before store synchronization"
                        .to_owned(),
                ));
            }
            Self::sync_directory(&self.store_root)?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_after_root_publish) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "injected failure after lifecycle root publication".to_owned(),
                ));
            }
            self.committed = Some(marker);
            // The root marker is the sole commit point. Once it is durable,
            // the predecessor slot is no longer needed for crash recovery.
            // Cleanup is best-effort because the successor is already
            // committed and returning an error would misreport its outcome.
            let predecessor_path = self.state_path_for_generation(current_generation);
            if Self::artifact_exists(&predecessor_path).unwrap_or(false)
                && fs::remove_file(&predecessor_path).is_ok()
            {
                let _ = Self::sync_directory(&self.directory);
            }
            Ok(snapshot)
        })();
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
}

/// Unambiguous process-local encoding used only by reliable-flush projections.
///
/// Every component, including fixed-width primitives, carries a u64 length
/// prefix. These bytes never enter Norito, the wire, persistence, or consensus.
#[derive(Default)]
struct ReliableFlushProjectionBytes {
    bytes: Vec<u8>,
}

impl ReliableFlushProjectionBytes {
    fn field(&mut self, bytes: &[u8]) {
        let len = u64::try_from(bytes.len())
            .expect("bounded reliable-flush projection field length fits u64");
        self.bytes.extend_from_slice(&len.to_le_bytes());
        self.bytes.extend_from_slice(bytes);
    }

    fn encoded<T: Encode>(&mut self, value: &T) {
        self.field(&value.encode());
    }

    fn bool(&mut self, value: bool) {
        self.field(&[u8::from(value)]);
    }

    fn u8(&mut self, value: u8) {
        self.field(&[value]);
    }

    fn u64(&mut self, value: u64) {
        self.field(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(u64::try_from(value).expect("bounded reliable-flush usize fits u64"));
    }

    fn hash(&mut self, value: Hash) {
        self.field(value.as_ref());
    }

    fn typed_hash<T>(&mut self, value: HashOf<T>) {
        self.field(value.as_ref());
    }

    fn source(&mut self, source: &ServerRequestSource) {
        match source {
            ServerRequestSource::Synthetic(peer) => {
                self.u8(1);
                self.encoded(peer);
            }
            ServerRequestSource::Authenticated(key) => {
                self.u8(2);
                self.hash(key.process_local_identity_hash());
            }
            ServerRequestSource::RecoveredAuthenticated(peer) => {
                self.u8(3);
                self.encoded(peer);
            }
        }
    }

    fn cursor(&mut self, cursor: ServerResponseCursor) {
        match cursor {
            ServerResponseCursor::Pending(index) => {
                self.u8(1);
                self.usize(index);
            }
            ServerResponseCursor::Complete => self.u8(2),
        }
    }

    fn pending_chunk(&mut self, pending: Option<&ServerPendingChunkIdentity>) {
        let Some(pending) = pending else {
            self.bool(false);
            return;
        };
        self.bool(true);
        self.hash(pending.request_id);
        self.u64(pending.service_generation.get());
        self.u64(pending.stream_epoch.get());
        self.u64(pending.semantic_sequence.get());
        self.typed_hash(pending.entry_hash);
        self.u64(pending.encoded_len);
        self.u64(pending.epoch_id);
        self.hash(pending.reference_digest);
        self.encoded(&pending.requester);
        self.encoded(&pending.responder);
        self.typed_hash(pending.canonical_response_hash);
        self.typed_hash(pending.sidecar_response_hash);
        self.typed_hash(pending.chunk_hash);
        self.hash(pending.payload_digest);
        self.u64(u64::from(pending.chunk_index));
        self.u64(u64::from(pending.chunk_count));
        self.u8(reliable_flush_topic_tag(pending.topic));
    }

    fn key(&mut self, key: &ServerRequestKey) {
        self.encoded(&key.0);
        self.hash(key.1);
    }

    fn finish(self, domain: &[u8]) -> Hash {
        Hash::new_from_chunks(&[domain, self.bytes.as_slice()])
    }
}

#[derive(Clone, Debug)]
struct ReliableFlushRouteIdentity(NetworkReplyRoute);

impl ReliableFlushRouteIdentity {
    fn capture(route: &NetworkReplyRoute) -> Self {
        Self(route.clone())
    }

    fn digest(&self) -> Hash {
        self.0.process_local_identity_hash()
    }
}

impl PartialEq for ReliableFlushRouteIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.0.same_delivery(&other.0)
    }
}

impl Eq for ReliableFlushRouteIdentity {}

#[derive(Clone, Debug)]
struct ReliableFlushTargetGateResidual {
    key: ServerRequestKey,
    source: ServerRequestSource,
    request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    source_capacity: Option<usize>,
    reply_route: Option<ReliableFlushRouteIdentity>,
    materialization_authorized: bool,
    authorized_materialization_route: Option<ReliableFlushRouteIdentity>,
    materialization_retryable: bool,
}

impl ReliableFlushTargetGateResidual {
    fn capture(
        key: &ServerRequestKey,
        source: &ServerRequestSource,
        gate: &ServerRequestGate,
        attempt: &ServerRequestGateAttempt,
    ) -> Self {
        Self {
            key: key.clone(),
            source: source.clone(),
            request_hash: gate.request_hash,
            service_generation: gate.service_generation,
            stream_epoch: gate.stream_epoch,
            semantic_sequence: gate.semantic_sequence,
            source_capacity: gate.source_capacity,
            reply_route: attempt
                .reply_route
                .as_ref()
                .map(ReliableFlushRouteIdentity::capture),
            materialization_authorized: attempt.materialization_authorized,
            authorized_materialization_route: attempt
                .authorized_materialization_route
                .as_ref()
                .map(ReliableFlushRouteIdentity::capture),
            materialization_retryable: attempt.materialization_retryable,
        }
    }

    fn digest(&self) -> Hash {
        let mut bytes = ReliableFlushProjectionBytes::default();
        bytes.key(&self.key);
        bytes.source(&self.source);
        bytes.typed_hash(self.request_hash);
        bytes.u64(self.service_generation.get());
        bytes.u64(self.stream_epoch.get());
        bytes.u64(self.semantic_sequence.get());
        if let Some(capacity) = self.source_capacity {
            bytes.bool(true);
            bytes.usize(capacity);
        } else {
            bytes.bool(false);
        }
        if let Some(route) = &self.reply_route {
            bytes.bool(true);
            bytes.hash(route.digest());
        } else {
            bytes.bool(false);
        }
        bytes.bool(self.materialization_authorized);
        if let Some(route) = &self.authorized_materialization_route {
            bytes.bool(true);
            bytes.hash(route.digest());
        } else {
            bytes.bool(false);
        }
        bytes.bool(self.materialization_retryable);
        bytes.finish(RELIABLE_FLUSH_TARGET_GATE_DIGEST_DOMAIN)
    }
}

impl PartialEq for ReliableFlushTargetGateResidual {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.source == other.source
            && self.request_hash == other.request_hash
            && self.service_generation == other.service_generation
            && self.stream_epoch == other.stream_epoch
            && self.semantic_sequence == other.semantic_sequence
            && self.source_capacity == other.source_capacity
            && self.reply_route == other.reply_route
            && self.materialization_authorized == other.materialization_authorized
            && self.authorized_materialization_route == other.authorized_materialization_route
            && self.materialization_retryable == other.materialization_retryable
    }
}

impl Eq for ReliableFlushTargetGateResidual {}

#[derive(Clone, Debug)]
struct ReliableFlushTargetOutboundResidual {
    key: ServerRequestKey,
    source: ServerRequestSource,
    reply_route: Option<ReliableFlushRouteIdentity>,
}

impl ReliableFlushTargetOutboundResidual {
    fn capture(
        key: &ServerRequestKey,
        source: &ServerRequestSource,
        attempt: &OutboundAttempt,
    ) -> Self {
        Self {
            key: key.clone(),
            source: source.clone(),
            reply_route: attempt
                .reply_route
                .as_ref()
                .map(ReliableFlushRouteIdentity::capture),
        }
    }

    fn digest(&self) -> Hash {
        let mut bytes = ReliableFlushProjectionBytes::default();
        bytes.key(&self.key);
        bytes.source(&self.source);
        if let Some(route) = &self.reply_route {
            bytes.bool(true);
            bytes.hash(route.digest());
        } else {
            bytes.bool(false);
        }
        bytes.finish(RELIABLE_FLUSH_TARGET_OUTBOUND_DIGEST_DOMAIN)
    }
}

impl PartialEq for ReliableFlushTargetOutboundResidual {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.source == other.source
            && self.reply_route == other.reply_route
    }
}

impl Eq for ReliableFlushTargetOutboundResidual {}

#[derive(Clone, Debug)]
struct ReliableFlushChunkArcIdentity {
    message: Arc<CertifiedMergeSidecarMessage>,
    payload_len: usize,
    request_id: Hash,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    entry_hash: HashOf<MergeLedgerEntry>,
    encoded_len: u64,
    epoch_id: u64,
    reference_digest: Hash,
    requester: PeerId,
    responder: PeerId,
    chunk_index: u32,
    chunk_count: u32,
}

impl ReliableFlushChunkArcIdentity {
    fn capture(message: &Arc<CertifiedMergeSidecarMessage>) -> Self {
        let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {
            unreachable!("outbound shared transfer contains only certified chunks")
        };
        Self {
            message: Arc::clone(message),
            payload_len: chunk.bytes.len(),
            request_id: chunk.request_id,
            service_generation: chunk.service_generation,
            stream_epoch: chunk.stream_epoch,
            semantic_sequence: chunk.semantic_sequence,
            entry_hash: chunk.entry_hash,
            encoded_len: chunk.encoded_len,
            epoch_id: chunk.epoch_id,
            reference_digest: chunk.reference_digest,
            requester: chunk.requester.clone(),
            responder: chunk.responder.clone(),
            chunk_index: chunk.chunk_index,
            chunk_count: chunk.chunk_count,
        }
    }

    fn append_to(&self, bytes: &mut ReliableFlushProjectionBytes) {
        bytes.usize(Arc::as_ptr(&self.message) as usize);
        bytes.usize(self.payload_len);
        bytes.hash(self.request_id);
        bytes.u64(self.service_generation.get());
        bytes.u64(self.stream_epoch.get());
        bytes.u64(self.semantic_sequence.get());
        bytes.typed_hash(self.entry_hash);
        bytes.u64(self.encoded_len);
        bytes.u64(self.epoch_id);
        bytes.hash(self.reference_digest);
        bytes.encoded(&self.requester);
        bytes.encoded(&self.responder);
        bytes.u64(u64::from(self.chunk_index));
        bytes.u64(u64::from(self.chunk_count));
    }
}

impl PartialEq for ReliableFlushChunkArcIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.message, &other.message)
            && self.payload_len == other.payload_len
            && self.request_id == other.request_id
            && self.service_generation == other.service_generation
            && self.stream_epoch == other.stream_epoch
            && self.semantic_sequence == other.semantic_sequence
            && self.entry_hash == other.entry_hash
            && self.encoded_len == other.encoded_len
            && self.epoch_id == other.epoch_id
            && self.reference_digest == other.reference_digest
            && self.requester == other.requester
            && self.responder == other.responder
            && self.chunk_index == other.chunk_index
            && self.chunk_count == other.chunk_count
    }
}

impl Eq for ReliableFlushChunkArcIdentity {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReliableFlushSharedTransferSnapshot {
    request: CertifiedMergeSidecarRequestV1,
    response_len: usize,
    chunks: Vec<ReliableFlushChunkArcIdentity>,
}

impl ReliableFlushSharedTransferSnapshot {
    fn capture(transfer: &OutboundTransfer) -> Self {
        Self {
            request: transfer.request.clone(),
            response_len: transfer.response_len,
            chunks: transfer
                .chunks
                .iter()
                .map(ReliableFlushChunkArcIdentity::capture)
                .collect(),
        }
    }

    fn digest(&self) -> Hash {
        let mut bytes = ReliableFlushProjectionBytes::default();
        bytes.typed_hash(HashOf::new(&self.request));
        bytes.usize(self.response_len);
        bytes.usize(self.chunks.len());
        for chunk in &self.chunks {
            chunk.append_to(&mut bytes);
        }
        bytes.finish(RELIABLE_FLUSH_SHARED_TRANSFER_DIGEST_DOMAIN)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReliableFlushSiblingGateSnapshot {
    key: ServerRequestKey,
    request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    service_generation: CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    source_capacity: Option<usize>,
    source: ServerRequestSource,
    reply_route: Option<ReliableFlushRouteIdentity>,
    materialization_authorized: bool,
    authorized_materialization_route: Option<ReliableFlushRouteIdentity>,
    materialization_retryable: bool,
    cursor: ServerResponseCursor,
    pending_flush_chunk: Option<ServerPendingChunkIdentity>,
    inserted: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReliableFlushSiblingOutboundSnapshot {
    key: ServerRequestKey,
    source: ServerRequestSource,
    reply_route: Option<ReliableFlushRouteIdentity>,
    next_chunk: usize,
    in_flight_chunk: Option<usize>,
    queued: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReliableFlushSiblingStateSnapshot {
    gates: Vec<ReliableFlushSiblingGateSnapshot>,
    transfers: Vec<(ServerRequestKey, ReliableFlushSharedTransferSnapshot)>,
    outbound: Vec<ReliableFlushSiblingOutboundSnapshot>,
    order: Vec<OutboundAttemptKey>,
}

impl ReliableFlushSiblingStateSnapshot {
    fn capture(
        transport: &MergeSidecarTransport,
        target_key: &ServerRequestKey,
        target_source: &ServerRequestSource,
    ) -> Self {
        let mut gates = Vec::new();
        for (key, gate) in &transport.server_request_gates {
            for (source, attempt) in &gate.attempts {
                if key == target_key && source == target_source {
                    continue;
                }
                gates.push(ReliableFlushSiblingGateSnapshot {
                    key: key.clone(),
                    request_hash: gate.request_hash,
                    service_generation: gate.service_generation,
                    stream_epoch: gate.stream_epoch,
                    semantic_sequence: gate.semantic_sequence,
                    source_capacity: gate.source_capacity,
                    source: source.clone(),
                    reply_route: attempt
                        .reply_route
                        .as_ref()
                        .map(ReliableFlushRouteIdentity::capture),
                    materialization_authorized: attempt.materialization_authorized,
                    authorized_materialization_route: attempt
                        .authorized_materialization_route
                        .as_ref()
                        .map(ReliableFlushRouteIdentity::capture),
                    materialization_retryable: attempt.materialization_retryable,
                    cursor: attempt.cursor,
                    pending_flush_chunk: attempt.pending_flush_chunk.clone(),
                    inserted: attempt.inserted,
                });
            }
        }

        let mut transfers = Vec::new();
        let mut outbound = Vec::new();
        for (key, transfer) in &transport.outbound {
            let has_sibling = transfer
                .attempts
                .keys()
                .any(|source| key != target_key || source != target_source);
            if has_sibling {
                transfers.push((
                    key.clone(),
                    ReliableFlushSharedTransferSnapshot::capture(transfer),
                ));
            }
            for (source, attempt) in &transfer.attempts {
                if key == target_key && source == target_source {
                    continue;
                }
                outbound.push(ReliableFlushSiblingOutboundSnapshot {
                    key: key.clone(),
                    source: source.clone(),
                    reply_route: attempt
                        .reply_route
                        .as_ref()
                        .map(ReliableFlushRouteIdentity::capture),
                    next_chunk: attempt.next_chunk,
                    in_flight_chunk: attempt.in_flight_chunk,
                    queued: attempt.queued,
                });
            }
        }

        let order = transport
            .outbound_order
            .iter()
            .filter(|(key, source)| key != target_key || source != target_source)
            .cloned()
            .collect();
        Self {
            gates,
            transfers,
            outbound,
            order,
        }
    }

    fn digest(&self) -> Hash {
        let mut bytes = ReliableFlushProjectionBytes::default();
        bytes.usize(self.gates.len());
        for gate in &self.gates {
            bytes.u8(1);
            bytes.key(&gate.key);
            bytes.typed_hash(gate.request_hash);
            bytes.u64(gate.service_generation.get());
            bytes.u64(gate.stream_epoch.get());
            bytes.u64(gate.semantic_sequence.get());
            if let Some(capacity) = gate.source_capacity {
                bytes.bool(true);
                bytes.usize(capacity);
            } else {
                bytes.bool(false);
            }
            bytes.source(&gate.source);
            if let Some(route) = &gate.reply_route {
                bytes.bool(true);
                bytes.hash(route.digest());
            } else {
                bytes.bool(false);
            }
            bytes.bool(gate.materialization_authorized);
            if let Some(route) = &gate.authorized_materialization_route {
                bytes.bool(true);
                bytes.hash(route.digest());
            } else {
                bytes.bool(false);
            }
            bytes.bool(gate.materialization_retryable);
            bytes.cursor(gate.cursor);
            bytes.pending_chunk(gate.pending_flush_chunk.as_ref());
            // `Instant` has no stable, exact byte representation. It remains
            // in the sibling record and is protected by `sibling_records_equal`;
            // it is intentionally absent from this fixed-width digest.
        }
        bytes.usize(self.transfers.len());
        for (key, transfer) in &self.transfers {
            bytes.u8(2);
            bytes.key(key);
            bytes.hash(transfer.digest());
        }
        bytes.usize(self.outbound.len());
        for outbound in &self.outbound {
            bytes.u8(3);
            bytes.key(&outbound.key);
            bytes.source(&outbound.source);
            if let Some(route) = &outbound.reply_route {
                bytes.bool(true);
                bytes.hash(route.digest());
            } else {
                bytes.bool(false);
            }
            bytes.usize(outbound.next_chunk);
            if let Some(in_flight) = outbound.in_flight_chunk {
                bytes.bool(true);
                bytes.usize(in_flight);
            } else {
                bytes.bool(false);
            }
            bytes.bool(outbound.queued);
        }
        bytes.usize(self.order.len());
        for (key, source) in &self.order {
            bytes.u8(4);
            bytes.key(key);
            bytes.source(source);
        }
        bytes.finish(RELIABLE_FLUSH_SIBLING_STATE_DIGEST_DOMAIN)
    }
}

fn reliable_flush_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn reliable_flush_hash_identity(domain: u8, kind: u8, hash: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}

fn reliable_flush_peer_identity(peer: &PeerId) -> CanonicalIdentityProjection {
    reliable_flush_typed_identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, HashOf::new(peer))
}

fn reliable_flush_ordinal_halves(ordinal: u128) -> (u64, u64) {
    ((ordinal >> 64) as u64, ordinal as u64)
}

fn reliable_flush_usize(value: usize) -> Result<u64, MergeSidecarError> {
    u64::try_from(value).map_err(|_| {
        MergeSidecarError::FlushIdentityMismatch(
            "sidecar flush application field is not representable as u64",
        )
    })
}

/// Map a network topic to the exact primitive tag consumed by the shared
/// production/Verus reliable-flush kernels.
pub(crate) const fn reliable_flush_topic_tag(topic: Topic) -> u8 {
    match topic {
        Topic::ConsensusSafety => 1,
        Topic::Consensus => 2,
        Topic::ConsensusChunk => 3,
        Topic::ConsensusPayload => 4,
        Topic::Control => 5,
        Topic::BlockSync => 6,
        Topic::TxGossip => 7,
        Topic::TxGossipRestricted => 8,
        Topic::PeerGossip => 9,
        Topic::TrustGossip => 10,
        Topic::Health => 11,
        Topic::Other => 12,
    }
}

#[derive(Debug)]
struct ReliableFlushGateApplicationPlan {
    key: ServerRequestKey,
    source: ServerRequestSource,
    cursor_before: usize,
    pending_marker: ServerPendingChunkIdentity,
    inserted_before: Instant,
    residual_before: ReliableFlushTargetGateResidual,
}

enum ReliableFlushGatePreflight {
    /// A missing, complete, or already-advanced gate is a harmless late receipt.
    ConsumeWithoutMutation,
    Ready(ReliableFlushGateApplicationPlan),
}

#[derive(Debug)]
struct ReliableFlushOutboundAttemptPlan {
    route_active: bool,
    cursor_before: usize,
    in_flight_before: Option<usize>,
    queued_before: bool,
    residual_before: ReliableFlushTargetOutboundResidual,
}

#[derive(Debug)]
struct ReliableFlushOutboundApplicationPlan {
    shared_transfer_before: Option<ReliableFlushSharedTransferSnapshot>,
    shared_transfer_other_attempts_before: bool,
    order_count_before: usize,
    order_rank_before: Option<usize>,
    sibling_order_len_before: usize,
    attempt: Option<ReliableFlushOutboundAttemptPlan>,
}

enum ReliableFlushOutboundPreflight {
    /// Retained outbound state did not belong to this exact source occurrence.
    RejectWithoutClaim,
    Ready(ReliableFlushOutboundApplicationPlan),
}

enum ReliableFlushOutboundAttemptPreflight {
    RejectWithoutClaim,
    Ready(Option<ReliableFlushOutboundAttemptPlan>),
}

#[derive(Debug)]
struct ReliableFlushApplicationPlan {
    gate: ReliableFlushGateApplicationPlan,
    outbound: ReliableFlushOutboundApplicationPlan,
    sibling_state_before: ReliableFlushSiblingStateSnapshot,
    occurrence: ProductionReliableFlushApplicationProjection,
    expected_cursor_after: usize,
    count: usize,
    gate_cursor_before: u64,
    outbound_cursor_before: u64,
    outbound_in_flight_before: u64,
    outbound_order_count_before: u64,
    outbound_order_rank_before: u64,
    sibling_order_len_before: u64,
}

fn reliable_flush_target_order_position(
    order: &VecDeque<OutboundAttemptKey>,
    target_key: &ServerRequestKey,
    target_source: &ServerRequestSource,
) -> (usize, Option<usize>, usize) {
    let mut target_count = 0usize;
    let mut target_rank = None;
    let mut sibling_len = 0usize;
    for (key, source) in order {
        if key == target_key && source == target_source {
            target_count = target_count
                .checked_add(1)
                .expect("bounded sidecar output order cannot overflow usize");
            target_rank.get_or_insert(sibling_len);
        } else {
            sibling_len = sibling_len
                .checked_add(1)
                .expect("bounded sidecar sibling order cannot overflow usize");
        }
    }
    (target_count, target_rank, sibling_len)
}

fn preflight_reliable_flush_gate(
    transport: &MergeSidecarTransport,
    admission: &CertifiedMergeSidecarChunkAdmission,
    chunk_index: usize,
) -> Result<ReliableFlushGatePreflight, MergeSidecarError> {
    let evidence = admission.projection();
    let key = (evidence.requester.clone(), evidence.request_id);
    let source = ServerRequestSource::Authenticated(admission.source_key.clone());
    let Some(gate) = transport.server_request_gates.get(&key) else {
        return Ok(ReliableFlushGatePreflight::ConsumeWithoutMutation);
    };
    if gate.service_generation != evidence.service_generation
        || gate.stream_epoch != evidence.stream_epoch
        || gate.semantic_sequence != evidence.semantic_sequence
    {
        return Ok(ReliableFlushGatePreflight::ConsumeWithoutMutation);
    }
    let Some(attempt) = gate.attempts.get(&source) else {
        return Ok(ReliableFlushGatePreflight::ConsumeWithoutMutation);
    };
    let ServerResponseCursor::Pending(cursor_before) = attempt.cursor else {
        return Ok(ReliableFlushGatePreflight::ConsumeWithoutMutation);
    };
    if cursor_before != chunk_index {
        if chunk_index < cursor_before {
            return Ok(ReliableFlushGatePreflight::ConsumeWithoutMutation);
        }
        return Err(MergeSidecarError::FlushIdentityMismatch(
            "acknowledgement skipped the retained source cursor",
        ));
    }
    let Some(pending_marker) = attempt.pending_flush_chunk.as_ref() else {
        return Err(MergeSidecarError::FlushIdentityMismatch(
            "retained source cursor has no byte-free chunk identity",
        ));
    };
    if !pending_marker.matches_admission(admission) {
        return Err(MergeSidecarError::FlushIdentityMismatch(
            "acknowledgement differs from the retained byte-free chunk identity",
        ));
    }
    Ok(ReliableFlushGatePreflight::Ready(
        ReliableFlushGateApplicationPlan {
            key: key.clone(),
            source: source.clone(),
            cursor_before,
            pending_marker: pending_marker.clone(),
            inserted_before: attempt.inserted,
            residual_before: ReliableFlushTargetGateResidual::capture(&key, &source, gate, attempt),
        },
    ))
}

fn preflight_reliable_flush_outbound(
    transport: &MergeSidecarTransport,
    admission: &CertifiedMergeSidecarChunkAdmission,
    gate: &ReliableFlushGateApplicationPlan,
    chunk_index: usize,
    count: usize,
) -> Result<ReliableFlushOutboundPreflight, MergeSidecarError> {
    let (order_count_before, order_rank_before, sibling_order_len_before) =
        reliable_flush_target_order_position(&transport.outbound_order, &gate.key, &gate.source);
    let Some(transfer) = transport.outbound.get(&gate.key) else {
        if order_count_before != 0 {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "missing outbound attempt retained a source-order reservation",
            ));
        }
        return Ok(ReliableFlushOutboundPreflight::Ready(
            ReliableFlushOutboundApplicationPlan {
                shared_transfer_before: None,
                shared_transfer_other_attempts_before: false,
                order_count_before,
                order_rank_before,
                sibling_order_len_before,
                attempt: None,
            },
        ));
    };
    let evidence = admission.projection();
    let request = &transfer.request;
    if request.request_id != evidence.request_id
        || request.service_generation != evidence.service_generation
        || request.stream_epoch != evidence.stream_epoch
        || request.semantic_sequence != evidence.semantic_sequence
        || request.entry_hash != evidence.entry_hash
        || request.encoded_len != evidence.encoded_len
        || request.epoch_id != evidence.epoch_id
        || request.reference_digest != evidence.reference_digest
        || request.requester != evidence.requester
        || request.responder != evidence.responder
        || usize::try_from(request.encoded_len).ok() != Some(transfer.response_len)
        || transfer.chunks.len() != count
    {
        return Err(MergeSidecarError::FlushIdentityMismatch(
            "cached response request changed before acknowledgement",
        ));
    }
    let expected_message =
        transfer
            .chunks
            .get(chunk_index)
            .ok_or(MergeSidecarError::FlushIdentityMismatch(
                "chunk cursor does not name a cached response chunk",
            ))?;
    if !admission.matches_materialized_chunk(expected_message) {
        return Err(MergeSidecarError::FlushIdentityMismatch(
            "materialized response differs from the actor-admitted chunk",
        ));
    }

    let shared_transfer_other_attempts_before = transfer
        .attempts
        .keys()
        .any(|candidate| candidate != &gate.source);
    let attempt = match preflight_reliable_flush_outbound_attempt(
        transfer,
        admission,
        gate,
        chunk_index,
        order_count_before,
    )? {
        ReliableFlushOutboundAttemptPreflight::RejectWithoutClaim => {
            return Ok(ReliableFlushOutboundPreflight::RejectWithoutClaim);
        }
        ReliableFlushOutboundAttemptPreflight::Ready(attempt) => attempt,
    };
    Ok(ReliableFlushOutboundPreflight::Ready(
        ReliableFlushOutboundApplicationPlan {
            shared_transfer_before: Some(ReliableFlushSharedTransferSnapshot::capture(transfer)),
            shared_transfer_other_attempts_before,
            order_count_before,
            order_rank_before,
            sibling_order_len_before,
            attempt,
        },
    ))
}

fn preflight_reliable_flush_outbound_attempt(
    transfer: &OutboundTransfer,
    admission: &CertifiedMergeSidecarChunkAdmission,
    gate: &ReliableFlushGateApplicationPlan,
    chunk_index: usize,
    order_count_before: usize,
) -> Result<ReliableFlushOutboundAttemptPreflight, MergeSidecarError> {
    let attempt = if let Some(attempt) = transfer.attempts.get(&gate.source) {
        let Some(route) = attempt.reply_route.as_ref() else {
            return Ok(ReliableFlushOutboundAttemptPreflight::RejectWithoutClaim);
        };
        if !admission.is_bound_to_source(route) {
            return Ok(ReliableFlushOutboundAttemptPreflight::RejectWithoutClaim);
        }
        if attempt.next_chunk != chunk_index {
            if chunk_index < attempt.next_chunk {
                return Ok(ReliableFlushOutboundAttemptPreflight::RejectWithoutClaim);
            }
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "acknowledgement skipped the materialized source cursor",
            ));
        }
        if attempt
            .in_flight_chunk
            .is_some_and(|in_flight| in_flight != chunk_index)
        {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "acknowledgement does not name the in-flight source chunk",
            ));
        }
        if order_count_before > 1 || attempt.queued != (order_count_before == 1) {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "source queue marker differs from its exact output-order reservation",
            ));
        }
        Some(ReliableFlushOutboundAttemptPlan {
            route_active: route.is_active(),
            cursor_before: attempt.next_chunk,
            in_flight_before: attempt.in_flight_chunk,
            queued_before: attempt.queued,
            residual_before: ReliableFlushTargetOutboundResidual::capture(
                &gate.key,
                &gate.source,
                attempt,
            ),
        })
    } else {
        if order_count_before != 0 {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "missing source attempt retained an output-order reservation",
            ));
        }
        None
    };
    Ok(ReliableFlushOutboundAttemptPreflight::Ready(attempt))
}

fn finish_reliable_flush_application_plan(
    transport: &MergeSidecarTransport,
    gate: ReliableFlushGateApplicationPlan,
    outbound: ReliableFlushOutboundApplicationPlan,
    occurrence: ProductionReliableFlushApplicationProjection,
    expected_cursor_after: usize,
    count: usize,
) -> Result<ReliableFlushApplicationPlan, MergeSidecarError> {
    let outbound_cursor_before = outbound
        .attempt
        .as_ref()
        .map_or(0, |attempt| attempt.cursor_before);
    let outbound_in_flight_before = outbound
        .attempt
        .as_ref()
        .and_then(|attempt| attempt.in_flight_before)
        .unwrap_or_default();
    Ok(ReliableFlushApplicationPlan {
        sibling_state_before: ReliableFlushSiblingStateSnapshot::capture(
            transport,
            &gate.key,
            &gate.source,
        ),
        occurrence,
        gate_cursor_before: reliable_flush_usize(gate.cursor_before)?,
        outbound_cursor_before: reliable_flush_usize(outbound_cursor_before)?,
        outbound_in_flight_before: reliable_flush_usize(outbound_in_flight_before)?,
        outbound_order_count_before: reliable_flush_usize(outbound.order_count_before)?,
        outbound_order_rank_before: reliable_flush_usize(
            outbound.order_rank_before.unwrap_or_default(),
        )?,
        sibling_order_len_before: reliable_flush_usize(outbound.sibling_order_len_before)?,
        gate,
        outbound,
        expected_cursor_after,
        count,
    })
}

fn apply_reliable_flush_application(
    transport: &mut MergeSidecarTransport,
    plan: &ReliableFlushApplicationPlan,
    now: Instant,
) {
    if plan.outbound.attempt.is_some() {
        let attempt = transport
            .outbound
            .get_mut(&plan.gate.key)
            .and_then(|transfer| transfer.attempts.get_mut(&plan.gate.source))
            .expect("prevalidated outbound source attempt remains present");
        attempt.next_chunk = plan.expected_cursor_after;
        attempt.in_flight_chunk = None;
    }

    let completed = plan.expected_cursor_after == plan.count;
    let cursor = if completed {
        ServerResponseCursor::Complete
    } else {
        ServerResponseCursor::Pending(plan.expected_cursor_after)
    };
    let active_attempt = plan
        .outbound
        .attempt
        .as_ref()
        .map(|attempt| attempt.route_active);
    let gate_attempt = transport
        .server_request_gates
        .get_mut(&plan.gate.key)
        .and_then(|gate| gate.attempts.get_mut(&plan.gate.source))
        .expect("prevalidated pending server gate remains present");
    gate_attempt.cursor = cursor;
    gate_attempt.pending_flush_chunk = None;
    if completed || active_attempt.is_none_or(|active| !active) {
        gate_attempt.inserted = now;
    }

    if active_attempt.is_some_and(|active| completed || !active) {
        let transfer = transport
            .outbound
            .get_mut(&plan.gate.key)
            .expect("prevalidated outbound transfer remains present");
        transfer.attempts.remove(&plan.gate.source);
        if transfer.attempts.is_empty() {
            transport.outbound.remove(&plan.gate.key);
        }
        transport
            .outbound_order
            .retain(|(key, source)| key != &plan.gate.key || source != &plan.gate.source);
    } else if active_attempt == Some(true) {
        let attempt = transport
            .outbound
            .get_mut(&plan.gate.key)
            .expect("prevalidated outbound transfer remains present")
            .attempts
            .get_mut(&plan.gate.source)
            .expect("prevalidated outbound source attempt remains present");
        if !attempt.queued {
            attempt.queued = true;
            transport
                .outbound_order
                .push_back((plan.gate.key.clone(), plan.gate.source.clone()));
        }
    }
}

#[derive(Debug)]
struct ReliableFlushApplicationObservation {
    gate_marker_present_after: bool,
    gate_cursor_after: u64,
    gate_complete_after: bool,
    inserted_after: Instant,
    target_gate_residual_after: ReliableFlushTargetGateResidual,
    outbound_cursor_after: u64,
    outbound_attempt_after: Option<ReliableFlushTargetOutboundResidual>,
    outbound_in_flight_after_present: bool,
    outbound_queued_after: bool,
    outbound_order_count_after: u64,
    outbound_order_rank_after: u64,
    sibling_order_len_after: u64,
    shared_transfer_after: Option<ReliableFlushSharedTransferSnapshot>,
    sibling_state_after: ReliableFlushSiblingStateSnapshot,
}

fn predict_reliable_flush_application(
    plan: &ReliableFlushApplicationPlan,
    now: Instant,
) -> ReliableFlushApplicationObservation {
    let completed = plan.expected_cursor_after == plan.count;
    let active_attempt = plan
        .outbound
        .attempt
        .as_ref()
        .map(|attempt| attempt.route_active);
    let retains_target_attempt = active_attempt == Some(true) && !completed;
    let retains_shared_transfer =
        plan.outbound.shared_transfer_other_attempts_before || retains_target_attempt;
    ReliableFlushApplicationObservation {
        gate_marker_present_after: false,
        gate_cursor_after: u64::try_from(plan.expected_cursor_after)
            .expect("preflighted sidecar gate cursor remains representable"),
        gate_complete_after: completed,
        inserted_after: if retains_target_attempt {
            plan.gate.inserted_before
        } else {
            now
        },
        target_gate_residual_after: plan.gate.residual_before.clone(),
        outbound_cursor_after: u64::try_from(plan.expected_cursor_after)
            .expect("preflighted sidecar outbound cursor remains representable"),
        outbound_attempt_after: retains_target_attempt.then(|| {
            plan.outbound
                .attempt
                .as_ref()
                .expect("retained target attempt was preflighted present")
                .residual_before
                .clone()
        }),
        outbound_in_flight_after_present: false,
        outbound_queued_after: retains_target_attempt,
        outbound_order_count_after: u64::from(retains_target_attempt),
        outbound_order_rank_after: if retains_target_attempt {
            plan.sibling_order_len_before
        } else {
            0
        },
        sibling_order_len_after: plan.sibling_order_len_before,
        shared_transfer_after: retains_shared_transfer
            .then(|| plan.outbound.shared_transfer_before.clone())
            .flatten(),
        sibling_state_after: plan.sibling_state_before.clone(),
    }
}

fn observe_reliable_flush_application(
    transport: &MergeSidecarTransport,
    plan: &ReliableFlushApplicationPlan,
) -> ReliableFlushApplicationObservation {
    let gate = transport
        .server_request_gates
        .get(&plan.gate.key)
        .expect("acknowledged server gate remains present");
    let gate_attempt = gate
        .attempts
        .get(&plan.gate.source)
        .expect("acknowledged server gate source remains present");
    let (gate_complete_after, gate_cursor_after) = match gate_attempt.cursor {
        ServerResponseCursor::Pending(cursor) => (false, cursor),
        ServerResponseCursor::Complete => (true, plan.count),
    };
    let outbound_attempt = transport
        .outbound
        .get(&plan.gate.key)
        .and_then(|transfer| transfer.attempts.get(&plan.gate.source));
    let (order_count_after, order_rank_after, sibling_order_len_after) =
        reliable_flush_target_order_position(
            &transport.outbound_order,
            &plan.gate.key,
            &plan.gate.source,
        );
    ReliableFlushApplicationObservation {
        gate_marker_present_after: gate_attempt.pending_flush_chunk.is_some(),
        gate_cursor_after: u64::try_from(gate_cursor_after)
            .expect("prevalidated sidecar gate cursor remains representable"),
        gate_complete_after,
        inserted_after: gate_attempt.inserted,
        target_gate_residual_after: ReliableFlushTargetGateResidual::capture(
            &plan.gate.key,
            &plan.gate.source,
            gate,
            gate_attempt,
        ),
        outbound_cursor_after: u64::try_from(
            outbound_attempt.map_or(plan.expected_cursor_after, |attempt| attempt.next_chunk),
        )
        .expect("prevalidated sidecar outbound cursor remains representable"),
        outbound_attempt_after: outbound_attempt.map(|attempt| {
            ReliableFlushTargetOutboundResidual::capture(&plan.gate.key, &plan.gate.source, attempt)
        }),
        outbound_in_flight_after_present: outbound_attempt
            .is_some_and(|attempt| attempt.in_flight_chunk.is_some()),
        outbound_queued_after: outbound_attempt.is_some_and(|attempt| attempt.queued),
        outbound_order_count_after: u64::try_from(order_count_after)
            .expect("bounded sidecar output-order multiplicity fits u64"),
        outbound_order_rank_after: u64::try_from(order_rank_after.unwrap_or_default())
            .expect("bounded sidecar output-order rank fits u64"),
        sibling_order_len_after: u64::try_from(sibling_order_len_after)
            .expect("bounded sidecar sibling output order fits u64"),
        shared_transfer_after: transport
            .outbound
            .get(&plan.gate.key)
            .map(ReliableFlushSharedTransferSnapshot::capture),
        sibling_state_after: ReliableFlushSiblingStateSnapshot::capture(
            transport,
            &plan.gate.key,
            &plan.gate.source,
        ),
    }
}

fn reliable_flush_application_occurrence_projection(
    admission: &CertifiedMergeSidecarChunkAdmission,
) -> Result<ProductionReliableFlushApplicationProjection, MergeSidecarError> {
    let evidence = admission.projection();
    let (connection_high, connection_low) =
        reliable_flush_ordinal_halves(evidence.connection_tenure_ordinal);
    let (delivery_high, delivery_low) = reliable_flush_ordinal_halves(evidence.delivery_ordinal);
    let mut application = ProductionReliableFlushApplicationProjection::default();
    application.semantic_target = reliable_flush_peer_identity(&evidence.semantic_target);
    application.authenticated_source = reliable_flush_peer_identity(&evidence.authenticated_source);
    application.source_key_identity =
        process_local_projection(IDENTITY_KIND_REPLY_SOURCE_KEY, evidence.source_key_identity);
    application.delivery_route_identity = process_local_projection(
        IDENTITY_KIND_REPLY_DELIVERY_ROUTE,
        evidence.delivery_route_identity,
    );
    application.writer_occurrence_identity = process_local_projection(
        IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
        evidence.writer_occurrence_identity,
    );
    application.requester = reliable_flush_peer_identity(&evidence.requester);
    application.responder = reliable_flush_peer_identity(&evidence.responder);
    application.connection_tenure_ordinal_high = connection_high;
    application.connection_tenure_ordinal_low = connection_low;
    application.delivery_ordinal_high = delivery_high;
    application.delivery_ordinal_low = delivery_low;
    application.ticket_id = evidence.ticket_id;
    application.ticket_rank = reliable_flush_usize(evidence.ticket_rank)?;
    application.ticket_topic = reliable_flush_topic_tag(evidence.ticket_topic);
    application.reply_writer_timeout_attempt = evidence.reply_writer_timeout_attempt;
    application.canonical_request_digest = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_REPLY_PAYLOAD,
        evidence.canonical_request_digest,
    );
    application.stream_wire_bytes = reliable_flush_usize(evidence.stream_wire_bytes)?;
    application.request_id = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_REQUEST,
        evidence.request_id,
    );
    application.service_generation = evidence.service_generation.get();
    application.stream_epoch = evidence.stream_epoch.get();
    application.semantic_sequence = evidence.semantic_sequence.get();
    application.entry_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_MERGE_ENTRY,
        evidence.entry_hash,
    );
    application.encoded_len = evidence.encoded_len;
    application.epoch_id = evidence.epoch_id;
    application.reference_digest = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_REFERENCE_DIGEST,
        evidence.reference_digest,
    );
    application.canonical_response_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_NETWORK_RESPONSE,
        evidence.canonical_response_hash,
    );
    application.sidecar_response_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_RESPONSE,
        evidence.sidecar_response_hash,
    );
    application.chunk_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_CHUNK,
        evidence.chunk_hash,
    );
    application.payload_digest = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_PAYLOAD,
        evidence.payload_digest,
    );
    application.chunk_index = u64::from(evidence.chunk_index);
    application.chunk_count = u64::from(evidence.chunk_count);
    application.message_cursor_before = reliable_flush_usize(evidence.message_cursor_before)?;
    application.message_cursor_after = reliable_flush_usize(evidence.message_cursor_after)?;
    application.chunk_cursor_before = reliable_flush_usize(evidence.chunk_cursor_before)?;
    application.chunk_cursor_after = reliable_flush_usize(evidence.chunk_cursor_after)?;
    // The two-phase link checks the worker completion against the exact
    // byte-free marker expected for this admitted occurrence before lane
    // state is inspected. The application path independently overwrites
    // these fields from the retained gate marker after preflight, so this
    // expectation cannot substitute for the production marker observation.
    application.marker_request_id = application.request_id;
    application.marker_service_generation = application.service_generation;
    application.marker_stream_epoch = application.stream_epoch;
    application.marker_semantic_sequence = application.semantic_sequence;
    application.marker_entry_hash = application.entry_hash;
    application.marker_encoded_len = application.encoded_len;
    application.marker_epoch_id = application.epoch_id;
    application.marker_reference_digest = application.reference_digest;
    application.marker_requester = application.requester;
    application.marker_responder = application.responder;
    application.marker_canonical_response_hash = application.canonical_response_hash;
    application.marker_sidecar_response_hash = application.sidecar_response_hash;
    application.marker_chunk_hash = application.chunk_hash;
    application.marker_payload_digest = application.payload_digest;
    application.marker_chunk_index = application.chunk_index;
    application.marker_chunk_count = application.chunk_count;
    application.marker_topic = application.ticket_topic;
    Ok(application)
}

fn project_reliable_flush_marker(
    application: &mut ProductionReliableFlushApplicationProjection,
    marker: &ServerPendingChunkIdentity,
) {
    application.marker_request_id = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_REQUEST,
        marker.request_id,
    );
    application.marker_service_generation = marker.service_generation.get();
    application.marker_stream_epoch = marker.stream_epoch.get();
    application.marker_semantic_sequence = marker.semantic_sequence.get();
    application.marker_entry_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_MERGE_ENTRY,
        marker.entry_hash,
    );
    application.marker_encoded_len = marker.encoded_len;
    application.marker_epoch_id = marker.epoch_id;
    application.marker_reference_digest = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_REFERENCE_DIGEST,
        marker.reference_digest,
    );
    application.marker_requester = reliable_flush_peer_identity(&marker.requester);
    application.marker_responder = reliable_flush_peer_identity(&marker.responder);
    application.marker_canonical_response_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_NETWORK_RESPONSE,
        marker.canonical_response_hash,
    );
    application.marker_sidecar_response_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_RESPONSE,
        marker.sidecar_response_hash,
    );
    application.marker_chunk_hash = reliable_flush_typed_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_CHUNK,
        marker.chunk_hash,
    );
    application.marker_payload_digest = reliable_flush_hash_identity(
        IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_SIDECAR_PAYLOAD,
        marker.payload_digest,
    );
    application.marker_chunk_index = u64::from(marker.chunk_index);
    application.marker_chunk_count = u64::from(marker.chunk_count);
    application.marker_topic = reliable_flush_topic_tag(marker.topic);
}

fn project_reliable_flush_transition(
    application: &mut ProductionReliableFlushApplicationProjection,
    plan: &ReliableFlushApplicationPlan,
    observation: &ReliableFlushApplicationObservation,
    now: Instant,
) {
    let outbound_before = plan.outbound.attempt.as_ref();
    application.claim_acquired = true;
    application.gate_marker_present_before = true;
    application.gate_marker_present_after = observation.gate_marker_present_after;
    application.gate_cursor_before = plan.gate_cursor_before;
    application.gate_cursor_after = observation.gate_cursor_after;
    application.gate_complete_after = observation.gate_complete_after;
    application.gate_attempt_present_after = true;
    application.outbound_attempt_present_before = outbound_before.is_some();
    application.outbound_route_bound_before = outbound_before.is_some();
    application.outbound_route_active_before =
        outbound_before.is_some_and(|attempt| attempt.route_active);
    application.outbound_cursor_before = plan.outbound_cursor_before;
    application.outbound_cursor_after = observation.outbound_cursor_after;
    application.outbound_in_flight_before_present =
        outbound_before.is_some_and(|attempt| attempt.in_flight_before.is_some());
    application.outbound_in_flight_before = plan.outbound_in_flight_before;
    application.outbound_queued_before =
        outbound_before.is_some_and(|attempt| attempt.queued_before);
    application.outbound_order_count_before = plan.outbound_order_count_before;
    application.outbound_order_rank_before = plan.outbound_order_rank_before;
    application.sibling_order_len_before = plan.sibling_order_len_before;
    application.outbound_attempt_present_after = observation.outbound_attempt_after.is_some();
    application.outbound_in_flight_after_present = observation.outbound_in_flight_after_present;
    application.outbound_queued_after = observation.outbound_queued_after;
    application.outbound_order_count_after = observation.outbound_order_count_after;
    application.outbound_order_rank_after = observation.outbound_order_rank_after;
    application.sibling_order_len_after = observation.sibling_order_len_after;
    application.inserted_preserved = observation.inserted_after == plan.gate.inserted_before;
    application.inserted_equals_now = observation.inserted_after == now;
}

fn process_local_projection(kind: u8, digest: Hash) -> CanonicalIdentityProjection {
    reliable_flush_hash_identity(IDENTITY_DOMAIN_PROCESS_LOCAL, kind, digest)
}

fn project_reliable_flush_residuals(
    application: &mut ProductionReliableFlushApplicationProjection,
    plan: &ReliableFlushApplicationPlan,
    observation: &ReliableFlushApplicationObservation,
) {
    application.target_gate_residual_records_equal =
        plan.gate.residual_before == observation.target_gate_residual_after;
    application.target_gate_residual_before = process_local_projection(
        IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
        plan.gate.residual_before.digest(),
    );
    application.target_gate_residual_after = process_local_projection(
        IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE,
        observation.target_gate_residual_after.digest(),
    );

    let target_outbound_before = plan
        .outbound
        .attempt
        .as_ref()
        .map(|attempt| &attempt.residual_before);
    application.target_outbound_residual_records_equal = target_outbound_before
        .zip(observation.outbound_attempt_after.as_ref())
        .is_some_and(|(before, after)| before == after);
    application.target_outbound_residual_before =
        target_outbound_before.map_or_else(CanonicalIdentityProjection::zero, |before| {
            process_local_projection(IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE, before.digest())
        });
    application.target_outbound_residual_after = observation
        .outbound_attempt_after
        .as_ref()
        .map_or_else(CanonicalIdentityProjection::zero, |after| {
            process_local_projection(IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE, after.digest())
        });

    let shared_before = plan.outbound.shared_transfer_before.as_ref();
    let shared_after = observation.shared_transfer_after.as_ref();
    application.shared_transfer_present_before = shared_before.is_some();
    application.shared_transfer_present_after = shared_after.is_some();
    application.shared_transfer_other_attempts_before =
        plan.outbound.shared_transfer_other_attempts_before;
    application.shared_transfer_records_equal = shared_before
        .zip(shared_after)
        .is_some_and(|(before, after)| before == after);
    application.shared_transfer_state_before =
        shared_before.map_or_else(CanonicalIdentityProjection::zero, |before| {
            process_local_projection(IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE, before.digest())
        });
    application.shared_transfer_state_after =
        shared_after.map_or_else(CanonicalIdentityProjection::zero, |after| {
            process_local_projection(IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE, after.digest())
        });

    application.sibling_records_equal =
        plan.sibling_state_before == observation.sibling_state_after;
    application.sibling_state_before = process_local_projection(
        IDENTITY_KIND_SIDECAR_SIBLING_STATE,
        plan.sibling_state_before.digest(),
    );
    application.sibling_state_after = process_local_projection(
        IDENTITY_KIND_SIDECAR_SIBLING_STATE,
        observation.sibling_state_after.digest(),
    );
}

fn reliable_flush_application_projection(
    plan: &ReliableFlushApplicationPlan,
    observation: &ReliableFlushApplicationObservation,
    now: Instant,
) -> ProductionReliableFlushApplicationProjection {
    let mut application = plan.occurrence;
    project_reliable_flush_marker(&mut application, &plan.gate.pending_marker);
    project_reliable_flush_transition(&mut application, plan, observation, now);
    project_reliable_flush_residuals(&mut application, plan, observation);
    application
}

/// Network action emitted by the bounded transfer manager.
#[derive(Clone, Debug)]
pub(crate) struct MergeSidecarPost {
    /// Authenticated destination peer.
    pub(crate) peer: PeerId,
    /// Exact authenticated return route for request-induced responses.
    pub(crate) reply_route: Option<NetworkReplyRoute>,
    /// Request or chunk to send.
    pub(crate) message: Arc<CertifiedMergeSidecarMessage>,
}

impl PartialEq for MergeSidecarPost {
    fn eq(&self, other: &Self) -> bool {
        let same_reply_authority = match (&self.reply_route, &other.reply_route) {
            (None, None) => true,
            (Some(left), Some(right)) => {
                left.semantic_target() == right.semantic_target() && left.same_delivery(right)
            }
            (None, Some(_)) | (Some(_), None) => false,
        };
        self.peer == other.peer && same_reply_authority && self.message == other.message
    }
}

impl Eq for MergeSidecarPost {}

/// Result of authenticating one server-side request occurrence.
#[derive(Debug)]
pub(crate) enum ServerRequestAdmission {
    /// The caller owns the terminating Kura lookup for this occurrence.
    Materialize,
    /// Existing bounded work or output already owns this occurrence.
    Existing,
    /// The request named a compacted responder generation.
    GenerationHint(MergeSidecarPost),
}

/// One fair-scheduler-selected server lookup.
///
/// The transport has already durably advanced its requester round-robin cursor
/// and bound terminating materialization authority to `reply_route`. The
/// caller must either enqueue the exact response, release the authorization
/// after transient capacity pressure, or durably retire a terminal failure.
#[derive(Clone, Debug)]
pub(crate) struct ServerRequestMaterialization {
    /// Semantic requester which owns the logical occurrence.
    pub(crate) requester: PeerId,
    /// Exact canonical request selected within that requester's stream.
    pub(crate) request: CertifiedMergeSidecarRequestV1,
    /// Exact active route whose attempt authorizes the lookup.
    pub(crate) reply_route: Option<NetworkReplyRoute>,
}

/// Fully reassembled response awaiting canonical/QC validation and persistence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletedMergeSidecar {
    /// Exact compact reference from the deferred block.
    pub(crate) reference: CertifiedMergeLedgerReference,
    /// Reassembled canonical full-entry bytes.
    pub(crate) bytes: Vec<u8>,
}

/// Outcome of accepting one response chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChunkIngestOutcome {
    /// Chunk was accepted but the entry is incomplete.
    Accepted,
    /// Every exact fixed-boundary chunk is present.
    Complete(CompletedMergeSidecar),
}

/// In-memory bounded transport state. Incomplete bytes are never durable.
#[derive(Debug)]
pub(crate) struct MergeSidecarTransport {
    limits: MergeSidecarLimits,
    reply_source_capacity: usize,
    /// Canonical identity of the admitted responder roster.
    server_roster_digest: MergeSidecarRosterDigest,
    /// Admitted semantic requesters in the current responder generation.
    server_stream_capacity: usize,
    outbound_session_capacity: usize,
    outbound_byte_capacity: usize,
    /// Unique logical request gates, independently of delivery attempts.
    server_request_gate_capacity: usize,
    /// Authenticated delivery attempts retained across all logical gates.
    server_request_attempt_capacity: usize,
    inbound: BTreeMap<InboundSidecarKey, InboundAssembly>,
    inbound_cursor: Option<InboundSidecarKey>,
    outbound: BTreeMap<ServerRequestKey, OutboundTransfer>,
    /// Exact source-attempt ownership order. Every serviced incomplete attempt
    /// moves to the tail and every new source starts behind all current owners.
    outbound_order: VecDeque<OutboundAttemptKey>,
    tick_response_next: bool,
    tick_close_next: bool,
    /// True after one timeout retry was allowed to run before a due Close.
    timeout_retry_close_deferred: bool,
    server_request_gates: BTreeMap<ServerRequestKey, ServerRequestGate>,
    /// Last requester-issued stream epoch. Zero means no epoch was issued yet.
    next_stream_epoch: u64,
    request_streams: BTreeMap<PeerId, RequestStreamState>,
    /// Durable responder-owned fence for every retained server stream.
    server_service_generation: CertifiedMergeSidecarServiceGenerationV1,
    server_streams: BTreeMap<PeerId, ServerStreamState>,
    /// Durable first-level round-robin cursor for response materialization.
    materialization_requester_cursor: Option<PeerId>,
    pending_server_closures: BTreeMap<PeerId, CertifiedMergeSidecarClosedPrefix>,
    /// A drained close prefix still awaiting application by the exact-output
    /// owner.
    ///
    /// Transport gates and bytes may already be gone, but a lane or worker can
    /// still retain the covered chunk or its writer-flush receipt. Responder
    /// generation rollover therefore remains blocked until the runner confirms
    /// that every drained prefix reached that downstream owner. This flag is
    /// deliberately process-local: after a crash none of those queues survives.
    server_closure_handoff_pending: bool,
    lifecycle_journal: Option<MergeSidecarLifecycleJournal>,
    #[cfg(test)]
    obstruct_next_terminal_retirement_persist: bool,
}

struct ServerServiceGenerationTransitionPlan {
    server_stream_capacity: usize,
    server_roster_digest: MergeSidecarRosterDigest,
    server_request_gate_capacity: usize,
    server_request_attempt_capacity: usize,
    next_geometry: MergeSidecarLifecycleGeometryV3,
    next_generation: CertifiedMergeSidecarServiceGenerationV1,
}

#[derive(Clone, Copy)]
enum ServerServiceGenerationRetirement {
    /// Every retained occurrence was cumulatively closed by its requester.
    AuthenticatedTerminal,
    /// A certified roster successor superseded the responder tables after all
    /// process-local exact-output ownership became unreachable.
    ExactOutputSuperseded,
}

/// Private evidence that durable lifecycle restoration invalidated every
/// predecessor process-local writer and output queue.
struct RestoredLifecycleResponderFence;

impl MergeSidecarTransport {
    /// Construct an empty transport with the dependent-test source geometry.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_reply_source_capacity(DEFAULT_REPLY_SOURCE_CAPACITY)
            .expect("default sidecar reply-source geometry is representable")
    }

    /// Construct an empty transport whose global corridors reserve every
    /// configured authenticated source's independent per-source limits.
    #[cfg(test)]
    pub(crate) fn with_reply_source_capacity(
        reply_source_capacity: usize,
    ) -> Result<Self, MergeSidecarError> {
        Self::with_limits(reply_source_capacity, MergeSidecarLimits::defaults())
    }

    /// Construct an empty transport from the exact fingerprinted geometry.
    #[cfg(test)]
    pub(crate) fn with_limits(
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
    ) -> Result<Self, MergeSidecarError> {
        Self::with_limits_and_server_stream_capacity(
            reply_source_capacity,
            limits,
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,
            unbound_test_merge_sidecar_roster_digest(),
        )
    }

    /// Construct an empty transport with an explicit immutable roster identity.
    ///
    /// The caller must supply the admitted height roster's unique size and the
    /// digest returned by [`canonical_merge_sidecar_roster_digest`], never the
    /// number or identity of currently connected peers.
    pub(crate) fn with_limits_and_server_stream_capacity(
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<Self, MergeSidecarError> {
        if reply_source_capacity == 0 {
            return Err(MergeSidecarError::Capacity(
                "reply-source geometry must be non-zero",
            ));
        }
        let outbound_session_capacity = reply_source_capacity
            .checked_mul(limits.outbound_sessions_per_source)
            .ok_or(MergeSidecarError::Capacity(
                "outbound response session geometry",
            ))?;
        let outbound_byte_capacity = reply_source_capacity
            .checked_mul(limits.outbound_bytes_per_source)
            .ok_or(MergeSidecarError::Capacity(
                "outbound response byte geometry",
            ))?;
        let (server_request_gate_capacity, server_request_attempt_capacity) =
            Self::derive_server_request_capacities(
                reply_source_capacity,
                limits,
                server_stream_capacity,
            )?;
        Ok(Self {
            limits,
            reply_source_capacity,
            server_roster_digest,
            server_stream_capacity,
            outbound_session_capacity,
            outbound_byte_capacity,
            server_request_gate_capacity,
            server_request_attempt_capacity,
            inbound: BTreeMap::new(),
            inbound_cursor: None,
            outbound: BTreeMap::new(),
            outbound_order: VecDeque::new(),
            tick_response_next: true,
            // When request and close work first become simultaneously ready,
            // service the progress-bearing request before alternating to the
            // close stream. A standalone close remains immediately eligible.
            tick_close_next: false,
            timeout_retry_close_deferred: false,
            server_request_gates: BTreeMap::new(),
            next_stream_epoch: 0,
            request_streams: BTreeMap::new(),
            server_service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            server_streams: BTreeMap::new(),
            materialization_requester_cursor: None,
            pending_server_closures: BTreeMap::new(),
            server_closure_handoff_pending: false,
            lifecycle_journal: None,
            #[cfg(test)]
            obstruct_next_terminal_retirement_persist: false,
        })
    }

    fn derive_server_request_capacities(
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
    ) -> Result<(usize, usize), MergeSidecarError> {
        if server_stream_capacity == 0
            || server_stream_capacity > MAX_CERTIFIED_MERGE_SERVER_STREAMS
        {
            return Err(MergeSidecarError::Capacity(
                "server semantic requester geometry",
            ));
        }
        let gates = server_stream_capacity
            .checked_mul(limits.inbound_sessions_per_peer)
            .ok_or(MergeSidecarError::Capacity("server request gate geometry"))?;
        let attempts =
            gates
                .checked_mul(reply_source_capacity)
                .ok_or(MergeSidecarError::Capacity(
                    "server request attempt geometry",
                ))?;
        Ok((gates, attempts))
    }

    fn configure_server_roster_geometry(
        &mut self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<(), MergeSidecarError> {
        let (server_request_gate_capacity, server_request_attempt_capacity) =
            Self::derive_server_request_capacities(
                self.reply_source_capacity,
                self.limits,
                server_stream_capacity,
            )?;
        self.server_roster_digest = server_roster_digest;
        self.server_stream_capacity = server_stream_capacity;
        self.server_request_gate_capacity = server_request_gate_capacity;
        self.server_request_attempt_capacity = server_request_attempt_capacity;
        Ok(())
    }

    fn lifecycle_runtime_geometry_v3(
        &self,
    ) -> Result<MergeSidecarRuntimeGeometryV3, MergeSidecarError> {
        let as_u64 = |value: usize| {
            u64::try_from(value)
                .map_err(|_| MergeSidecarError::Capacity("lifecycle journal geometry"))
        };
        Ok(MergeSidecarRuntimeGeometryV3 {
            reply_source_capacity: as_u64(self.reply_source_capacity)?,
            semantic_peer_capacity: as_u64(MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)?,
            inbound_session_capacity: as_u64(self.limits.inbound_session_capacity)?,
            inbound_sessions_per_peer: as_u64(self.limits.inbound_sessions_per_peer)?,
            inbound_assembly_bytes: as_u64(self.limits.inbound_assembly_bytes)?,
            inbound_assembly_bytes_per_peer: as_u64(self.limits.inbound_assembly_bytes_per_peer)?,
            deferred_block_capacity: as_u64(self.limits.deferred_block_capacity)?,
            future_block_distance: self.limits.future_block_distance,
            request_timeout_secs: self.limits.request_timeout.as_secs(),
            request_timeout_nanos: self.limits.request_timeout.subsec_nanos(),
            outbound_sessions_per_source: as_u64(self.limits.outbound_sessions_per_source)?,
            outbound_bytes_per_source: as_u64(self.limits.outbound_bytes_per_source)?,
            server_request_gates_per_source: as_u64(self.limits.server_request_gates_per_source)?,
        })
    }

    fn lifecycle_geometry(&self) -> Result<MergeSidecarLifecycleGeometryV3, MergeSidecarError> {
        self.lifecycle_geometry_for_server_roster(
            self.server_stream_capacity,
            self.server_roster_digest.clone(),
        )
    }

    fn lifecycle_geometry_for_server_roster(
        &self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<MergeSidecarLifecycleGeometryV3, MergeSidecarError> {
        let as_u64 = |value: usize| {
            u64::try_from(value)
                .map_err(|_| MergeSidecarError::Capacity("lifecycle journal geometry"))
        };
        let (server_request_gate_capacity, server_request_attempt_capacity) =
            Self::derive_server_request_capacities(
                self.reply_source_capacity,
                self.limits,
                server_stream_capacity,
            )?;
        Ok(MergeSidecarLifecycleGeometryV3 {
            runtime: self.lifecycle_runtime_geometry_v3()?,
            server_roster_digest,
            server_stream_capacity: as_u64(server_stream_capacity)?,
            server_request_gate_capacity: as_u64(server_request_gate_capacity)?,
            server_request_attempt_capacity: as_u64(server_request_attempt_capacity)?,
        })
    }

    fn lifecycle_max_snapshot_bytes_for_attempt_capacity(
        server_request_attempt_capacity: usize,
    ) -> Result<usize, MergeSidecarError> {
        let gate_bytes = server_request_attempt_capacity
            .checked_mul(LIFECYCLE_JOURNAL_GATE_BYTES)
            .ok_or(MergeSidecarError::Capacity(
                "lifecycle journal gate byte geometry",
            ))?;
        // Requester and responder semantic-stream records have distinct hard
        // bounds; each record includes its durable non-zero stream epoch.
        let stream_bytes = MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
            .checked_add(MAX_CERTIFIED_MERGE_SERVER_STREAMS)
            .and_then(|count| count.checked_mul(LIFECYCLE_JOURNAL_STREAM_BYTES))
            .ok_or(MergeSidecarError::Capacity(
                "lifecycle journal stream byte geometry",
            ))?;
        LIFECYCLE_JOURNAL_BASE_BYTES
            .checked_add(gate_bytes)
            .and_then(|bytes| bytes.checked_add(stream_bytes))
            .ok_or(MergeSidecarError::Capacity(
                "lifecycle journal total byte geometry",
            ))
    }

    fn lifecycle_protocol_max_snapshot_bytes(&self) -> Result<usize, MergeSidecarError> {
        let (_, attempts) = Self::derive_server_request_capacities(
            self.reply_source_capacity,
            self.limits,
            MAX_CERTIFIED_MERGE_SERVER_STREAMS,
        )?;
        Self::lifecycle_max_snapshot_bytes_for_attempt_capacity(attempts)
    }

    fn lifecycle_snapshot(&self) -> Result<MergeSidecarLifecycleSnapshotV3, MergeSidecarError> {
        let request_streams = self
            .request_streams
            .iter()
            .map(|(responder, stream)| RequestStreamLifecycleV3 {
                responder: responder.clone(),
                service_generation: stream.service_generation,
                stream_epoch: stream.stream_epoch,
                next_sequence: stream.next_sequence,
                closed_through: stream.closed_through,
                acknowledged_through: stream.acknowledged_through,
            })
            .collect();
        let server_streams = self
            .server_streams
            .iter()
            .map(|(requester, stream)| ServerStreamLifecycleV3 {
                requester: requester.clone(),
                service_generation: self.server_service_generation,
                stream_epoch: stream.stream_epoch,
                closed_through: stream.closed_through,
                highest_sequence: stream.highest_sequence,
            })
            .collect();
        let mut server_request_gates = Vec::with_capacity(self.server_request_gates.len());
        for ((requester, request_id), gate) in &self.server_request_gates {
            let mut attempts = Vec::with_capacity(gate.attempts.len());
            for (source, attempt) in &gate.attempts {
                let source = match source {
                    ServerRequestSource::Synthetic(peer) => {
                        DurableServerRequestSourceV3::Synthetic(peer.clone())
                    }
                    ServerRequestSource::Authenticated(_) => {
                        let route = attempt.reply_route.as_ref().ok_or_else(|| {
                            MergeSidecarError::LifecycleJournal(
                                "authenticated lifecycle attempt lost its route source".to_owned(),
                            )
                        })?;
                        DurableServerRequestSourceV3::Authenticated(
                            route.authenticated_source_peer().clone(),
                        )
                    }
                    ServerRequestSource::RecoveredAuthenticated(peer) => {
                        DurableServerRequestSourceV3::Authenticated(peer.clone())
                    }
                };
                let cursor = match attempt.cursor {
                    ServerResponseCursor::Pending(index) => DurableServerResponseCursorV3::Pending(
                        u64::try_from(index).map_err(|_| {
                            MergeSidecarError::LifecycleJournal(
                                "server response cursor is not representable".to_owned(),
                            )
                        })?,
                    ),
                    ServerResponseCursor::Complete => DurableServerResponseCursorV3::Complete,
                };
                attempts.push(ServerRequestAttemptLifecycleV3 {
                    source,
                    cursor,
                    pending_flush_chunk: attempt
                        .pending_flush_chunk
                        .as_ref()
                        .map(ServerPendingChunkLifecycleV3::from),
                });
            }
            server_request_gates.push(ServerRequestGateLifecycleV3 {
                requester: requester.clone(),
                request_id: *request_id,
                request: gate.request.clone(),
                request_hash: gate.request_hash,
                service_generation: gate.service_generation,
                stream_epoch: gate.stream_epoch,
                semantic_sequence: gate.semantic_sequence,
                source_capacity: gate.source_capacity.map(|capacity| {
                    u64::try_from(capacity)
                        .expect("validated reply-source capacity is representable as u64")
                }),
                attempts,
            });
        }
        Ok(MergeSidecarLifecycleSnapshotV3::new(
            MergeSidecarLifecyclePayloadV3 {
                version: LIFECYCLE_JOURNAL_VERSION_V3,
                root_generation: self
                    .lifecycle_journal
                    .as_ref()
                    .and_then(|journal| journal.committed.as_ref())
                    .map_or(0, |marker| marker.root_generation),
                geometry: self.lifecycle_geometry()?,
                next_stream_epoch: self.next_stream_epoch,
                server_service_generation: self.server_service_generation,
                materialization_requester_cursor: self.materialization_requester_cursor.clone(),
                request_streams,
                server_streams,
                server_request_gates,
            },
        ))
    }

    fn configure_prior_lifecycle_server_geometry(
        &mut self,
        geometry: &MergeSidecarLifecycleGeometryV3,
    ) -> Result<(), MergeSidecarError> {
        if geometry.runtime != self.lifecycle_runtime_geometry_v3()? {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal non-roster geometry drift".to_owned(),
            ));
        }
        let server_stream_capacity =
            usize::try_from(geometry.server_stream_capacity).map_err(|_| {
                MergeSidecarError::LifecycleJournal(
                    "durable server roster capacity is not representable".to_owned(),
                )
            })?;
        let (gates, attempts) = Self::derive_server_request_capacities(
            self.reply_source_capacity,
            self.limits,
            server_stream_capacity,
        )
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))?;
        if usize::try_from(geometry.server_request_gate_capacity).ok() != Some(gates)
            || usize::try_from(geometry.server_request_attempt_capacity).ok() != Some(attempts)
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "durable server roster geometry is internally inconsistent".to_owned(),
            ));
        }
        self.configure_server_roster_geometry(
            server_stream_capacity,
            geometry.server_roster_digest.clone(),
        )
        .map_err(|error| MergeSidecarError::LifecycleJournal(error.to_string()))
    }

    fn restore_lifecycle_snapshot(
        &mut self,
        snapshot: MergeSidecarLifecycleSnapshotV3,
        now: Instant,
    ) -> Result<(), MergeSidecarError> {
        if !snapshot.integrity_is_valid() {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal payload digest mismatch".to_owned(),
            ));
        }
        let snapshot = snapshot.payload;
        if snapshot.version != LIFECYCLE_JOURNAL_VERSION_V3
            || snapshot.geometry != self.lifecycle_geometry()?
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "unsupported lifecycle journal version or geometry drift".to_owned(),
            ));
        }
        if snapshot.request_streams.len() > MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
            || snapshot.server_streams.len() > self.server_stream_capacity
            || snapshot.server_request_gates.len() > self.server_request_gate_capacity
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "lifecycle journal exceeds configured source geometry".to_owned(),
            ));
        }
        let mut request_streams = BTreeMap::new();
        let mut requester_epochs = BTreeSet::new();
        for stream in snapshot.request_streams {
            if stream.acknowledged_through > stream.closed_through
                || stream.closed_through > stream.next_sequence
                || stream.stream_epoch.get() > snapshot.next_stream_epoch
                || !requester_epochs.insert(stream.stream_epoch)
            {
                return Err(MergeSidecarError::LifecycleJournal(
                    "request stream lifecycle regressed".to_owned(),
                ));
            }
            let recovered = RequestStreamState {
                service_generation: stream.service_generation,
                stream_epoch: stream.stream_epoch,
                next_sequence: stream.next_sequence,
                closed_through: stream.next_sequence,
                acknowledged_through: stream.acknowledged_through,
                last_close_sent_at: None,
                last_close_message_hash: None,
                open_sequences: BTreeSet::new(),
            };
            if request_streams
                .insert(stream.responder, recovered)
                .is_some()
            {
                return Err(MergeSidecarError::LifecycleJournal(
                    "duplicate request stream lifecycle".to_owned(),
                ));
            }
        }
        let mut server_streams = BTreeMap::new();
        for stream in snapshot.server_streams {
            if stream.closed_through > stream.highest_sequence
                || stream.service_generation != snapshot.server_service_generation
                || server_streams
                    .insert(
                        stream.requester,
                        ServerStreamState {
                            stream_epoch: stream.stream_epoch,
                            closed_through: stream.closed_through,
                            highest_sequence: stream.highest_sequence,
                        },
                    )
                    .is_some()
            {
                return Err(MergeSidecarError::LifecycleJournal(
                    "server stream floor/high-water lifecycle diverged".to_owned(),
                ));
            }
        }
        let max_chunk_count =
            MAX_MERGE_LEDGER_ENTRY_BYTES.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        let mut server_request_gates = BTreeMap::new();
        let mut server_occurrences = BTreeSet::new();
        let mut requester_gate_counts = BTreeMap::<PeerId, usize>::new();
        let mut source_gate_counts = BTreeMap::<ServerRequestBudgetSource, usize>::new();
        let mut total_attempts = 0usize;
        for gate in snapshot.server_request_gates {
            let gate_requester = gate.requester.clone();
            let stream = server_streams.get(&gate.requester).ok_or_else(|| {
                MergeSidecarError::LifecycleJournal(
                    "server request gate has no stream state".to_owned(),
                )
            })?;
            if gate.request.requester != gate.requester
                || gate.request_id != gate.request.request_id
                || gate.request_hash != HashOf::new(&gate.request)
                || gate.request.request_id != gate.request.canonical_request_id()
                || gate.request.service_generation != gate.service_generation
                || gate.request.stream_epoch != gate.stream_epoch
                || gate.request.semantic_sequence != gate.semantic_sequence
                || gate.request.closed_through >= gate.request.semantic_sequence.get()
                || gate.request.encoded_len == 0
                || gate.request.encoded_len
                    > u64::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES)
                        .expect("maximum merge entry size fits u64")
                || gate.service_generation != snapshot.server_service_generation
                || gate.stream_epoch != stream.stream_epoch
                || gate.request.closed_through > stream.closed_through
                || gate.semantic_sequence.get() <= stream.closed_through
                || gate.semantic_sequence.get() > stream.highest_sequence
                || gate.attempts.is_empty()
                || gate.source_capacity.is_some_and(|capacity| {
                    usize::try_from(capacity).ok() != Some(self.reply_source_capacity)
                })
            {
                return Err(MergeSidecarError::LifecycleJournal(
                    "invalid durable server request gate".to_owned(),
                ));
            }
            if !server_occurrences.insert((
                gate.requester.clone(),
                gate.service_generation,
                gate.stream_epoch,
                gate.semantic_sequence,
            )) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "duplicate durable server semantic occurrence".to_owned(),
                ));
            }
            let source_capacity = gate
                .source_capacity
                .map(|capacity| {
                    usize::try_from(capacity).map_err(|_| {
                        MergeSidecarError::LifecycleJournal(
                            "durable source capacity is not representable".to_owned(),
                        )
                    })
                })
                .transpose()?;
            let response_len = usize::try_from(gate.request.encoded_len).map_err(|_| {
                MergeSidecarError::LifecycleJournal(
                    "durable request length is not representable".to_owned(),
                )
            })?;
            let expected_chunk_count = response_len.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
            if gate.attempts.len() > source_capacity.unwrap_or(1) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "durable server attempts exceed their source capacity".to_owned(),
                ));
            }
            total_attempts = total_attempts
                .checked_add(gate.attempts.len())
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "server request attempt count overflowed".to_owned(),
                    )
                })?;
            if total_attempts > self.server_request_attempt_capacity {
                return Err(MergeSidecarError::LifecycleJournal(
                    "durable server attempts exceed configured geometry".to_owned(),
                ));
            }
            let mut attempts = BTreeMap::new();
            for attempt in gate.attempts {
                let source = match attempt.source {
                    DurableServerRequestSourceV3::Synthetic(peer)
                        if source_capacity.is_none() && peer == gate.requester =>
                    {
                        ServerRequestSource::Synthetic(peer)
                    }
                    DurableServerRequestSourceV3::Authenticated(peer)
                        if source_capacity.is_some() =>
                    {
                        ServerRequestSource::RecoveredAuthenticated(peer)
                    }
                    DurableServerRequestSourceV3::Synthetic(_)
                    | DurableServerRequestSourceV3::Authenticated(_) => {
                        return Err(MergeSidecarError::LifecycleJournal(
                            "durable server source kind differs from its route geometry".to_owned(),
                        ));
                    }
                };
                let cursor = match attempt.cursor {
                    DurableServerResponseCursorV3::Pending(index) => {
                        let index = usize::try_from(index).map_err(|_| {
                            MergeSidecarError::LifecycleJournal(
                                "durable server cursor is not representable".to_owned(),
                            )
                        })?;
                        if index >= expected_chunk_count || index >= max_chunk_count {
                            return Err(MergeSidecarError::LifecycleJournal(
                                "durable server cursor exceeds the maximum response".to_owned(),
                            ));
                        }
                        ServerResponseCursor::Pending(index)
                    }
                    DurableServerResponseCursorV3::Complete => ServerResponseCursor::Complete,
                };
                let pending_flush_chunk = attempt
                    .pending_flush_chunk
                    .map(ServerPendingChunkIdentity::from);
                if matches!(cursor, ServerResponseCursor::Complete) && pending_flush_chunk.is_some()
                {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "terminal durable cursor retained an in-flight chunk".to_owned(),
                    ));
                }
                if let Some(pending) = &pending_flush_chunk {
                    let ServerResponseCursor::Pending(index) = cursor else {
                        unreachable!("terminal pending identity rejected above")
                    };
                    if pending.request_id != gate.request_id
                        || pending.service_generation != gate.service_generation
                        || pending.stream_epoch != gate.stream_epoch
                        || pending.semantic_sequence != gate.semantic_sequence
                        || pending.entry_hash != gate.request.entry_hash
                        || pending.encoded_len != gate.request.encoded_len
                        || pending.epoch_id != gate.request.epoch_id
                        || pending.reference_digest != gate.request.reference_digest
                        || pending.requester != gate.requester
                        || pending.responder != gate.request.responder
                        || usize::try_from(pending.chunk_index).ok() != Some(index)
                        || usize::try_from(pending.chunk_count).ok() != Some(expected_chunk_count)
                        || pending.chunk_index >= pending.chunk_count
                    {
                        return Err(MergeSidecarError::LifecycleJournal(
                            "durable pending chunk differs from its request gate".to_owned(),
                        ));
                    }
                }
                let materialization_retryable = matches!(cursor, ServerResponseCursor::Pending(_));
                if attempts
                    .insert(
                        source.clone(),
                        ServerRequestGateAttempt {
                            reply_route: None,
                            materialization_authorized: false,
                            authorized_materialization_route: None,
                            materialization_retryable,
                            cursor,
                            pending_flush_chunk,
                            inserted: now,
                        },
                    )
                    .is_some()
                {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "duplicate durable server source attempt".to_owned(),
                    ));
                }
                if attempts
                    .keys()
                    .filter(|retained| retained.shares_budget_with(&source))
                    .count()
                    != 1
                {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "durable server gate duplicates an authenticated source budget".to_owned(),
                    ));
                }
            }
            for source in attempts
                .keys()
                .map(ServerRequestSource::budget_source)
                .collect::<BTreeSet<_>>()
            {
                let count = source_gate_counts.entry(source).or_default();
                *count = count.checked_add(1).ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "durable authenticated-source gate count overflowed".to_owned(),
                    )
                })?;
                if *count > self.limits.server_request_gates_per_source {
                    return Err(MergeSidecarError::LifecycleJournal(
                        "durable gates exceed configured authenticated-source geometry".to_owned(),
                    ));
                }
            }
            let requester_count = requester_gate_counts.entry(gate_requester).or_default();
            *requester_count = requester_count.checked_add(1).ok_or_else(|| {
                MergeSidecarError::LifecycleJournal(
                    "durable requester gate count overflowed".to_owned(),
                )
            })?;
            if *requester_count > self.limits.inbound_sessions_per_peer {
                return Err(MergeSidecarError::LifecycleJournal(
                    "durable requester gates exceed their forward window".to_owned(),
                ));
            }
            let key = (gate.requester, gate.request_id);
            if server_request_gates
                .insert(
                    key,
                    ServerRequestGate {
                        request: gate.request,
                        request_hash: gate.request_hash,
                        service_generation: gate.service_generation,
                        stream_epoch: gate.stream_epoch,
                        semantic_sequence: gate.semantic_sequence,
                        source_capacity,
                        attempts,
                    },
                )
                .is_some()
            {
                return Err(MergeSidecarError::LifecycleJournal(
                    "duplicate durable server request gate".to_owned(),
                ));
            }
        }
        if server_streams.iter().any(|(requester, stream)| {
            let retained_high_water = server_request_gates
                .iter()
                .filter(|(key, gate)| {
                    &key.0 == requester && gate.stream_epoch == stream.stream_epoch
                })
                .map(|(_, gate)| gate.semantic_sequence.get())
                .fold(stream.closed_through, u64::max);
            stream.highest_sequence != retained_high_water
        }) {
            return Err(MergeSidecarError::LifecycleJournal(
                "server stream high-water differs from durable request gates".to_owned(),
            ));
        }
        if snapshot
            .materialization_requester_cursor
            .as_ref()
            .is_some_and(|requester| !server_streams.contains_key(requester))
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "materialization cursor names no retained server stream".to_owned(),
            ));
        }
        self.next_stream_epoch = snapshot.next_stream_epoch;
        self.request_streams = request_streams;
        self.server_service_generation = snapshot.server_service_generation;
        self.server_streams = server_streams;
        self.materialization_requester_cursor = snapshot.materialization_requester_cursor;
        self.server_request_gates = server_request_gates;
        self.outbound.clear();
        self.outbound_order.clear();
        self.pending_server_closures.clear();
        // Exact-output fanouts and writer-flush receipts are process-local and
        // cannot survive the restart which owns snapshot restoration.
        self.server_closure_handoff_pending = false;
        Ok(())
    }

    /// Open the crash-safe semantic lifecycle journal under the Kura root.
    #[cfg(test)]
    pub(crate) fn open_durable(
        store_root: &Path,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
    ) -> Result<Self, MergeSidecarError> {
        Self::open_durable_with_server_stream_capacity(
            store_root,
            reply_source_capacity,
            limits,
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,
            unbound_test_merge_sidecar_roster_digest(),
        )
    }

    /// Open the crash-safe lifecycle journal for one canonical roster.
    ///
    /// A valid prior snapshot with a different roster identity is restored
    /// under its own recorded geometry, then crash-consistently fenced into
    /// the supplied roster before this method returns.
    pub(crate) fn open_durable_with_server_stream_capacity(
        store_root: &Path,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<Self, MergeSidecarError> {
        let target_roster_digest = server_roster_digest.clone();
        let mut transport = Self::with_limits_and_server_stream_capacity(
            reply_source_capacity,
            limits,
            server_stream_capacity,
            server_roster_digest,
        )?;
        let (mut journal, snapshot) = MergeSidecarLifecycleJournal::open(
            store_root,
            transport.lifecycle_protocol_max_snapshot_bytes()?,
        )?;
        let restored = snapshot.is_some();
        if let Some(snapshot) = snapshot.as_ref() {
            transport.configure_prior_lifecycle_server_geometry(&snapshot.payload.geometry)?;
            transport.restore_lifecycle_snapshot(snapshot.clone(), Instant::now())?;
        }
        journal.finalize_validated_open(snapshot.as_ref())?;
        transport.lifecycle_journal = Some(journal);
        transport = if restored {
            transport.rehydrate_after_lifecycle_restore(
                reply_source_capacity,
                limits,
                server_stream_capacity,
                target_roster_digest,
                RestoredLifecycleResponderFence,
            )?
        } else {
            transport.rehydrate_with_exact_geometry(
                reply_source_capacity,
                limits,
                server_stream_capacity,
                target_roster_digest,
                Instant::now(),
            )?
        };
        transport.persist_lifecycle_state()?;
        Ok(transport)
    }

    fn persist_lifecycle_projection(
        &mut self,
        snapshot: MergeSidecarLifecycleSnapshotV3,
    ) -> Result<(), MergeSidecarError> {
        let Some(journal) = self.lifecycle_journal.as_mut() else {
            return Ok(());
        };
        journal.persist_next(snapshot)?;
        Ok(())
    }

    fn preflight_lifecycle_mutation(&mut self) -> Result<(), MergeSidecarError> {
        let Some(journal) = self.lifecycle_journal.as_mut() else {
            return Ok(());
        };
        journal.preflight_next_commit()
    }

    /// Atomically persist all semantic request ownership and non-regressing cursors.
    pub(crate) fn persist_lifecycle_state(&mut self) -> Result<(), MergeSidecarError> {
        if self.lifecycle_journal.is_none() {
            return Ok(());
        }
        let snapshot = self.lifecycle_snapshot()?;
        self.persist_lifecycle_projection(snapshot)
    }

    /// Obstruct the next durable lifecycle write for lane fail-stop tests.
    #[cfg(test)]
    pub(crate) fn obstruct_lifecycle_journal_temp_for_test(&self) {
        let journal = self
            .lifecycle_journal
            .as_ref()
            .expect("durable merge-sidecar transport has a lifecycle journal");
        fs::create_dir(journal.temp_path())
            .expect("create an unsafe lifecycle journal temp artifact");
    }

    /// Return the lifecycle-state replacement path for crash-boundary tests.
    #[cfg(test)]
    pub(crate) fn lifecycle_journal_temp_path_for_test(&self) -> PathBuf {
        self.lifecycle_journal
            .as_ref()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .temp_path()
    }

    /// Return the lifecycle state path for exact crash-boundary assertions.
    #[cfg(test)]
    pub(crate) fn lifecycle_journal_state_path_for_test(&self) -> PathBuf {
        self.lifecycle_journal
            .as_ref()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .state_path()
    }

    /// Return the independent root high-water path for crash-boundary assertions.
    #[cfg(test)]
    pub(crate) fn lifecycle_root_high_water_path_for_test(&self) -> PathBuf {
        self.lifecycle_journal
            .as_ref()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .root_high_water_path()
    }

    /// Inject a one-shot failure after state replacement but before its parent fsync.
    #[cfg(test)]
    pub(crate) fn fail_after_lifecycle_state_replace_before_sync_for_test(&mut self) {
        self.lifecycle_journal
            .as_mut()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .fail_after_state_replace_before_directory_sync = true;
    }

    /// Inject a one-shot failure after state publication but before root publication.
    #[cfg(test)]
    pub(crate) fn fail_after_lifecycle_state_publish_for_test(&mut self) {
        self.lifecycle_journal
            .as_mut()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .fail_after_state_publish = true;
    }

    /// Inject a one-shot failure after root replacement but before its parent fsync.
    #[cfg(test)]
    pub(crate) fn fail_after_lifecycle_root_replace_before_sync_for_test(&mut self) {
        self.lifecycle_journal
            .as_mut()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .fail_after_root_replace_before_store_sync = true;
    }

    /// Inject a one-shot failure after the root commit is durable but before memory publication.
    #[cfg(test)]
    pub(crate) fn fail_after_lifecycle_root_publish_for_test(&mut self) {
        self.lifecycle_journal
            .as_mut()
            .expect("durable merge-sidecar transport has a lifecycle journal")
            .fail_after_root_publish = true;
    }

    /// Obstruct only the terminal-retirement write after admission is durable.
    #[cfg(test)]
    pub(crate) fn obstruct_next_terminal_retirement_persist_for_test(&mut self) {
        assert!(
            self.lifecycle_journal.is_some(),
            "terminal-retirement obstruction requires a durable transport"
        );
        self.obstruct_next_terminal_retirement_persist = true;
    }

    /// Return whether an exact server gate remains owned by the transport.
    #[cfg(test)]
    pub(crate) fn has_server_request_gate_for_test(
        &self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) -> bool {
        self.server_request_gates
            .get(&(sender.clone(), request.request_id))
            .is_some_and(|gate| gate.request.same_occurrence_except_close_floor(request))
    }

    /// Advance an otherwise quiescent responder fence for a changed test roster.
    #[cfg(test)]
    pub(crate) fn transition_server_service_generation_for_test(
        &mut self,
        server_roster: &[PeerId],
    ) -> Result<(), MergeSidecarError> {
        self.transition_server_service_generation_with_capacity_for_test(
            server_roster.len(),
            server_roster,
        )
    }

    /// Advance a quiescent responder fence with explicit test-only reserved
    /// stream geometry.
    #[cfg(test)]
    pub(crate) fn transition_server_service_generation_with_capacity_for_test(
        &mut self,
        server_stream_capacity: usize,
        server_roster: &[PeerId],
    ) -> Result<(), MergeSidecarError> {
        self.transition_server_service_generation(
            server_stream_capacity,
            canonical_merge_sidecar_roster_digest(server_roster),
        )
    }

    /// Reuse process-local ownership across an identity-checked height rollover.
    ///
    /// An identical canonical roster preserves responder state and requires
    /// identical capacity. A changed roster advances the durable service
    /// generation together with the new geometry only after every
    /// responder-owned stream and output occurrence is terminal. Requester-side
    /// streams and inbound assemblies survive either path.
    pub(crate) fn rehydrate_with_exact_geometry(
        mut self,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
        _now: Instant,
    ) -> Result<Self, MergeSidecarError> {
        self.validate_retained_height_geometry(reply_source_capacity, limits)?;
        if self.server_roster_digest != server_roster_digest {
            self.transition_server_service_generation(
                server_stream_capacity,
                server_roster_digest,
            )?;
            return Ok(self);
        }
        if self.server_stream_capacity != server_stream_capacity {
            return Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height roster capacity drift",
            ));
        }
        self.requeue_retained_outbound_after_height_rollover();
        Ok(self)
    }

    /// Rehydrate after consuming the exact predecessor's durable output handoff.
    ///
    /// Equal roster identity preserves responder state and reproduces each
    /// current chunk for the successor's exact writer. A changed certified
    /// roster may supersede non-terminal responder tables because the move-only
    /// authority proves that every predecessor exact-output writer and queued
    /// occurrence has been durably sealed. The successor generation and empty
    /// responder projection are committed before memory changes; requester-side
    /// streams and inbound assemblies are preserved.
    pub(crate) fn rehydrate_with_exact_geometry_after_durable_handoff(
        mut self,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
        _now: Instant,
        authority: DurableMergeSidecarRolloverAuthority,
    ) -> Result<Self, MergeSidecarError> {
        self.validate_retained_height_geometry(reply_source_capacity, limits)?;
        if self.server_roster_digest != server_roster_digest {
            self.transition_server_service_generation_after_durable_handoff(
                server_stream_capacity,
                server_roster_digest,
                authority,
            )?;
            return Ok(self);
        }
        drop(authority);
        if self.server_stream_capacity != server_stream_capacity {
            return Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height roster capacity drift",
            ));
        }
        // The exact-output handoff sealed any previously drained closure
        // occurrence. Undrained authenticated prefixes remain queued.
        self.server_closure_handoff_pending = false;
        self.requeue_retained_outbound_after_height_rollover();
        Ok(self)
    }

    fn rehydrate_after_lifecycle_restore(
        mut self,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
        _authority: RestoredLifecycleResponderFence,
    ) -> Result<Self, MergeSidecarError> {
        self.validate_retained_height_geometry(reply_source_capacity, limits)?;
        if self.server_roster_digest != server_roster_digest {
            self.transition_server_service_generation_after_exact_output_fence(
                server_stream_capacity,
                server_roster_digest,
            )?;
            return Ok(self);
        }
        if server_stream_capacity < self.server_stream_capacity {
            return Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height roster capacity drift",
            ));
        }
        if server_stream_capacity > self.server_stream_capacity {
            // A validated restart fence has already made every process-local
            // writer unreachable.  Expanding the aggregate responder bound
            // for the exact same canonical roster therefore preserves every
            // semantic stream and generation while adding only empty slots.
            // The caller persists the expanded V3 geometry before returning
            // the transport.  A crash before that commit safely replays this
            // monotonic migration from the predecessor snapshot.
            self.configure_server_roster_geometry(server_stream_capacity, server_roster_digest)?;
        }
        self.requeue_retained_outbound_after_height_rollover();
        Ok(self)
    }

    fn validate_retained_height_geometry(
        &self,
        reply_source_capacity: usize,
        limits: MergeSidecarLimits,
    ) -> Result<(), MergeSidecarError> {
        if self.reply_source_capacity != reply_source_capacity || self.limits != limits {
            return Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height geometry drift",
            ));
        }
        Ok(())
    }

    fn requeue_retained_outbound_after_height_rollover(&mut self) {
        // The height-local exact-output worker has already relinquished its
        // writer occurrences under the durable rollover authority. Preserve
        // each source's current chunk, but make every formerly in-flight item
        // eligible for an exact retry in this height. The receiver deduplicates
        // the immutable chunk identity if the prior writer flushed before its
        // acknowledgement was observed.
        let mut retained_order = BTreeSet::new();
        self.outbound_order.retain(|attempt_key| {
            let valid = self
                .outbound
                .get(&attempt_key.0)
                .is_some_and(|transfer| transfer.attempts.contains_key(&attempt_key.1));
            valid && retained_order.insert(attempt_key.clone())
        });
        for (key, transfer) in &mut self.outbound {
            for (source, attempt) in &mut transfer.attempts {
                attempt.in_flight_chunk = None;
                let attempt_key = (key.clone(), source.clone());
                if retained_order.insert(attempt_key.clone()) {
                    self.outbound_order.push_back(attempt_key);
                }
                attempt.queued = true;
            }
        }
    }

    fn validate_reference_len(
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<usize, MergeSidecarError> {
        let len = usize::try_from(reference.encoded_len)
            .map_err(|_| MergeSidecarError::InvalidEncodedLength(reference.encoded_len))?;
        if len == 0 || len > MAX_MERGE_LEDGER_ENTRY_BYTES {
            return Err(MergeSidecarError::InvalidEncodedLength(
                reference.encoded_len,
            ));
        }
        Ok(len)
    }

    fn allocate_request_sequence(
        &mut self,
        responder: &PeerId,
    ) -> Result<
        (
            CertifiedMergeSidecarStreamEpochV1,
            CertifiedMergeSidecarSemanticSequenceV1,
            u64,
        ),
        MergeSidecarError,
    > {
        if !self.request_streams.contains_key(responder) {
            let reclaim = (self.request_streams.len() >= MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)
                .then(|| {
                    self.request_streams
                        .iter()
                        // A sent Close remains durable retry ownership until
                        // its exact CloseAck advances the acknowledgement floor.
                        .find(|(_, stream)| {
                            stream.open_sequences.is_empty()
                                && stream.closed_through == stream.acknowledged_through
                        })
                        .map(|(peer, _)| peer.clone())
                })
                .flatten();
            if self.request_streams.len() >= MAX_CERTIFIED_MERGE_SEMANTIC_PEERS && reclaim.is_none()
            {
                return Err(MergeSidecarError::Capacity(
                    "requester semantic responder geometry",
                ));
            }
            // Stage every fallible operation before reclaiming durable state.
            // An exhausted epoch counter must leave both memory and journal
            // byte-for-byte unchanged.
            let next_stream_epoch =
                self.next_stream_epoch
                    .checked_add(1)
                    .ok_or(MergeSidecarError::Capacity(
                        "semantic stream epoch exhausted",
                    ))?;
            let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
                NonZeroU64::new(next_stream_epoch)
                    .expect("a successfully incremented stream epoch is non-zero"),
            );
            if let Some(reclaim) = reclaim {
                self.request_streams.remove(&reclaim);
            }
            self.next_stream_epoch = next_stream_epoch;
            self.request_streams
                .insert(responder.clone(), RequestStreamState::new(stream_epoch));
        }
        let stream = self
            .request_streams
            .get_mut(responder)
            .expect("request stream was inserted above");
        let (semantic_sequence, closed_through) = stream.allocate()?;
        Ok((stream.stream_epoch, semantic_sequence, closed_through))
    }

    fn close_request_sequence(
        &mut self,
        responder: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
    ) {
        if let Some(stream) = self
            .request_streams
            .get_mut(responder)
            .filter(|stream| stream.stream_epoch == stream_epoch)
        {
            stream.close(semantic_sequence);
        }
    }

    fn due_close_responders(&self, now: Instant) -> VecDeque<PeerId> {
        self.request_streams
            .iter()
            .filter(|(_, stream)| stream.close_due(now, self.limits.request_timeout))
            .map(|(responder, _)| responder.clone())
            .collect()
    }

    fn begin_close(
        &mut self,
        requester: &PeerId,
        responder: &PeerId,
        now: Instant,
    ) -> Option<MergeSidecarPost> {
        let stream = self.request_streams.get_mut(responder)?;
        if !stream.close_due(now, self.limits.request_timeout) {
            return None;
        }
        let close = stream.emit_close(requester, responder, now);
        Some(MergeSidecarPost {
            peer: responder.clone(),
            reply_route: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Close(close)),
        })
    }

    fn begin_request_or_close(
        &mut self,
        requester: &PeerId,
        idle: &mut VecDeque<InboundSidecarKey>,
        close_responders: &mut VecDeque<PeerId>,
        now: Instant,
    ) -> Result<Option<MergeSidecarPost>, MergeSidecarError> {
        let contended = !idle.is_empty() && !close_responders.is_empty();
        let close_first = !close_responders.is_empty() && (idle.is_empty() || self.tick_close_next);
        if close_first {
            while let Some(responder) = close_responders.pop_front() {
                if let Some(post) = self.begin_close(requester, &responder, now) {
                    self.timeout_retry_close_deferred = false;
                    if contended {
                        self.tick_close_next = false;
                    }
                    return Ok(Some(post));
                }
            }
        }
        while let Some(key) = idle.pop_front() {
            if let Some(post) = self.begin_request(key, requester, now)? {
                if contended {
                    self.tick_close_next = true;
                }
                return Ok(Some(post));
            }
        }
        if !close_first {
            while let Some(responder) = close_responders.pop_front() {
                if let Some(post) = self.begin_close(requester, &responder, now) {
                    self.timeout_retry_close_deferred = false;
                    return Ok(Some(post));
                }
            }
        }
        Ok(None)
    }

    pub(crate) fn acknowledge_close(
        &mut self,
        sender: &PeerId,
        ack: &CertifiedMergeSidecarCloseAckV1,
        local_peer: &PeerId,
    ) -> Result<bool, MergeSidecarError> {
        if ack.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
            return Err(MergeSidecarError::UnsupportedVersion(ack.version));
        }
        if &ack.requester != local_peer || &ack.responder != sender {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if ack.closed_through == 0 || ack.close_id != ack.canonical_close_id() {
            return Err(MergeSidecarError::CloseIdMismatch);
        }
        let Some(stream) = self.request_streams.get(sender) else {
            // The exact ACK may be duplicated after its first application
            // reclaimed the terminal stream. Its canonical identity and
            // authenticated endpoints were checked above, so retaining no
            // tombstone and treating the duplicate as a no-op is both bounded
            // and idempotent. A reallocated stream is still protected by the
            // generation/epoch check below.
            return Ok(false);
        };
        if stream.service_generation != ack.service_generation
            || stream.stream_epoch != ack.stream_epoch
        {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if ack.closed_through <= stream.acknowledged_through
            || ack.closed_through > stream.closed_through
        {
            return Ok(false);
        }
        self.preflight_lifecycle_mutation()?;
        let stream = self
            .request_streams
            .get_mut(sender)
            .expect("preflight cannot remove the checked request stream");
        let advanced = stream.acknowledge_close(ack.closed_through);
        debug_assert!(advanced, "the immutable preflight established ACK progress");
        let retire =
            stream.open_sequences.is_empty() && stream.acknowledged_through == stream.next_sequence;
        if retire {
            self.request_streams.remove(sender);
        }
        self.persist_lifecycle_state()?;
        Ok(advanced)
    }

    /// Apply an authenticated responder-generation fence.
    ///
    /// A strictly newer Hint retires every old-generation attempt to that
    /// responder, starts a fresh requester stream epoch, and persists the new
    /// fence before any retry can be emitted. Stale or unaffiliated Hints are
    /// harmless no-ops.
    pub(crate) fn acknowledge_generation_hint(
        &mut self,
        sender: &PeerId,
        hint: &CertifiedMergeSidecarGenerationHintV1,
        local_peer: &PeerId,
    ) -> Result<bool, MergeSidecarError> {
        if hint.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
            return Err(MergeSidecarError::UnsupportedVersion(hint.version));
        }
        if &hint.requester != local_peer || &hint.responder != sender {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if hint.hint_id != hint.canonical_hint_id() {
            return Err(MergeSidecarError::RequestIdMismatch);
        }
        let Some(stream) = self.request_streams.get(sender) else {
            return Ok(false);
        };
        if hint.current_generation <= stream.service_generation
            || hint.observed_generation > stream.service_generation
        {
            return Ok(false);
        }
        let observed_active_request = self.inbound.values().any(|assembly| {
            assembly.current.as_ref().is_some_and(|attempt| {
                &attempt.holder == sender
                    && attempt.service_generation == hint.observed_generation
                    && attempt.message_hash == hint.observed_message_hash
            })
        });
        let observed_close = stream.last_close_message_hash == Some(hint.observed_message_hash)
            && stream.service_generation == hint.observed_generation;
        if !observed_active_request && !observed_close {
            return Ok(false);
        }

        let next_stream_epoch =
            self.next_stream_epoch
                .checked_add(1)
                .ok_or(MergeSidecarError::Capacity(
                    "semantic stream epoch exhausted",
                ))?;
        let stream_epoch = CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(next_stream_epoch)
                .expect("a successfully incremented stream epoch is non-zero"),
        );
        let mut replacement = RequestStreamState::new(stream_epoch);
        replacement.service_generation = hint.current_generation;

        // Stage and persist the exact durable replacement before resetting any
        // process-local assembly or allowing a new request to be scheduled.
        if self.lifecycle_journal.is_some() {
            let mut snapshot = self.lifecycle_snapshot()?;
            snapshot.payload.next_stream_epoch = next_stream_epoch;
            let durable = snapshot
                .payload
                .request_streams
                .iter_mut()
                .find(|candidate| &candidate.responder == sender)
                .expect("live request stream is represented in its lifecycle snapshot");
            durable.service_generation = hint.current_generation;
            durable.stream_epoch = stream_epoch;
            durable.next_sequence = 0;
            durable.closed_through = 0;
            durable.acknowledged_through = 0;
            snapshot.payload_hash = HashOf::new(&snapshot.payload);
            self.persist_lifecycle_projection(snapshot)?;
        }

        for assembly in self.inbound.values_mut() {
            if assembly.current.as_ref().is_some_and(|attempt| {
                &attempt.holder == sender && attempt.service_generation < hint.current_generation
            }) {
                assembly.current = None;
                assembly.chunks.clear();
                assembly.received_bytes = 0;
                assembly.complete_pending_validation = false;
            }
        }
        self.next_stream_epoch = next_stream_epoch;
        self.request_streams.insert(sender.clone(), replacement);
        Ok(true)
    }

    fn inbound_peer_session_count(&self, peer: &PeerId) -> usize {
        self.inbound
            .values()
            .filter(|assembly| {
                assembly
                    .current
                    .as_ref()
                    .is_some_and(|attempt| &attempt.holder == peer)
            })
            .count()
    }

    fn ordinary_inbound_session_count(&self) -> usize {
        self.inbound
            .values()
            .filter(|assembly| assembly.priority == InboundPriority::Ordinary)
            .count()
    }

    fn ordinary_inbound_peer_session_count(&self, peer: &PeerId) -> usize {
        self.inbound
            .values()
            .filter(|assembly| {
                assembly.priority == InboundPriority::Ordinary
                    && assembly
                        .current
                        .as_ref()
                        .is_some_and(|attempt| &attempt.holder == peer)
            })
            .count()
    }

    fn inbound_received_bytes(&self) -> usize {
        self.inbound
            .values()
            .map(|assembly| assembly.received_bytes)
            .sum()
    }

    fn inbound_reserved_bytes(&self) -> usize {
        self.inbound
            .values()
            .map(|assembly| usize::try_from(assembly.reference.encoded_len).unwrap_or(usize::MAX))
            .sum()
    }

    fn ordinary_inbound_reserved_bytes(&self) -> usize {
        self.inbound
            .values()
            .filter(|assembly| assembly.priority == InboundPriority::Ordinary)
            .map(|assembly| usize::try_from(assembly.reference.encoded_len).unwrap_or(usize::MAX))
            .sum()
    }

    fn inbound_peer_reserved_bytes(&self, peer: &PeerId) -> usize {
        self.inbound
            .values()
            .filter(|assembly| {
                assembly
                    .current
                    .as_ref()
                    .is_some_and(|attempt| &attempt.holder == peer)
            })
            .map(|assembly| usize::try_from(assembly.reference.encoded_len).unwrap_or(usize::MAX))
            .sum()
    }

    fn ordinary_inbound_peer_reserved_bytes(&self, peer: &PeerId) -> usize {
        self.inbound
            .values()
            .filter(|assembly| {
                assembly.priority == InboundPriority::Ordinary
                    && assembly
                        .current
                        .as_ref()
                        .is_some_and(|attempt| &attempt.holder == peer)
            })
            .map(|assembly| usize::try_from(assembly.reference.encoded_len).unwrap_or(usize::MAX))
            .sum()
    }

    fn inbound_peer_received_bytes(&self, peer: &PeerId) -> usize {
        self.inbound
            .values()
            .filter(|assembly| {
                assembly
                    .current
                    .as_ref()
                    .is_some_and(|attempt| &attempt.holder == peer)
            })
            .map(|assembly| assembly.received_bytes)
            .sum()
    }

    fn deferred_count(&self) -> usize {
        self.inbound
            .values()
            .map(|assembly| assembly.deferred.len())
            .sum()
    }

    fn ordinary_deferred_count(&self) -> usize {
        self.inbound
            .values()
            .filter(|assembly| assembly.priority == InboundPriority::Ordinary)
            .map(|assembly| assembly.deferred.len())
            .sum()
    }

    fn begin_request(
        &mut self,
        key: InboundSidecarKey,
        requester: &PeerId,
        now: Instant,
    ) -> Result<Option<MergeSidecarPost>, MergeSidecarError> {
        let (holders, start_cursor, priority) = {
            let Some(assembly) = self.inbound.get(&key) else {
                return Ok(None);
            };
            if assembly.current.is_some() || assembly.complete_pending_validation {
                return Ok(None);
            }
            (
                assembly.holders.clone(),
                assembly.holder_cursor,
                assembly.priority,
            )
        };
        let selected = (0..holders.len()).find_map(|offset| {
            let index = (start_cursor + offset) % holders.len();
            let holder = &holders[index];
            let requested_len = self
                .inbound
                .get(&key)
                .and_then(|assembly| usize::try_from(assembly.reference.encoded_len).ok())
                .unwrap_or(usize::MAX);
            let full_peer_capacity = self.inbound_peer_session_count(holder)
                < self.limits.inbound_sessions_per_peer
                && self
                    .inbound_peer_reserved_bytes(holder)
                    .saturating_add(requested_len)
                    <= self.limits.inbound_assembly_bytes_per_peer;
            let priority_capacity = priority == InboundPriority::Decided
                || (self.ordinary_inbound_peer_session_count(holder)
                    < self.limits.inbound_sessions_per_peer - RESERVED_DECIDED_INBOUND_SESSIONS
                    && self
                        .ordinary_inbound_peer_reserved_bytes(holder)
                        .saturating_add(requested_len)
                        <= self.limits.inbound_assembly_bytes_per_peer
                            - RESERVED_DECIDED_INBOUND_BYTES);
            if holder == requester || !full_peer_capacity || !priority_capacity {
                None
            } else {
                Some((index, holder.clone()))
            }
        });
        let Some((holder_index, holder)) = selected else {
            return Ok(None);
        };
        self.preflight_lifecycle_mutation()?;
        let (stream_epoch, semantic_sequence, closed_through) =
            self.allocate_request_sequence(&holder)?;
        let service_generation = self
            .request_streams
            .get(&holder)
            .expect("request stream was allocated for the selected holder")
            .service_generation;
        let assembly = self
            .inbound
            .get_mut(&key)
            .expect("assembly exists while beginning request");
        let previous_attempts = assembly.attempts;
        assembly.attempts = assembly.attempts.saturating_add(1);
        assembly.holder_cursor = (holder_index + 1) % holders.len();
        assembly.chunks.clear();
        assembly.received_bytes = 0;
        let reference = &assembly.reference;
        let mut request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation,
            stream_epoch,
            semantic_sequence,
            closed_through,
            request_id: Hash::prehashed([0; Hash::LENGTH]),
            entry_hash: key.0,
            encoded_len: reference.encoded_len,
            epoch_id: reference.epoch_id,
            reference_digest: key.1,
            requester: requester.clone(),
            responder: holder.clone(),
        };
        request.bind_canonical_request_id();
        let message_hash = HashOf::new(&request).into();
        assembly.current = Some(RequestAttempt {
            id: request.request_id,
            message_hash,
            service_generation,
            stream_epoch,
            semantic_sequence,
            holder: holder.clone(),
            last_progress_at: now,
            previous_holder_cursor: start_cursor,
            previous_attempts,
        });
        self.inbound_cursor = Some(key);
        self.persist_lifecycle_state()?;
        Ok(Some(MergeSidecarPost {
            peer: holder,
            reply_route: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Request(request)),
        }))
    }

    /// Return a request to the idle state when the caller's bounded outbound
    /// queue could not retain the post. No network attempt occurred, so a
    /// later bounded tick may select a holder immediately without waiting for
    /// the ordinary response timeout.
    pub(crate) fn release_unsent_request(
        &mut self,
        request: &CertifiedMergeSidecarRequestV1,
    ) -> Result<(), MergeSidecarError> {
        let key = (request.entry_hash, request.reference_digest);
        let Some(assembly) = self.inbound.get(&key) else {
            return Ok(());
        };
        if !assembly.current.as_ref().is_some_and(|attempt| {
            attempt.id == request.request_id
                && attempt.service_generation == request.service_generation
                && attempt.stream_epoch == request.stream_epoch
                && attempt.semantic_sequence == request.semantic_sequence
                && attempt.holder == request.responder
        }) {
            return Ok(());
        }
        self.preflight_lifecycle_mutation()?;
        let assembly = self
            .inbound
            .get_mut(&key)
            .expect("preflight cannot remove the checked inbound assembly");
        let attempt = assembly
            .current
            .take()
            .expect("matching request attempt was checked above");
        assembly.holder_cursor = attempt.previous_holder_cursor;
        assembly.attempts = attempt.previous_attempts;
        assembly.chunks.clear();
        assembly.received_bytes = 0;
        assembly.complete_pending_validation = false;
        self.close_request_sequence(
            &attempt.holder,
            attempt.stream_epoch,
            attempt.semantic_sequence,
        );
        self.persist_lifecycle_state()
    }

    /// Register a block whose exact sidecar is missing and begin a bounded
    /// request to one holder selected by its QC bitmap.
    pub(crate) fn defer_block(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        reference: CertifiedMergeLedgerReference,
        requester: &PeerId,
        committed_height: u64,
        now: Instant,
    ) -> Result<Option<MergeSidecarPost>, MergeSidecarError> {
        self.defer_block_with_priority(
            block_hash,
            height,
            view,
            reference,
            requester,
            committed_height,
            now,
            InboundPriority::Ordinary,
        )
    }

    /// Register a decided carrier using capacity reserved from ordinary
    /// validation work, so unsigned same-hash reference variants cannot crowd
    /// the exact finality dependency out of global or per-holder limits.
    pub(crate) fn defer_decided_block(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        reference: CertifiedMergeLedgerReference,
        requester: &PeerId,
        committed_height: u64,
        now: Instant,
    ) -> Result<Option<MergeSidecarPost>, MergeSidecarError> {
        self.defer_block_with_priority(
            block_hash,
            height,
            view,
            reference,
            requester,
            committed_height,
            now,
            InboundPriority::Decided,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn defer_block_with_priority(
        &mut self,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        view: u64,
        reference: CertifiedMergeLedgerReference,
        requester: &PeerId,
        committed_height: u64,
        now: Instant,
        priority: InboundPriority,
    ) -> Result<Option<MergeSidecarPost>, MergeSidecarError> {
        Self::validate_reference_len(&reference)?;
        if height <= committed_height
            || height > committed_height.saturating_add(self.limits.future_block_distance)
        {
            return Err(MergeSidecarError::InvalidCarrierHeight);
        }
        let holders = certified_merge_sidecar_holders(&reference)?;
        let entry_hash = reference.entry_hash;
        let key = (entry_hash, certified_merge_reference_digest(&reference));
        let already_deferred = self
            .inbound
            .get(&key)
            .is_some_and(|assembly| assembly.deferred.contains_key(&block_hash));
        if !already_deferred
            && (self.deferred_count() >= self.limits.deferred_block_capacity
                || (priority == InboundPriority::Ordinary
                    && self.ordinary_deferred_count()
                        >= self.limits.deferred_block_capacity - RESERVED_DECIDED_DEFERRED_BLOCKS))
        {
            return Err(MergeSidecarError::Capacity("deferred block count"));
        }
        if !self.inbound.contains_key(&key) {
            if self.inbound.len() >= self.limits.inbound_session_capacity
                || (priority == InboundPriority::Ordinary
                    && self.ordinary_inbound_session_count()
                        >= self.limits.inbound_session_capacity - RESERVED_DECIDED_INBOUND_SESSIONS)
            {
                return Err(MergeSidecarError::Capacity("inbound session count"));
            }
            let requested_len = usize::try_from(reference.encoded_len).unwrap_or(usize::MAX);
            if self.inbound_reserved_bytes().saturating_add(requested_len)
                > self.limits.inbound_assembly_bytes
                || (priority == InboundPriority::Ordinary
                    && self
                        .ordinary_inbound_reserved_bytes()
                        .saturating_add(requested_len)
                        > self.limits.inbound_assembly_bytes - RESERVED_DECIDED_INBOUND_BYTES)
            {
                return Err(MergeSidecarError::Capacity("global inbound reservation"));
            }
            self.inbound.insert(
                key,
                InboundAssembly {
                    reference,
                    requester: requester.clone(),
                    priority,
                    holders,
                    holder_cursor: 0,
                    current: None,
                    chunks: Vec::new(),
                    received_bytes: 0,
                    attempts: 0,
                    deferred: BTreeMap::new(),
                    complete_pending_validation: false,
                },
            );
        } else if priority == InboundPriority::Decided {
            self.inbound
                .get_mut(&key)
                .expect("existing exact inbound assembly")
                .priority = InboundPriority::Decided;
        }
        self.inbound
            .get_mut(&key)
            .expect("inserted inbound assembly")
            .deferred
            .entry(block_hash)
            .or_insert(DeferredCarrier {
                hash: block_hash,
                height,
                view,
            });
        self.begin_request(key, requester, now)
    }

    fn validate_chunk_shape(
        chunk: &CertifiedMergeSidecarChunkV1,
    ) -> Result<(usize, usize), MergeSidecarError> {
        if chunk.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
            return Err(MergeSidecarError::UnsupportedVersion(chunk.version));
        }
        let total_len = usize::try_from(chunk.encoded_len)
            .map_err(|_| MergeSidecarError::InvalidEncodedLength(chunk.encoded_len))?;
        if total_len == 0 || total_len > MAX_MERGE_LEDGER_ENTRY_BYTES {
            return Err(MergeSidecarError::InvalidEncodedLength(chunk.encoded_len));
        }
        let expected_count = total_len.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        let chunk_count = usize::try_from(chunk.chunk_count)
            .map_err(|_| MergeSidecarError::InvalidChunk("chunk count exceeds usize"))?;
        if chunk_count == 0
            || chunk_count > MAX_CERTIFIED_MERGE_CHUNKS
            || chunk_count != expected_count
        {
            return Err(MergeSidecarError::InvalidChunk(
                "chunk count does not match exact encoded length",
            ));
        }
        let chunk_index = usize::try_from(chunk.chunk_index)
            .map_err(|_| MergeSidecarError::InvalidChunk("chunk index exceeds usize"))?;
        if chunk_index >= chunk_count {
            return Err(MergeSidecarError::InvalidChunk("chunk index out of bounds"));
        }
        let expected_chunk_len = if chunk_index + 1 == chunk_count {
            total_len - MAX_CERTIFIED_MERGE_CHUNK_BYTES * (chunk_count - 1)
        } else {
            MAX_CERTIFIED_MERGE_CHUNK_BYTES
        };
        if chunk.bytes.len() != expected_chunk_len {
            return Err(MergeSidecarError::InvalidChunk(
                "chunk payload does not match its fixed boundary",
            ));
        }
        Ok((chunk_count, chunk_index))
    }

    /// Accept a response only from the exact currently selected QC signer.
    pub(crate) fn ingest_chunk(
        &mut self,
        sender: &PeerId,
        chunk: CertifiedMergeSidecarChunkV1,
        now: Instant,
    ) -> Result<ChunkIngestOutcome, MergeSidecarError> {
        let (chunk_count, chunk_index) = Self::validate_chunk_shape(&chunk)?;
        if &chunk.responder != sender {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        let key = (chunk.entry_hash, chunk.reference_digest);
        let Some(snapshot) = self.inbound.get(&key) else {
            return Err(MergeSidecarError::UnsolicitedResponse);
        };
        let Some(attempt) = snapshot.current.as_ref() else {
            return Err(MergeSidecarError::UnsolicitedResponse);
        };
        if &attempt.holder != sender {
            return Err(MergeSidecarError::UnexpectedResponder);
        }
        if attempt.id != chunk.request_id {
            return Err(MergeSidecarError::RequestIdMismatch);
        }
        if attempt.service_generation != chunk.service_generation
            || attempt.stream_epoch != chunk.stream_epoch
            || attempt.semantic_sequence != chunk.semantic_sequence
        {
            return Err(MergeSidecarError::MetadataMismatch);
        }
        let reference = &snapshot.reference;
        if chunk.encoded_len != reference.encoded_len
            || chunk.epoch_id != reference.epoch_id
            || chunk.requester != snapshot.requester
        {
            return Err(MergeSidecarError::MetadataMismatch);
        }
        if snapshot.complete_pending_validation {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if !snapshot.chunks.is_empty() && snapshot.chunks.len() != chunk_count {
            return Err(MergeSidecarError::MetadataMismatch);
        }
        let new_global_bytes = self
            .inbound_received_bytes()
            .checked_add(chunk.bytes.len())
            .ok_or(MergeSidecarError::Capacity("inbound byte counter overflow"))?;
        if new_global_bytes > self.limits.inbound_assembly_bytes {
            return Err(MergeSidecarError::Capacity("global inbound bytes"));
        }
        let new_peer_bytes = self
            .inbound_peer_received_bytes(sender)
            .checked_add(chunk.bytes.len())
            .ok_or(MergeSidecarError::Capacity(
                "per-peer byte counter overflow",
            ))?;
        if new_peer_bytes > self.limits.inbound_assembly_bytes_per_peer {
            return Err(MergeSidecarError::Capacity("per-peer inbound bytes"));
        }
        let assembly = self
            .inbound
            .get_mut(&key)
            .expect("assembly was checked above");
        if assembly.chunks.is_empty() {
            assembly.chunks.resize_with(chunk_count, || None);
        }
        if assembly.chunks[chunk_index].is_some() {
            return Err(MergeSidecarError::DuplicateChunk(chunk.chunk_index));
        }
        assembly.received_bytes = assembly
            .received_bytes
            .checked_add(chunk.bytes.len())
            .ok_or(MergeSidecarError::Capacity("session byte counter overflow"))?;
        if assembly.received_bytes
            > usize::try_from(assembly.reference.encoded_len).unwrap_or(usize::MAX)
        {
            return Err(MergeSidecarError::Capacity("session inbound bytes"));
        }
        assembly.chunks[chunk_index] = Some(chunk.bytes);
        if let Some(attempt) = &mut assembly.current {
            attempt.last_progress_at = now;
        }
        if assembly.chunks.iter().any(Option::is_none) {
            return Ok(ChunkIngestOutcome::Accepted);
        }
        let expected_len = usize::try_from(assembly.reference.encoded_len)
            .map_err(|_| MergeSidecarError::InvalidEncodedLength(assembly.reference.encoded_len))?;
        let mut bytes = Vec::with_capacity(expected_len);
        for part in &assembly.chunks {
            bytes.extend_from_slice(part.as_deref().expect("all chunks checked present"));
        }
        if bytes.len() != expected_len {
            return Err(MergeSidecarError::LengthMismatch {
                expected: expected_len,
                actual: bytes.len(),
            });
        }
        assembly.complete_pending_validation = true;
        Ok(ChunkIngestOutcome::Complete(CompletedMergeSidecar {
            reference: assembly.reference.clone(),
            bytes,
        }))
    }

    /// Finish validation of a completed response. Success releases the exact
    /// deferred blocks; failure discards all partial bytes and rotates holders.
    pub(crate) fn finish_completed(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
        reference_digest: Hash,
        success: bool,
        requester: &PeerId,
        now: Instant,
    ) -> Result<
        (
            Vec<(HashOf<BlockHeader>, u64, u64)>,
            Option<MergeSidecarPost>,
        ),
        MergeSidecarError,
    > {
        let key = (entry_hash, reference_digest);
        if success {
            if self
                .inbound
                .get(&key)
                .is_some_and(|assembly| assembly.current.is_some())
            {
                self.preflight_lifecycle_mutation()?;
            }
            let deferred = self.inbound.remove(&key).map_or_else(Vec::new, |assembly| {
                if let Some(attempt) = &assembly.current {
                    self.close_request_sequence(
                        &attempt.holder,
                        attempt.stream_epoch,
                        attempt.semantic_sequence,
                    );
                }
                assembly
                    .deferred
                    .into_values()
                    .map(|carrier| (carrier.hash, carrier.height, carrier.view))
                    .collect()
            });
            self.persist_lifecycle_state()?;
            return Ok((deferred, None));
        }
        if self
            .inbound
            .get(&key)
            .is_some_and(|assembly| assembly.current.is_some())
        {
            self.preflight_lifecycle_mutation()?;
        }
        let closed = self.inbound.get_mut(&key).and_then(|assembly| {
            let closed = assembly.current.take().map(|attempt| {
                (
                    attempt.holder,
                    attempt.stream_epoch,
                    attempt.semantic_sequence,
                )
            });
            assembly.chunks.clear();
            assembly.received_bytes = 0;
            assembly.complete_pending_validation = false;
            closed
        });
        let closed_any = closed.is_some();
        if let Some((holder, stream_epoch, semantic_sequence)) = closed {
            self.close_request_sequence(&holder, stream_epoch, semantic_sequence);
            // Publish release of the failed occurrence before allocating a
            // replacement. Sequence/epoch exhaustion or holder-capacity
            // rejection must leave a restartable idle assembly, not an
            // unjournaled in-memory close.
            self.persist_lifecycle_state()?;
        }
        let request = self.begin_request(key, requester, now)?;
        if request.is_none() && !closed_any {
            self.persist_lifecycle_state()?;
        }
        Ok((Vec::new(), request))
    }

    /// Drop an invalid exact reference and return all affected carrier blocks.
    pub(crate) fn discard_invalid(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
    ) -> Result<Vec<(HashOf<BlockHeader>, u64, u64)>, MergeSidecarError> {
        let keys = self
            .inbound
            .keys()
            .filter(|key| key.0 == entry_hash)
            .copied()
            .collect::<Vec<_>>();
        if keys.iter().any(|key| {
            self.inbound
                .get(key)
                .is_some_and(|assembly| assembly.current.is_some())
        }) {
            self.preflight_lifecycle_mutation()?;
        }
        let mut affected = Vec::new();
        for key in keys {
            let Some(assembly) = self.inbound.remove(&key) else {
                continue;
            };
            if let Some(attempt) = &assembly.current {
                self.close_request_sequence(
                    &attempt.holder,
                    attempt.stream_epoch,
                    attempt.semantic_sequence,
                );
            }
            affected.extend(
                assembly
                    .deferred
                    .into_values()
                    .map(|carrier| (carrier.hash, carrier.height, carrier.view)),
            );
        }
        self.persist_lifecycle_state()?;
        Ok(affected)
    }

    fn outbound_attempt_has_writable_route(
        source: &ServerRequestSource,
        attempt: &OutboundAttempt,
    ) -> bool {
        match (source, attempt.reply_route.as_ref()) {
            (ServerRequestSource::Synthetic(_), None) => true,
            (ServerRequestSource::Authenticated(_), Some(route)) => route.is_reply_writable(),
            (
                ServerRequestSource::Synthetic(_) | ServerRequestSource::RecoveredAuthenticated(_),
                Some(_),
            )
            | (
                ServerRequestSource::Authenticated(_)
                | ServerRequestSource::RecoveredAuthenticated(_),
                None,
            ) => false,
        }
    }

    /// Release every inactive or reply-unwritable writer reservation without
    /// discarding its durable source cursor or current-chunk identity.
    ///
    /// The projected cursor state is persisted before ephemeral attempts and
    /// shared bytes are removed. A genuine late flush receipt may therefore
    /// still advance exactly once, while a later writable route can
    /// rematerialize the same pending chunk without regressing the cursor.
    ///
    /// `NetworkReplyRoute::is_active` deliberately remains true while inbound
    /// delivery guards drain after a writer timeout. Outbound ownership follows
    /// `is_reply_writable` instead, so unrelated inbound receivers cannot pin
    /// response bytes or a responder-generation transition.
    pub(crate) fn reclaim_inactive_outbound_attempts(
        &mut self,
        now: Instant,
    ) -> Result<usize, MergeSidecarError> {
        let unwritable = self
            .outbound
            .iter()
            .flat_map(|(key, transfer)| {
                transfer.attempts.iter().filter_map(|(source, attempt)| {
                    (!Self::outbound_attempt_has_writable_route(source, attempt)).then(|| {
                        (
                            key.clone(),
                            source.clone(),
                            attempt.in_flight_chunk.unwrap_or(attempt.next_chunk),
                        )
                    })
                })
            })
            .collect::<Vec<_>>();
        if unwritable.is_empty() {
            return Ok(0);
        }

        let mut projected = self.lifecycle_snapshot()?;
        for (key, source, resume_chunk) in &unwritable {
            let durable_gate = projected
                .payload
                .server_request_gates
                .iter_mut()
                .find(|gate| gate.requester == key.0 && gate.request_id == key.1)
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "unwritable outbound attempt lost its durable gate".to_owned(),
                    )
                })?;
            let durable_attempt = durable_gate
                .attempts
                .iter_mut()
                .find(|attempt| match (&attempt.source, source) {
                    (
                        DurableServerRequestSourceV3::Synthetic(durable),
                        ServerRequestSource::Synthetic(live),
                    ) => durable == live,
                    (
                        DurableServerRequestSourceV3::Authenticated(durable),
                        ServerRequestSource::Authenticated(live),
                    ) => durable == live.authenticated_source_peer(),
                    (
                        DurableServerRequestSourceV3::Authenticated(durable),
                        ServerRequestSource::RecoveredAuthenticated(live),
                    ) => durable == live,
                    (
                        DurableServerRequestSourceV3::Synthetic(_),
                        ServerRequestSource::Authenticated(_)
                        | ServerRequestSource::RecoveredAuthenticated(_),
                    )
                    | (
                        DurableServerRequestSourceV3::Authenticated(_),
                        ServerRequestSource::Synthetic(_),
                    ) => false,
                })
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "unwritable outbound attempt lost its durable source".to_owned(),
                    )
                })?;
            durable_attempt.cursor = DurableServerResponseCursorV3::Pending(
                u64::try_from(*resume_chunk).map_err(|_| {
                    MergeSidecarError::LifecycleJournal(
                        "unwritable outbound cursor is not representable".to_owned(),
                    )
                })?,
            );
        }
        projected.payload_hash = HashOf::new(&projected.payload);
        self.persist_lifecycle_projection(projected)?;

        let reclaimed = unwritable.len();
        let unwritable_keys = unwritable
            .iter()
            .map(|(key, source, _)| (key.clone(), source.clone()))
            .collect::<BTreeSet<_>>();
        for (key, source, resume_chunk) in unwritable {
            if let Some(gate_attempt) = self
                .server_request_gates
                .get_mut(&key)
                .and_then(|gate| gate.attempts.get_mut(&source))
            {
                gate_attempt.cursor = ServerResponseCursor::Pending(resume_chunk);
                gate_attempt.materialization_authorized = false;
                gate_attempt.authorized_materialization_route = None;
                gate_attempt.materialization_retryable = true;
                gate_attempt.inserted = now;
            }
            if let Some(transfer) = self.outbound.get_mut(&key) {
                transfer.attempts.remove(&source);
            }
        }
        self.outbound
            .retain(|_, transfer| !transfer.attempts.is_empty());
        self.outbound_order
            .retain(|attempt| !unwritable_keys.contains(attempt));
        Ok(reclaimed)
    }

    fn prune_server_gates(&mut self, now: Instant) -> Result<usize, MergeSidecarError> {
        let reclaimed = self.reclaim_inactive_outbound_attempts(now)?;
        for gate in self.server_request_gates.values_mut() {
            for attempt in gate.attempts.values_mut().filter(|attempt| {
                attempt.materialization_authorized
                    && attempt
                        .authorized_materialization_route
                        .as_ref()
                        .is_some_and(|route| !route.is_reply_writable())
            }) {
                attempt.materialization_authorized = false;
                attempt.authorized_materialization_route = None;
                attempt.materialization_retryable =
                    matches!(attempt.cursor, ServerResponseCursor::Pending(_));
                attempt.inserted = now;
            }
        }
        // Semantic ownership has no wall-clock expiry. A completed source
        // remains terminal until the authenticated requester advances its
        // cumulative close floor; elapsed time must never reset its cursor.
        Ok(reclaimed)
    }

    fn generation_hint_post(
        &self,
        requester: &PeerId,
        responder: &PeerId,
        reply_route: &NetworkReplyRoute,
        observed_generation: CertifiedMergeSidecarServiceGenerationV1,
        observed_message_hash: Hash,
    ) -> MergeSidecarPost {
        let mut hint = CertifiedMergeSidecarGenerationHintV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            observed_generation,
            current_generation: self.server_service_generation,
            observed_message_hash,
            hint_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        hint.bind_canonical_hint_id();
        MergeSidecarPost {
            peer: requester.clone(),
            reply_route: Some(reply_route.clone()),
            message: Arc::new(CertifiedMergeSidecarMessage::GenerationHint(hint)),
        }
    }

    fn preflight_server_request_stream(
        &self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) -> Result<(), MergeSidecarError> {
        if request.closed_through >= request.semantic_sequence.get()
            || request.request_id != request.canonical_request_id()
        {
            return Err(MergeSidecarError::RequestIdMismatch);
        }
        if let Some(stream) = self.server_streams.get(sender) {
            if request.stream_epoch < stream.stream_epoch
                || (request.stream_epoch == stream.stream_epoch
                    && request.closed_through < stream.closed_through)
            {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
        }
        let forward_window = u64::try_from(self.limits.inbound_sessions_per_peer)
            .map_err(|_| MergeSidecarError::Capacity("semantic request forward window"))?;
        let window_end = request.closed_through.checked_add(forward_window).ok_or(
            MergeSidecarError::Capacity("semantic request forward window"),
        )?;
        if request.semantic_sequence.get() > window_end {
            return Err(MergeSidecarError::Capacity(
                "semantic request forward window",
            ));
        }
        if self.server_request_gates.iter().any(|(key, gate)| {
            &key.0 == sender
                && gate.service_generation == request.service_generation
                && gate.stream_epoch == request.stream_epoch
                && gate.semantic_sequence == request.semantic_sequence
                && (key.1 != request.request_id
                    || !gate.request.same_occurrence_except_close_floor(request)
                    || request.closed_through < gate.request.closed_through)
        }) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        Ok(())
    }

    fn record_server_closure(
        &mut self,
        requester: &PeerId,
        service_generation: CertifiedMergeSidecarServiceGenerationV1,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        closed_through: u64,
    ) {
        if closed_through == 0 {
            return;
        }
        let candidate = CertifiedMergeSidecarClosedPrefix {
            requester: requester.clone(),
            service_generation,
            stream_epoch,
            closed_through,
        };
        match self.pending_server_closures.get_mut(requester) {
            Some(retained) if candidate.covers(retained) => *retained = candidate,
            Some(retained) if retained.covers(&candidate) => {}
            Some(retained) => {
                debug_assert_eq!(
                    retained.service_generation, candidate.service_generation,
                    "service generations are totally ordered"
                );
                debug_assert_eq!(
                    retained.stream_epoch, candidate.stream_epoch,
                    "stream epochs are totally ordered"
                );
                retained.closed_through = retained.closed_through.max(candidate.closed_through);
            }
            None => {
                self.pending_server_closures
                    .insert(requester.clone(), candidate);
            }
        }
    }

    fn server_generation_is_terminal(&self) -> bool {
        self.server_streams
            .values()
            .all(|stream| stream.closed_through == stream.highest_sequence)
            && self.server_request_gates.is_empty()
            && self.outbound.is_empty()
            && self.outbound_order.is_empty()
            && self.pending_server_closures.is_empty()
            && !self.server_closure_handoff_pending
    }

    fn transition_server_service_generation(
        &mut self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<(), MergeSidecarError> {
        let plan = self.prepare_server_service_generation_transition(
            server_stream_capacity,
            server_roster_digest,
        )?;
        if !self.server_generation_is_terminal() {
            return Err(MergeSidecarError::Capacity(
                "server semantic requester geometry",
            ));
        }
        self.commit_server_service_generation_transition(
            plan,
            ServerServiceGenerationRetirement::AuthenticatedTerminal,
        )
    }

    fn transition_server_service_generation_after_durable_handoff(
        &mut self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
        authority: DurableMergeSidecarRolloverAuthority,
    ) -> Result<(), MergeSidecarError> {
        drop(authority);
        self.transition_server_service_generation_after_exact_output_fence(
            server_stream_capacity,
            server_roster_digest,
        )
    }

    fn transition_server_service_generation_after_exact_output_fence(
        &mut self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<(), MergeSidecarError> {
        if self.lifecycle_journal.is_none() {
            return Err(MergeSidecarError::LifecycleJournal(
                "authority-gated responder rollover requires a durable lifecycle journal"
                    .to_owned(),
            ));
        }
        let plan = self.prepare_server_service_generation_transition(
            server_stream_capacity,
            server_roster_digest,
        )?;
        self.commit_server_service_generation_transition(
            plan,
            ServerServiceGenerationRetirement::ExactOutputSuperseded,
        )
    }

    fn prepare_server_service_generation_transition(
        &self,
        server_stream_capacity: usize,
        server_roster_digest: MergeSidecarRosterDigest,
    ) -> Result<ServerServiceGenerationTransitionPlan, MergeSidecarError> {
        if server_roster_digest == self.server_roster_digest {
            return Err(MergeSidecarError::Capacity(
                "server service generation requires a changed roster identity",
            ));
        }
        let (server_request_gate_capacity, server_request_attempt_capacity) =
            Self::derive_server_request_capacities(
                self.reply_source_capacity,
                self.limits,
                server_stream_capacity,
            )?;
        let next_geometry = self.lifecycle_geometry_for_server_roster(
            server_stream_capacity,
            server_roster_digest.clone(),
        )?;
        let next = self
            .server_service_generation
            .get()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(CertifiedMergeSidecarServiceGenerationV1)
            .ok_or(MergeSidecarError::Capacity(
                "server service generation exhausted",
            ))?;
        Ok(ServerServiceGenerationTransitionPlan {
            server_stream_capacity,
            server_roster_digest,
            server_request_gate_capacity,
            server_request_attempt_capacity,
            next_geometry,
            next_generation: next,
        })
    }

    fn commit_server_service_generation_transition(
        &mut self,
        plan: ServerServiceGenerationTransitionPlan,
        retirement: ServerServiceGenerationRetirement,
    ) -> Result<(), MergeSidecarError> {
        // Publish the successor generation, geometry, and empty responder
        // tables in the root-anchored V3 snapshot before changing memory or emitting a
        // generation hint. A crash observes either the complete predecessor or
        // the complete successor state.
        if self.lifecycle_journal.is_some() {
            let mut snapshot = self.lifecycle_snapshot()?;
            snapshot.payload.geometry = plan.next_geometry.clone();
            snapshot.payload.server_service_generation = plan.next_generation;
            snapshot.payload.materialization_requester_cursor = None;
            snapshot.payload.server_streams.clear();
            snapshot.payload.server_request_gates.clear();
            snapshot.payload_hash = HashOf::new(&snapshot.payload);
            self.persist_lifecycle_projection(snapshot)?;
        }

        match retirement {
            ServerServiceGenerationRetirement::AuthenticatedTerminal => {
                let retired = self
                    .server_streams
                    .iter()
                    .map(|(requester, stream)| (requester.clone(), *stream))
                    .collect::<Vec<_>>();
                for (requester, stream) in retired {
                    debug_assert_eq!(
                        stream.closed_through, stream.highest_sequence,
                        "ordinary responder rollover requires authenticated terminality"
                    );
                    self.record_server_closure(
                        &requester,
                        self.server_service_generation,
                        stream.stream_epoch,
                        stream.closed_through,
                    );
                }
            }
            ServerServiceGenerationRetirement::ExactOutputSuperseded => {
                // The generation fence invalidates active predecessor requests;
                // it must not manufacture requester-authenticated close
                // prefixes for semantic sequences that were never closed.
                self.pending_server_closures.clear();
                self.server_closure_handoff_pending = false;
            }
        }
        self.server_service_generation = plan.next_generation;
        self.server_roster_digest = plan.server_roster_digest;
        self.server_stream_capacity = plan.server_stream_capacity;
        self.server_request_gate_capacity = plan.server_request_gate_capacity;
        self.server_request_attempt_capacity = plan.server_request_attempt_capacity;
        self.server_streams.clear();
        self.materialization_requester_cursor = None;
        self.server_request_gates.clear();
        self.outbound.clear();
        self.outbound_order.clear();
        Ok(())
    }

    /// Ensure that requester admission stays inside the immutable roster bound.
    ///
    /// Exhaustion rejects locally even when every retained stream is terminal.
    /// Only a certified changed-roster geometry replacement may advance the
    /// responder generation and clear the prior table.
    fn ensure_server_stream_slot(&self, sender: &PeerId) -> Result<(), MergeSidecarError> {
        if self.server_streams.contains_key(sender) {
            return Ok(());
        }
        if self.server_streams.len() < self.server_stream_capacity {
            return Ok(());
        }
        Err(MergeSidecarError::Capacity(
            "server semantic requester geometry",
        ))
    }

    /// Return whether the current responder generation already owns this
    /// requester's bounded semantic stream.
    ///
    /// Callers may use this to authenticate a cumulative close from a peer
    /// whose historical request was admitted after a roster change.  Merely
    /// naming an earlier generation or requester never creates authority.
    pub(crate) fn owns_current_server_stream(
        &self,
        requester: &PeerId,
        service_generation: CertifiedMergeSidecarServiceGenerationV1,
    ) -> bool {
        service_generation == self.server_service_generation
            && self.server_streams.contains_key(requester)
    }

    /// Return whether a current-generation request would create a new
    /// responder stream.
    ///
    /// Lower-generation requests are stateless generation probes and future
    /// generations fail before allocation, so neither belongs to a reserved
    /// identity corridor.
    pub(crate) fn would_allocate_current_server_stream(
        &self,
        requester: &PeerId,
        service_generation: CertifiedMergeSidecarServiceGenerationV1,
    ) -> bool {
        service_generation == self.server_service_generation
            && !self.server_streams.contains_key(requester)
    }

    /// Count responder identities selected by an allocation-free classifier.
    ///
    /// The adapter uses this allocation-free projection to preserve every
    /// current-roster slot while admitting a bounded predecessor committee.
    pub(crate) fn server_stream_count_matching(
        &self,
        mut predicate: impl FnMut(&PeerId) -> bool,
    ) -> usize {
        self.server_streams
            .keys()
            .filter(|requester| predicate(requester))
            .count()
    }

    fn supersede_server_stream(
        &mut self,
        sender: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    ) {
        let Some(prior) = self.server_streams.get(sender).copied() else {
            self.server_streams.insert(
                sender.clone(),
                ServerStreamState {
                    stream_epoch,
                    closed_through: 0,
                    highest_sequence: 0,
                },
            );
            return;
        };
        debug_assert!(stream_epoch > prior.stream_epoch);
        let retired = self
            .server_request_gates
            .iter()
            .filter(|(key, gate)| &key.0 == sender && gate.stream_epoch == prior.stream_epoch)
            .map(|(key, _)| key.clone())
            .collect::<BTreeSet<_>>();
        for key in &retired {
            self.server_request_gates.remove(key);
            self.outbound.remove(key);
        }
        if !retired.is_empty() {
            self.outbound_order
                .retain(|(key, _)| !retired.contains(key));
        }
        if prior.highest_sequence > 0 {
            self.record_server_closure(
                sender,
                self.server_service_generation,
                prior.stream_epoch,
                prior.highest_sequence,
            );
        }
        self.server_streams.insert(
            sender.clone(),
            ServerStreamState {
                stream_epoch,
                closed_through: 0,
                highest_sequence: 0,
            },
        );
    }

    fn advance_server_close_floor(
        &mut self,
        sender: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        closed_through: u64,
    ) {
        let prior = self
            .server_streams
            .get(sender)
            .filter(|stream| stream.stream_epoch == stream_epoch)
            .map_or(0, |stream| stream.closed_through);
        if closed_through == prior {
            return;
        }
        debug_assert!(closed_through > prior);
        let retired = self
            .server_request_gates
            .iter()
            .filter(|(key, gate)| {
                &key.0 == sender
                    && gate.stream_epoch == stream_epoch
                    && gate.semantic_sequence.get() <= closed_through
            })
            .map(|(key, _)| key.clone())
            .collect::<BTreeSet<_>>();
        for key in &retired {
            self.server_request_gates.remove(key);
            self.outbound.remove(key);
        }
        if !retired.is_empty() {
            self.outbound_order
                .retain(|(key, _)| !retired.contains(key));
        }
        self.server_streams
            .get_mut(sender)
            .expect("server stream exists while advancing its close floor")
            .closed_through = closed_through;
        self.record_server_closure(
            sender,
            self.server_service_generation,
            stream_epoch,
            closed_through,
        );
    }

    fn advance_piggybacked_close_floor(
        &mut self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) -> Result<bool, MergeSidecarError> {
        let key = (sender.clone(), request.request_id);
        let Some(retained_request) = self
            .server_request_gates
            .get(&key)
            .map(|gate| gate.request.clone())
        else {
            return Ok(false);
        };
        if !retained_request.same_occurrence_except_close_floor(request)
            || request.closed_through < retained_request.closed_through
        {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if request.closed_through == retained_request.closed_through {
            return Ok(false);
        }

        // Publish the advanced floor and latest whole-message hash before
        // mutating live gates, transfers, or cancellation output. Pending
        // chunk identities remain valid because the cumulative floor is the
        // sole request field deliberately excluded from occurrence identity.
        if self.lifecycle_journal.is_some() {
            let mut projected = self.lifecycle_snapshot()?;
            let stream = projected
                .payload
                .server_streams
                .iter_mut()
                .find(|stream| {
                    stream.requester == *sender
                        && stream.service_generation == request.service_generation
                        && stream.stream_epoch == request.stream_epoch
                })
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "piggybacked close floor lost its durable server stream".to_owned(),
                    )
                })?;
            if request.closed_through < stream.closed_through
                || request.closed_through >= request.semantic_sequence.get()
            {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            stream.closed_through = request.closed_through;
            projected.payload.server_request_gates.retain(|gate| {
                gate.requester != *sender
                    || gate.service_generation != request.service_generation
                    || gate.stream_epoch != request.stream_epoch
                    || gate.semantic_sequence.get() > request.closed_through
            });
            let retained = projected
                .payload
                .server_request_gates
                .iter_mut()
                .find(|gate| gate.requester == *sender && gate.request_id == request.request_id)
                .ok_or_else(|| {
                    MergeSidecarError::LifecycleJournal(
                        "piggybacked close floor lost its durable request gate".to_owned(),
                    )
                })?;
            if !retained.request.same_occurrence_except_close_floor(request) {
                return Err(MergeSidecarError::LifecycleJournal(
                    "piggybacked close floor changed its durable occurrence".to_owned(),
                ));
            }
            retained.request = request.clone();
            retained.request_hash = HashOf::new(request);
            projected.payload_hash = HashOf::new(&projected.payload);
            self.persist_lifecycle_projection(projected)?;
        }

        self.advance_server_close_floor(sender, request.stream_epoch, request.closed_through);
        let gate = self
            .server_request_gates
            .get_mut(&key)
            .expect("the current occurrence lies above its piggybacked close floor");
        gate.request = request.clone();
        gate.request_hash = HashOf::new(request);
        if let Some(transfer) = self.outbound.get_mut(&key) {
            transfer.request = request.clone();
        }
        Ok(true)
    }

    /// Apply an authenticated standalone close and return its exact-route ACK.
    ///
    /// A close for an unknown requester is acknowledged statelessly. It cannot
    /// consume a roster-bounded stream slot merely to retain a tombstone for
    /// traffic the responder never admitted.
    pub(crate) fn admit_server_close(
        &mut self,
        sender: &PeerId,
        close: &CertifiedMergeSidecarCloseV1,
        reply_route: Option<&NetworkReplyRoute>,
        local_peer: &PeerId,
    ) -> Result<MergeSidecarPost, MergeSidecarError> {
        if close.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
            return Err(MergeSidecarError::UnsupportedVersion(close.version));
        }
        if &close.requester != sender || &close.responder != local_peer {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if reply_route.is_some_and(|route| route.semantic_target() != sender) {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if reply_route.is_some_and(|route| !route.is_active()) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if close.closed_through == 0 || close.close_id != close.canonical_close_id() {
            return Err(MergeSidecarError::CloseIdMismatch);
        }
        let observed_message_hash = HashOf::new(close).into();
        if close.service_generation > self.server_service_generation {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if close.service_generation < self.server_service_generation {
            let reply_route = reply_route.ok_or(MergeSidecarError::UnsolicitedResponse)?;
            return Ok(self.generation_hint_post(
                sender,
                local_peer,
                reply_route,
                close.service_generation,
                observed_message_hash,
            ));
        }
        let close_ack = || MergeSidecarPost {
            peer: sender.clone(),
            reply_route: reply_route.cloned(),
            message: Arc::new(CertifiedMergeSidecarMessage::CloseAck(
                CertifiedMergeSidecarCloseAckV1 {
                    version: close.version,
                    service_generation: close.service_generation,
                    stream_epoch: close.stream_epoch,
                    closed_through: close.closed_through,
                    close_id: close.close_id,
                    requester: close.requester.clone(),
                    responder: close.responder.clone(),
                },
            )),
        };
        if !self.server_streams.contains_key(sender) {
            return Ok(close_ack());
        }
        let stream = self
            .server_streams
            .get(sender)
            .copied()
            .expect("the checked server stream remains present");
        let will_change = if close.stream_epoch < stream.stream_epoch {
            return Err(MergeSidecarError::UnsolicitedResponse);
        } else if close.stream_epoch == stream.stream_epoch {
            if close.closed_through < stream.closed_through {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            close.closed_through > stream.highest_sequence
                || close.closed_through > stream.closed_through
        } else {
            true
        };
        if !will_change {
            return Ok(close_ack());
        }
        self.preflight_lifecycle_mutation()?;
        let mut changed = false;
        match self.server_streams.get(sender).copied() {
            Some(stream) if close.stream_epoch == stream.stream_epoch => {
                if close.closed_through > stream.highest_sequence {
                    self.server_streams
                        .get_mut(sender)
                        .expect("equal-epoch server stream remains installed")
                        .highest_sequence = close.closed_through;
                    changed = true;
                }
            }
            Some(_) => {
                self.supersede_server_stream(sender, close.stream_epoch);
                self.server_streams
                    .get_mut(sender)
                    .expect("new server stream was installed")
                    .highest_sequence = close.closed_through;
                changed = true;
            }
            None => unreachable!("unknown server close returned without allocating state"),
        }
        let prior = self
            .server_streams
            .get(sender)
            .expect("validated server stream exists")
            .closed_through;
        if close.closed_through > prior {
            self.advance_server_close_floor(sender, close.stream_epoch, close.closed_through);
            changed = true;
        }
        if changed {
            self.persist_lifecycle_state()?;
        }
        debug_assert!(
            changed,
            "the immutable preflight established close progress"
        );
        Ok(close_ack())
    }

    /// Drain coalesced server prefixes so every downstream queue can cancel
    /// covered response chunks before dispatching newer work.
    ///
    /// Responder-generation rollover stays blocked until the downstream owner
    /// applies the entire drained batch and calls
    /// [`Self::confirm_closed_server_prefix_handoff`].
    pub(crate) fn drain_closed_server_prefixes(
        &mut self,
    ) -> Vec<CertifiedMergeSidecarClosedPrefix> {
        let prefixes = std::mem::take(&mut self.pending_server_closures)
            .into_iter()
            .map(|(_, prefix)| prefix)
            .collect::<Vec<_>>();
        self.server_closure_handoff_pending |= !prefixes.is_empty();
        prefixes
    }

    /// Confirm that every previously drained close prefix reached the
    /// process-local exact-output owner.
    ///
    /// A new closure recorded after the drain keeps rollover blocked until it
    /// is drained and confirmed by a later call.
    pub(crate) fn confirm_closed_server_prefix_handoff(&mut self) {
        if self.pending_server_closures.is_empty() {
            self.server_closure_handoff_pending = false;
        }
    }

    fn server_request_source(
        sender: &PeerId,
        reply_route: Option<&NetworkReplyRoute>,
    ) -> ServerRequestSource {
        reply_route.map_or_else(
            || ServerRequestSource::Synthetic(sender.clone()),
            |route| ServerRequestSource::Authenticated(route.source_key()),
        )
    }

    fn source_gate_count(&self, source: &ServerRequestSource) -> usize {
        self.server_request_gates
            .values()
            .filter(|gate| {
                gate.attempts
                    .keys()
                    .any(|retained| retained.shares_budget_with(source))
            })
            .count()
    }

    fn server_gate_attempt_count(&self) -> usize {
        self.server_request_gates
            .values()
            .map(|gate| gate.attempts.len())
            .sum()
    }

    #[cfg(test)]
    /// Return the retained responder-stream count for cross-layer tests.
    pub(crate) fn server_stream_count_for_test(&self) -> usize {
        self.server_streams.len()
    }

    #[cfg(test)]
    /// Return the retained unique logical request-gate count.
    pub(crate) fn server_request_gate_count_for_test(&self) -> usize {
        self.server_request_gates.len()
    }

    #[cfg(test)]
    /// Return the exact responder generation for cross-layer rollover tests.
    pub(crate) const fn server_service_generation_for_test(
        &self,
    ) -> CertifiedMergeSidecarServiceGenerationV1 {
        self.server_service_generation
    }

    #[cfg(test)]
    /// Borrow the exact responder-roster identity for cross-layer rollover tests.
    pub(crate) fn server_roster_digest_for_test(&self) -> &MergeSidecarRosterDigest {
        &self.server_roster_digest
    }

    #[cfg(test)]
    /// Return the retained authenticated/synthetic attempt count.
    pub(crate) fn server_request_attempt_count_for_test(&self) -> usize {
        self.server_gate_attempt_count()
    }

    #[cfg(test)]
    /// Return the retained ephemeral outbound-attempt count.
    pub(crate) fn retained_outbound_attempt_count_for_test(&self) -> usize {
        self.outbound_attempt_count()
    }

    #[cfg(test)]
    /// Return the shared immutable outbound payload bytes currently retained.
    pub(crate) fn retained_outbound_bytes_for_test(&self) -> usize {
        self.global_outbound_bytes()
    }

    fn server_gate_attempt_count_after_close(
        &self,
        sender: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        closed_through: u64,
    ) -> usize {
        self.server_request_gates
            .iter()
            .filter(|(key, gate)| {
                &key.0 != sender
                    || gate.stream_epoch > stream_epoch
                    || (gate.stream_epoch == stream_epoch
                        && gate.semantic_sequence.get() > closed_through)
            })
            .map(|(_, gate)| gate.attempts.len())
            .sum()
    }

    fn server_gate_count_after_close(
        &self,
        sender: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        closed_through: u64,
    ) -> usize {
        self.server_request_gates
            .iter()
            .filter(|(key, gate)| {
                &key.0 != sender
                    || gate.stream_epoch > stream_epoch
                    || (gate.stream_epoch == stream_epoch
                        && gate.semantic_sequence.get() > closed_through)
            })
            .count()
    }

    fn source_gate_count_after_close(
        &self,
        source: &ServerRequestSource,
        sender: &PeerId,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1,
        closed_through: u64,
    ) -> usize {
        self.server_request_gates
            .iter()
            .filter(|(key, gate)| {
                (&key.0 != sender
                    || gate.stream_epoch > stream_epoch
                    || (gate.stream_epoch == stream_epoch
                        && gate.semantic_sequence.get() > closed_through))
                    && gate
                        .attempts
                        .keys()
                        .any(|retained| retained.shares_budget_with(source))
            })
            .count()
    }

    fn outbound_attempt_count(&self) -> usize {
        self.outbound
            .values()
            .map(|transfer| transfer.attempts.len())
            .sum()
    }

    fn source_outbound_count(&self, source: &ServerRequestSource) -> usize {
        self.outbound
            .values()
            .filter(|transfer| {
                transfer
                    .attempts
                    .keys()
                    .any(|retained| retained.shares_budget_with(source))
            })
            .count()
    }

    fn global_outbound_bytes(&self) -> usize {
        self.outbound
            .values()
            .map(|transfer| transfer.response_len)
            .sum()
    }

    fn source_outbound_bytes(&self, source: &ServerRequestSource) -> usize {
        self.outbound
            .values()
            .filter(|transfer| {
                transfer
                    .attempts
                    .keys()
                    .any(|retained| retained.shares_budget_with(source))
            })
            .map(|transfer| transfer.response_len)
            .sum()
    }

    fn route_update(
        candidate: Option<&NetworkReplyRoute>,
        prior: Option<&NetworkReplyRoute>,
    ) -> Result<NetworkReplyRouteSourceUpdate, MergeSidecarError> {
        match (candidate, prior) {
            (None, None) => Ok(NetworkReplyRouteSourceUpdate::Exact),
            (Some(candidate), Some(prior)) => candidate
                .source_update_from(prior)
                .map_err(|_| MergeSidecarError::UnsolicitedResponse),
            (None, Some(_)) | (Some(_), None) => Err(MergeSidecarError::UnsolicitedResponse),
        }
    }

    fn alternate_source_is_authorized(
        gate: &ServerRequestGate,
        candidate: Option<&NetworkReplyRoute>,
    ) -> bool {
        match candidate {
            Some(candidate) => gate.attempts.iter().all(|(source, attempt)| {
                attempt.reply_route.as_ref().map_or_else(
                    || matches!(source, ServerRequestSource::RecoveredAuthenticated(_)),
                    |prior| {
                        candidate.same_request_authority(prior)
                            && !candidate.equal_ordinal_different_tenure(prior)
                            && !candidate.equal_connection_ordinal_different_tenure(prior)
                    },
                )
            }),
            None => gate.attempts.iter().all(|(source, attempt)| {
                matches!(source, ServerRequestSource::Synthetic(_)) && attempt.reply_route.is_none()
            }),
        }
    }

    fn route_source_capacity(
        reply_route: Option<&NetworkReplyRoute>,
    ) -> Result<Option<usize>, MergeSidecarError> {
        reply_route
            .map(|route| {
                NetworkReplyRoutes::try_from_route(route.clone())
                    .map(|routes| routes.source_capacity())
                    .map_err(|_| MergeSidecarError::UnsolicitedResponse)
            })
            .transpose()
    }

    fn can_add_outbound_attempt(&self, source: &ServerRequestSource, bytes: usize) -> bool {
        self.outbound_attempt_count() < self.outbound_session_capacity
            && self.source_outbound_count(source) < self.limits.outbound_sessions_per_source
            && self.source_outbound_bytes(source).saturating_add(bytes)
                <= self.limits.outbound_bytes_per_source
    }

    fn attempt_has_writable_materialization_route(
        source: &ServerRequestSource,
        attempt: &ServerRequestGateAttempt,
    ) -> bool {
        match (source, attempt.reply_route.as_ref()) {
            (ServerRequestSource::Synthetic(_), None) => true,
            (ServerRequestSource::Authenticated(_), Some(route)) => route.is_reply_writable(),
            (
                ServerRequestSource::Synthetic(_) | ServerRequestSource::RecoveredAuthenticated(_),
                Some(_),
            )
            | (
                ServerRequestSource::Authenticated(_)
                | ServerRequestSource::RecoveredAuthenticated(_),
                None,
            ) => false,
        }
    }

    fn attempt_has_writable_authorized_materialization_route(
        source: &ServerRequestSource,
        attempt: &ServerRequestGateAttempt,
    ) -> bool {
        match (
            source,
            attempt.reply_route.as_ref(),
            attempt.authorized_materialization_route.as_ref(),
        ) {
            (ServerRequestSource::Synthetic(_), None, None) => true,
            (ServerRequestSource::Authenticated(_), Some(current), Some(authorized)) => {
                current.same_delivery(authorized) && authorized.is_reply_writable()
            }
            (
                ServerRequestSource::Synthetic(_) | ServerRequestSource::RecoveredAuthenticated(_),
                _,
                _,
            )
            | (ServerRequestSource::Authenticated(_), None, _)
            | (ServerRequestSource::Authenticated(_), Some(_), None) => false,
        }
    }

    fn gate_has_materialization_capacity(&self, gate: &ServerRequestGate) -> bool {
        let Ok(response_len) = usize::try_from(gate.request.encoded_len) else {
            return false;
        };
        response_len > 0
            && response_len <= MAX_MERGE_LEDGER_ENTRY_BYTES
            && self.global_outbound_bytes().saturating_add(response_len)
                <= self.outbound_byte_capacity
            && gate.attempts.iter().any(|(source, attempt)| {
                matches!(attempt.cursor, ServerResponseCursor::Pending(_))
                    && attempt.materialization_retryable
                    && Self::attempt_has_writable_materialization_route(source, attempt)
                    && self.can_add_outbound_attempt(source, response_len)
            })
    }

    fn authorized_server_request_materialization(&self) -> Option<ServerRequestMaterialization> {
        self.server_request_gates
            .iter()
            .filter_map(|((requester, _), gate)| {
                gate.attempts
                    .iter()
                    .find(|(source, attempt)| {
                        attempt.materialization_authorized
                            && Self::attempt_has_writable_authorized_materialization_route(
                                source, attempt,
                            )
                    })
                    .map(|(_, attempt)| ServerRequestMaterialization {
                        requester: requester.clone(),
                        request: gate.request.clone(),
                        reply_route: attempt.authorized_materialization_route.clone(),
                    })
            })
            .min_by(|left, right| {
                (
                    &left.requester,
                    left.request.stream_epoch,
                    left.request.semantic_sequence,
                    left.request.request_id,
                )
                    .cmp(&(
                        &right.requester,
                        right.request.stream_epoch,
                        right.request.semantic_sequence,
                        right.request.request_id,
                    ))
            })
    }

    /// Select one retryable response lookup with bounded two-level fairness.
    ///
    /// An already-authorized lookup is returned idempotently. Otherwise the
    /// first level advances strictly after the durable requester cursor and
    /// wraps; the second chooses that requester's lowest
    /// `(stream_epoch, semantic_sequence, request_id)` gate. The cursor is
    /// persisted before any attempt receives terminating lookup authority.
    pub(crate) fn next_server_request_materialization(
        &mut self,
        now: Instant,
    ) -> Result<Option<ServerRequestMaterialization>, MergeSidecarError> {
        self.prune_server_gates(now)?;
        if let Some(materialization) = self.authorized_server_request_materialization() {
            return Ok(Some(materialization));
        }

        let eligible_requesters = self
            .server_request_gates
            .iter()
            .filter(|(key, gate)| {
                !self.outbound.contains_key(*key)
                    && self.gate_has_materialization_capacity(gate)
                    && gate.attempts.iter().any(|(source, attempt)| {
                        matches!(attempt.cursor, ServerResponseCursor::Pending(_))
                            && attempt.materialization_retryable
                            && Self::attempt_has_writable_materialization_route(source, attempt)
                    })
            })
            .map(|(key, _)| key.0.clone())
            .collect::<BTreeSet<_>>();
        let Some(requester) = self
            .materialization_requester_cursor
            .as_ref()
            .and_then(|cursor| {
                eligible_requesters
                    .iter()
                    .find(|candidate| *candidate > cursor)
                    .cloned()
            })
            .or_else(|| eligible_requesters.first().cloned())
        else {
            return Ok(None);
        };

        let (key, selected_source, selected_route, request) = self
            .server_request_gates
            .iter()
            .filter(|(key, gate)| {
                key.0 == requester
                    && !self.outbound.contains_key(*key)
                    && self.gate_has_materialization_capacity(gate)
                    && gate.attempts.iter().any(|(source, attempt)| {
                        matches!(attempt.cursor, ServerResponseCursor::Pending(_))
                            && attempt.materialization_retryable
                            && Self::attempt_has_writable_materialization_route(source, attempt)
                    })
            })
            .min_by_key(|(key, gate)| (gate.stream_epoch, gate.semantic_sequence, key.1))
            .and_then(|(key, gate)| {
                gate.attempts
                    .iter()
                    .find(|(source, attempt)| {
                        matches!(attempt.cursor, ServerResponseCursor::Pending(_))
                            && attempt.materialization_retryable
                            && Self::attempt_has_writable_materialization_route(source, attempt)
                    })
                    .map(|(source, attempt)| {
                        (
                            key.clone(),
                            source.clone(),
                            attempt.reply_route.clone(),
                            gate.request.clone(),
                        )
                    })
            })
            .expect("eligible requester has a lowest eligible request attempt");

        let mut projected = self.lifecycle_snapshot()?;
        projected.payload.materialization_requester_cursor = Some(requester.clone());
        projected.payload_hash = HashOf::new(&projected.payload);
        self.persist_lifecycle_projection(projected)?;

        self.materialization_requester_cursor = Some(requester.clone());
        let gate = self
            .server_request_gates
            .get_mut(&key)
            .expect("selected materialization gate remains present");
        for (source, attempt) in &mut gate.attempts {
            if matches!(attempt.cursor, ServerResponseCursor::Pending(_))
                && attempt.materialization_retryable
                && Self::attempt_has_writable_materialization_route(source, attempt)
            {
                attempt.materialization_authorized = true;
                attempt.authorized_materialization_route = attempt.reply_route.clone();
                attempt.materialization_retryable = false;
                attempt.inserted = now;
            }
        }
        debug_assert!(
            gate.attempts
                .get(&selected_source)
                .is_some_and(|attempt| attempt.materialization_authorized)
        );
        Ok(Some(ServerRequestMaterialization {
            requester,
            request,
            reply_route: selected_route,
        }))
    }

    fn admission_after_fair_materialization_selection(
        &mut self,
        request: &CertifiedMergeSidecarRequestV1,
        reply_route: Option<&NetworkReplyRoute>,
        now: Instant,
    ) -> Result<ServerRequestAdmission, MergeSidecarError> {
        Ok(
            if self
                .next_server_request_materialization(now)?
                .is_some_and(|selected| {
                    let same_route = match (selected.reply_route.as_ref(), reply_route) {
                        (None, None) => true,
                        (Some(selected), Some(candidate)) => selected.same_delivery(candidate),
                        (None, Some(_)) | (Some(_), None) => false,
                    };
                    selected.request == *request && same_route
                })
            {
                ServerRequestAdmission::Materialize
            } else {
                ServerRequestAdmission::Existing
            },
        )
    }

    /// Rate-limit authenticated requests before any potentially expensive Kura lookup.
    ///
    /// The outcome explicitly distinguishes terminating materialization,
    /// already-owned work, and a stateless responder-generation hint.
    /// Every delivery update retains the authenticated source cursor. A
    /// reconnect has no writer-flush continuity, so it retries that source's
    /// current chunk. A newly observed alternate source starts at chunk zero.
    pub(crate) fn admit_server_request(
        &mut self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
        reply_route: Option<&NetworkReplyRoute>,
        local_peer: &PeerId,
        now: Instant,
    ) -> Result<ServerRequestAdmission, MergeSidecarError> {
        if request.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1 {
            return Err(MergeSidecarError::UnsupportedVersion(request.version));
        }
        if &request.requester != sender || &request.responder != local_peer {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if reply_route.is_some_and(|route| route.semantic_target() != sender) {
            return Err(MergeSidecarError::PeerIdentityMismatch);
        }
        if reply_route.is_some_and(|route| !route.is_active()) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        let len = usize::try_from(request.encoded_len)
            .map_err(|_| MergeSidecarError::InvalidEncodedLength(request.encoded_len))?;
        if len == 0 || len > MAX_MERGE_LEDGER_ENTRY_BYTES {
            return Err(MergeSidecarError::InvalidEncodedLength(request.encoded_len));
        }
        if request.closed_through >= request.semantic_sequence.get()
            || request.request_id != request.canonical_request_id()
        {
            return Err(MergeSidecarError::RequestIdMismatch);
        }
        let observed_message_hash = HashOf::new(request).into();
        if request.service_generation > self.server_service_generation {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if request.service_generation < self.server_service_generation {
            let reply_route = reply_route.ok_or(MergeSidecarError::UnsolicitedResponse)?;
            return Ok(ServerRequestAdmission::GenerationHint(
                self.generation_hint_post(
                    sender,
                    local_peer,
                    reply_route,
                    request.service_generation,
                    observed_message_hash,
                ),
            ));
        }
        // A full semantic table is a roster-bound capacity condition, not
        // evidence for a new service generation. Return before pruning or
        // touching any durable ownership so an unrecognised requester cannot
        // trigger same-roster state loss.
        self.ensure_server_stream_slot(sender)?;
        // Lower-generation probes returned above without touching lifecycle
        // state. Only current-generation admission may prune obsolete local
        // writer ownership or mutate a semantic gate.
        self.prune_server_gates(now)?;
        let key = (sender.clone(), request.request_id);
        let request_hash = HashOf::new(request);
        let source = Self::server_request_source(sender, reply_route);
        let source_capacity = Self::route_source_capacity(reply_route)?;
        if source_capacity.is_some_and(|capacity| capacity != self.reply_source_capacity) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        self.preflight_server_request_stream(sender, request)?;
        self.advance_piggybacked_close_floor(sender, request)?;
        if self.server_request_gates.get(&key).is_some_and(|existing| {
            existing.service_generation != request.service_generation
                || existing.stream_epoch != request.stream_epoch
                || existing.semantic_sequence != request.semantic_sequence
                || !existing.request.same_occurrence_except_close_floor(request)
        }) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if reply_route.is_some_and(|route| !route.is_reply_writable()) {
            // The delivery remains authenticated while its inbound receiver
            // drains, but no exact reply writer can own a Kura lookup or
            // response bytes. A later writable tenure will replay and bind the
            // validated bounded semantic occurrence.
            return Ok(ServerRequestAdmission::Existing);
        }
        if let Some(route) = reply_route
            && let Some(gate) = self.server_request_gates.get_mut(&key).filter(|gate| {
                gate.service_generation == request.service_generation
                    && gate.stream_epoch == request.stream_epoch
                    && gate.semantic_sequence == request.semantic_sequence
                    && gate.request.same_occurrence_except_close_floor(request)
                    && gate.source_capacity == source_capacity
            })
        {
            let recovered_source = ServerRequestSource::RecoveredAuthenticated(
                route.authenticated_source_peer().clone(),
            );
            if !gate.attempts.contains_key(&source)
                && let Some(mut recovered) = gate.attempts.remove(&recovered_source)
            {
                recovered.reply_route = Some(route.clone());
                recovered.materialization_authorized = false;
                recovered.authorized_materialization_route = None;
                recovered.materialization_retryable =
                    matches!(recovered.cursor, ServerResponseCursor::Pending(_));
                recovered.inserted = now;
                gate.attempts.insert(source.clone(), recovered);
            }
        }
        if let Some(existing) = self
            .server_request_gates
            .get(&key)
            .filter(|existing| {
                existing.service_generation == request.service_generation
                    && existing.stream_epoch == request.stream_epoch
                    && existing.semantic_sequence == request.semantic_sequence
            })
            .cloned()
        {
            if !existing.request.same_occurrence_except_close_floor(request) {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if existing.semantic_sequence != request.semantic_sequence {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if existing.source_capacity != source_capacity {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if self.outbound.get(&key).is_some_and(|transfer| {
                !transfer.request.same_occurrence_except_close_floor(request)
            }) {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }

            if let Some(prior) = existing.attempts.get(&source) {
                let update = Self::route_update(reply_route, prior.reply_route.as_ref())?;
                if prior.cursor == ServerResponseCursor::Complete {
                    let gate = self
                        .server_request_gates
                        .get_mut(&key)
                        .expect("existing server gate remains present");
                    let attempt = gate
                        .attempts
                        .get_mut(&source)
                        .expect("completed source gate remains present");
                    if update != NetworkReplyRouteSourceUpdate::Exact {
                        attempt.reply_route = reply_route.cloned();
                        attempt.inserted = now;
                    }
                    attempt.materialization_authorized = false;
                    attempt.authorized_materialization_route = None;
                    attempt.materialization_retryable = false;
                    return Ok(ServerRequestAdmission::Existing);
                }
                if self
                    .outbound
                    .get(&key)
                    .is_some_and(|transfer| transfer.attempts.contains_key(&source))
                {
                    let mut enqueue_attempt = false;
                    let mut reconnect_retry_chunk = None;
                    if update != NetworkReplyRouteSourceUpdate::Exact {
                        let transfer = self
                            .outbound
                            .get_mut(&key)
                            .expect("observed outbound transfer remains present");
                        let attempt = transfer
                            .attempts
                            .get_mut(&source)
                            .expect("observed source attempt remains present");
                        attempt.reply_route = reply_route.cloned();
                        if update == NetworkReplyRouteSourceUpdate::Reconnected {
                            let retry_chunk = attempt.in_flight_chunk.unwrap_or(attempt.next_chunk);
                            attempt.next_chunk = retry_chunk;
                            attempt.in_flight_chunk = None;
                            reconnect_retry_chunk = Some(retry_chunk);
                        }
                        // A later delivery on the same tenure updates only
                        // the route for this source. If its current chunk is
                        // already awaiting the exact writer-flush witness,
                        // queueing here would let the caller drain a second
                        // concurrent copy of that same chunk. The eventual
                        // acknowledgement schedules the next chunk through
                        // the current route. A reconnect has no writer-flush
                        // continuity, so it cleared `in_flight_chunk` above
                        // and must queue the retained current chunk again.
                        if attempt.in_flight_chunk.is_none() && !attempt.queued {
                            attempt.queued = true;
                            enqueue_attempt = true;
                        }
                    }
                    let gate = self
                        .server_request_gates
                        .get_mut(&key)
                        .expect("existing server gate remains present");
                    let attempt = gate
                        .attempts
                        .get_mut(&source)
                        .expect("existing source gate remains present");
                    if update != NetworkReplyRouteSourceUpdate::Exact {
                        attempt.reply_route = reply_route.cloned();
                        if let Some(retry_chunk) = reconnect_retry_chunk {
                            attempt.cursor = ServerResponseCursor::Pending(retry_chunk);
                        }
                        attempt.inserted = now;
                    }
                    attempt.materialization_authorized = false;
                    attempt.authorized_materialization_route = None;
                    attempt.materialization_retryable = false;
                    if enqueue_attempt {
                        self.outbound_order.push_back((key, source));
                    }
                    return Ok(ServerRequestAdmission::Existing);
                }

                if self.outbound.contains_key(&key) {
                    let bytes = self
                        .outbound
                        .get(&key)
                        .expect("observed outbound transfer remains present")
                        .response_len;
                    if !self.can_add_outbound_attempt(&source, bytes) {
                        return Err(MergeSidecarError::Capacity("outbound response budget"));
                    }
                    let ServerResponseCursor::Pending(resume_chunk) = prior.cursor else {
                        unreachable!("completed source returned before outbound reattachment")
                    };
                    let gate = self
                        .server_request_gates
                        .get_mut(&key)
                        .expect("existing server gate remains present");
                    let attempt = gate
                        .attempts
                        .get_mut(&source)
                        .expect("existing source gate remains present");
                    attempt.reply_route = reply_route.cloned();
                    attempt.cursor = ServerResponseCursor::Pending(resume_chunk);
                    attempt.materialization_authorized = false;
                    attempt.authorized_materialization_route = None;
                    attempt.materialization_retryable = false;
                    attempt.inserted = now;
                    self.outbound
                        .get_mut(&key)
                        .expect("observed outbound transfer remains present")
                        .attempts
                        .insert(
                            source.clone(),
                            OutboundAttempt {
                                reply_route: reply_route.cloned(),
                                next_chunk: resume_chunk,
                                in_flight_chunk: None,
                                queued: true,
                            },
                        );
                    self.outbound_order.push_back((key, source));
                    return Ok(ServerRequestAdmission::Existing);
                }

                if prior.materialization_authorized {
                    // Terminating Kura work is owned by the exact delivery
                    // retained in `authorized_materialization_route`. Keep
                    // `reply_route` pinned to that delivery until completion;
                    // rebinding only the current route would leave the gate
                    // authorized for one writer while scheduling output on
                    // another. `prune_server_gates` releases this authority
                    // first when the exact writer is no longer writable, so a
                    // replacement tenure can then acquire fresh authority.
                    debug_assert!(
                        Self::attempt_has_writable_authorized_materialization_route(
                            &source, &prior,
                        ),
                        "terminating materialization must retain its exact writable reply route"
                    );
                    return Ok(ServerRequestAdmission::Existing);
                }
                if update == NetworkReplyRouteSourceUpdate::Exact {
                    if !prior.materialization_retryable {
                        return Err(MergeSidecarError::UnsolicitedResponse);
                    }
                    let gate = self
                        .server_request_gates
                        .get_mut(&key)
                        .expect("existing server gate remains present");
                    let attempt = gate
                        .attempts
                        .get_mut(&source)
                        .expect("existing source gate remains present");
                    attempt.materialization_authorized = false;
                    attempt.authorized_materialization_route = None;
                    attempt.materialization_retryable = true;
                    attempt.inserted = now;
                    return self.admission_after_fair_materialization_selection(
                        request,
                        reply_route,
                        now,
                    );
                }
                let gate = self
                    .server_request_gates
                    .get_mut(&key)
                    .expect("existing server gate remains present");
                let attempt = gate
                    .attempts
                    .get_mut(&source)
                    .expect("existing source gate remains present");
                attempt.reply_route = reply_route.cloned();
                attempt.materialization_authorized = false;
                attempt.authorized_materialization_route = None;
                attempt.materialization_retryable = true;
                attempt.inserted = now;
                return self.admission_after_fair_materialization_selection(
                    request,
                    reply_route,
                    now,
                );
            }

            if !Self::alternate_source_is_authorized(&existing, reply_route) {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if source_capacity.is_some_and(|capacity| existing.attempts.len() >= capacity)
                || self.server_gate_attempt_count() >= self.server_request_attempt_capacity
                || self.source_gate_count(&source) >= self.limits.server_request_gates_per_source
            {
                return Err(MergeSidecarError::Capacity("server request rate gate"));
            }
            if let Some(bytes) = self
                .outbound
                .get(&key)
                .map(|transfer| transfer.response_len)
            {
                if !self.can_add_outbound_attempt(&source, bytes) {
                    return Err(MergeSidecarError::Capacity("outbound response budget"));
                }
                self.preflight_lifecycle_mutation()?;
                self.server_request_gates
                    .get_mut(&key)
                    .expect("existing server gate remains present")
                    .attempts
                    .insert(
                        source.clone(),
                        ServerRequestGateAttempt {
                            reply_route: reply_route.cloned(),
                            materialization_authorized: false,
                            authorized_materialization_route: None,
                            materialization_retryable: false,
                            cursor: ServerResponseCursor::Pending(0),
                            pending_flush_chunk: None,
                            inserted: now,
                        },
                    );
                self.outbound
                    .get_mut(&key)
                    .expect("observed outbound transfer remains present")
                    .attempts
                    .insert(
                        source.clone(),
                        OutboundAttempt {
                            reply_route: reply_route.cloned(),
                            next_chunk: 0,
                            in_flight_chunk: None,
                            queued: true,
                        },
                    );
                self.outbound_order.push_back((key, source));
                return Ok(ServerRequestAdmission::Existing);
            }

            let materialization_in_progress = existing
                .attempts
                .values()
                .any(|attempt| attempt.materialization_authorized);
            self.preflight_lifecycle_mutation()?;
            self.server_request_gates
                .get_mut(&key)
                .expect("existing server gate remains present")
                .attempts
                .insert(
                    source,
                    ServerRequestGateAttempt {
                        reply_route: reply_route.cloned(),
                        materialization_authorized: false,
                        authorized_materialization_route: None,
                        materialization_retryable: true,
                        cursor: ServerResponseCursor::Pending(0),
                        pending_flush_chunk: None,
                        inserted: now,
                    },
                );
            return if materialization_in_progress {
                Ok(ServerRequestAdmission::Existing)
            } else {
                self.admission_after_fair_materialization_selection(request, reply_route, now)
            };
        }
        if self.server_gate_count_after_close(sender, request.stream_epoch, request.closed_through)
            >= self.server_request_gate_capacity
        {
            return Err(MergeSidecarError::Capacity("server request gate geometry"));
        }
        if self.server_gate_attempt_count_after_close(
            sender,
            request.stream_epoch,
            request.closed_through,
        ) >= self.server_request_attempt_capacity
        {
            return Err(MergeSidecarError::Capacity(
                "server request attempt geometry",
            ));
        }
        let source_count = self.source_gate_count_after_close(
            &source,
            sender,
            request.stream_epoch,
            request.closed_through,
        );
        if source_count >= self.limits.server_request_gates_per_source {
            return Err(MergeSidecarError::Capacity("server request rate gate"));
        }
        self.ensure_server_stream_slot(sender)?;
        self.preflight_lifecycle_mutation()?;
        match self.server_streams.get(sender).copied() {
            Some(stream) if request.stream_epoch > stream.stream_epoch => {
                self.supersede_server_stream(sender, request.stream_epoch);
            }
            Some(stream) if request.stream_epoch == stream.stream_epoch => {}
            Some(_) => {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            None => {
                self.server_streams.insert(
                    sender.clone(),
                    ServerStreamState {
                        stream_epoch: request.stream_epoch,
                        closed_through: 0,
                        highest_sequence: 0,
                    },
                );
            }
        }
        let stream = self
            .server_streams
            .get_mut(sender)
            .expect("validated server stream is installed before gate admission");
        stream.highest_sequence = stream.highest_sequence.max(request.semantic_sequence.get());
        if request.closed_through > stream.closed_through {
            self.advance_server_close_floor(sender, request.stream_epoch, request.closed_through);
        }
        debug_assert!(!self.server_request_gates.contains_key(&key));
        self.server_request_gates.insert(
            key,
            ServerRequestGate {
                request: request.clone(),
                request_hash,
                service_generation: request.service_generation,
                stream_epoch: request.stream_epoch,
                semantic_sequence: request.semantic_sequence,
                source_capacity,
                attempts: BTreeMap::from([(
                    source,
                    ServerRequestGateAttempt {
                        reply_route: reply_route.cloned(),
                        materialization_authorized: false,
                        authorized_materialization_route: None,
                        materialization_retryable: true,
                        cursor: ServerResponseCursor::Pending(0),
                        pending_flush_chunk: None,
                        inserted: now,
                    },
                )]),
            },
        );
        self.admission_after_fair_materialization_selection(request, reply_route, now)
    }

    /// Release terminating lookup authority after transient response pressure.
    ///
    /// This path is reserved for outbound-capacity rejection after Kura and
    /// metadata validation succeeded. The semantic gate remains bounded and
    /// retryable so the exact authenticated delivery can try again once an
    /// older response releases capacity.
    pub(crate) fn cancel_unmaterialized_server_request(
        &mut self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) {
        let key = (sender.clone(), request.request_id);
        if self.outbound.contains_key(&key) {
            return;
        }
        if let Some(gate) = self.server_request_gates.get_mut(&key).filter(|gate| {
            gate.service_generation == request.service_generation
                && gate.stream_epoch == request.stream_epoch
                && gate.semantic_sequence == request.semantic_sequence
                && gate.request.same_occurrence_except_close_floor(request)
        }) {
            Self::release_authorized_server_request_attempts(gate);
        }
    }

    /// Durably retire an exact request which cannot materialize a response.
    ///
    /// Kura absence, read failure, metadata mismatch, and non-holder service
    /// decisions are terminal for this admitted occurrence. Persist the
    /// projected gate-free lifecycle before changing memory so a crash cannot
    /// resurrect the source reservations, while an exact later replay may
    /// acquire a fresh gate if the entry becomes serviceable.
    pub(crate) fn retire_unmaterialized_server_request(
        &mut self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) -> Result<(), MergeSidecarError> {
        let key = (sender.clone(), request.request_id);
        if self.outbound.contains_key(&key)
            || self
                .outbound_order
                .iter()
                .any(|(queued_key, _)| queued_key == &key)
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "terminal server request retirement observed materialized output".to_owned(),
            ));
        }
        let gate = self.server_request_gates.get(&key).ok_or_else(|| {
            MergeSidecarError::LifecycleJournal(
                "terminal server request retirement lost its exact gate".to_owned(),
            )
        })?;
        if !gate.request.same_occurrence_except_close_floor(request)
            || gate.service_generation != request.service_generation
            || gate.service_generation != self.server_service_generation
            || gate.stream_epoch != request.stream_epoch
            || gate.semantic_sequence != request.semantic_sequence
            || &request.requester != sender
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "terminal server request retirement differs from its exact gate".to_owned(),
            ));
        }
        let stream = self
            .server_streams
            .get(sender)
            .filter(|stream| stream.stream_epoch == request.stream_epoch)
            .ok_or_else(|| {
                MergeSidecarError::LifecycleJournal(
                    "terminal server request retirement lost its exact stream".to_owned(),
                )
            })?;
        let retained_highest_sequence = self
            .server_request_gates
            .iter()
            .filter(|(candidate_key, candidate)| {
                &candidate_key.0 == sender
                    && candidate_key.1 != request.request_id
                    && candidate.service_generation == request.service_generation
                    && candidate.stream_epoch == request.stream_epoch
            })
            .map(|(_, candidate)| candidate.semantic_sequence.get())
            .fold(stream.closed_through, u64::max);
        if stream.highest_sequence != retained_highest_sequence.max(request.semantic_sequence.get())
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "terminal server request retirement observed a divergent stream high-water"
                    .to_owned(),
            ));
        }

        let mut projected = self.lifecycle_snapshot()?;
        let gate_index = projected
            .payload
            .server_request_gates
            .iter()
            .position(|durable| {
                durable.requester == *sender && durable.request_id == request.request_id
            })
            .ok_or_else(|| {
                MergeSidecarError::LifecycleJournal(
                    "projected terminal server request retirement lost its exact gate".to_owned(),
                )
            })?;
        let durable_gate = &projected.payload.server_request_gates[gate_index];
        if !durable_gate
            .request
            .same_occurrence_except_close_floor(request)
            || durable_gate.service_generation != request.service_generation
            || durable_gate.stream_epoch != request.stream_epoch
            || durable_gate.semantic_sequence != request.semantic_sequence
        {
            return Err(MergeSidecarError::LifecycleJournal(
                "projected terminal server request retirement differs from its exact gate"
                    .to_owned(),
            ));
        }
        projected.payload.server_request_gates.remove(gate_index);
        let mut durable_streams = projected
            .payload
            .server_streams
            .iter_mut()
            .filter(|durable| {
                durable.requester == *sender
                    && durable.service_generation == request.service_generation
                    && durable.stream_epoch == request.stream_epoch
            });
        let durable_stream = durable_streams.next().ok_or_else(|| {
            MergeSidecarError::LifecycleJournal(
                "projected terminal server request retirement lost its exact stream".to_owned(),
            )
        })?;
        durable_stream.highest_sequence = retained_highest_sequence;
        if durable_streams.next().is_some() {
            return Err(MergeSidecarError::LifecycleJournal(
                "projected terminal server request retirement found duplicate streams".to_owned(),
            ));
        }
        projected.payload_hash = HashOf::new(&projected.payload);
        #[cfg(test)]
        if std::mem::take(&mut self.obstruct_next_terminal_retirement_persist) {
            self.obstruct_lifecycle_journal_temp_for_test();
        }
        self.persist_lifecycle_projection(projected)?;

        self.server_request_gates
            .remove(&key)
            .expect("preflighted terminal server request gate remains present");
        self.server_streams
            .get_mut(sender)
            .expect("preflighted terminal server request stream remains present")
            .highest_sequence = retained_highest_sequence;
        Ok(())
    }

    fn release_authorized_server_request_attempts(gate: &mut ServerRequestGate) {
        for attempt in gate
            .attempts
            .values_mut()
            .filter(|attempt| attempt.materialization_authorized)
        {
            attempt.materialization_authorized = false;
            attempt.authorized_materialization_route = None;
            attempt.materialization_retryable =
                matches!(attempt.cursor, ServerResponseCursor::Pending(_));
        }
    }

    fn park_authorized_server_request_attempts(gate: &mut ServerRequestGate, now: Instant) {
        for attempt in gate
            .attempts
            .values_mut()
            .filter(|attempt| attempt.materialization_authorized)
        {
            attempt.materialization_authorized = false;
            attempt.authorized_materialization_route = None;
            attempt.materialization_retryable = false;
            attempt.inserted = now;
        }
    }

    /// Consume one exact admitted request and queue its validated local entry.
    ///
    /// The admission gate binds the canonical request and exact authenticated
    /// delivery. Materialized bytes are shared by every admitted source, while
    /// each source receives its own source-local non-regressing chunk cursor.
    /// Completed attempts remain terminal until the authenticated requester's
    /// cumulative close floor covers their semantic sequence. No timer,
    /// reconnect, or height reconstruction may reset a source cursor.
    pub(crate) fn enqueue_response(
        &mut self,
        request: CertifiedMergeSidecarRequestV1,
        reply_route: Option<NetworkReplyRoute>,
        bytes: Vec<u8>,
        now: Instant,
    ) -> Result<(), MergeSidecarError> {
        let len = usize::try_from(request.encoded_len)
            .map_err(|_| MergeSidecarError::InvalidEncodedLength(request.encoded_len))?;
        if bytes.len() != len || len == 0 || len > MAX_MERGE_LEDGER_ENTRY_BYTES {
            return Err(MergeSidecarError::LengthMismatch {
                expected: len,
                actual: bytes.len(),
            });
        }
        let key = (request.requester.clone(), request.request_id);
        if self.outbound.contains_key(&key) {
            return Err(MergeSidecarError::Capacity("duplicate outbound session"));
        }
        let source = Self::server_request_source(&request.requester, reply_route.as_ref());
        let gate = self
            .server_request_gates
            .get(&key)
            .ok_or(MergeSidecarError::UnsolicitedResponse)?;
        let gate_attempt = gate
            .attempts
            .get(&source)
            .ok_or(MergeSidecarError::UnsolicitedResponse)?;
        let exact_attempt_completed = matches!(gate_attempt.cursor, ServerResponseCursor::Complete);
        let same_route = match (
            reply_route.as_ref(),
            gate_attempt.authorized_materialization_route.as_ref(),
        ) {
            (None, None) => true,
            (Some(candidate), Some(admitted)) => candidate.same_delivery(admitted),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if gate.service_generation != request.service_generation
            || gate.stream_epoch != request.stream_epoch
            || gate.semantic_sequence != request.semantic_sequence
            || !gate.request.same_occurrence_except_close_floor(&request)
            || !gate_attempt.materialization_authorized
            || !same_route
        {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        let request = gate.request.clone();
        let selected_route_is_draining = reply_route
            .as_ref()
            .is_some_and(|route| route.is_active() && !route.is_reply_writable());
        let response_len = bytes.len();
        let chunk_count = response_len.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        let chunk_count_wire = u32::try_from(chunk_count)
            .expect("bounded certified merge response chunk count fits u32");
        let chunks = bytes
            .chunks(MAX_CERTIFIED_MERGE_CHUNK_BYTES)
            .enumerate()
            .map(|(index, chunk_bytes)| {
                Arc::new(CertifiedMergeSidecarMessage::Chunk(
                    CertifiedMergeSidecarChunkV1 {
                        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                        service_generation: request.service_generation,
                        stream_epoch: request.stream_epoch,
                        semantic_sequence: request.semantic_sequence,
                        request_id: request.request_id,
                        entry_hash: request.entry_hash,
                        encoded_len: request.encoded_len,
                        epoch_id: request.epoch_id,
                        reference_digest: request.reference_digest,
                        requester: request.requester.clone(),
                        responder: request.responder.clone(),
                        chunk_index: u32::try_from(index)
                            .expect("bounded certified merge chunk index fits u32"),
                        chunk_count: chunk_count_wire,
                        bytes: chunk_bytes.to_vec(),
                    },
                ))
            })
            .collect::<Vec<_>>();
        debug_assert_eq!(chunks.len(), chunk_count);
        let mut capacity_rejected_attempts = Vec::new();
        let mut writer_unavailable_attempts = Vec::new();
        let mut admitted_attempts = Vec::new();
        let mut remaining_global_sessions = self
            .outbound_session_capacity
            .saturating_sub(self.outbound_attempt_count());
        for (source, attempt) in &gate.attempts {
            if let (ServerResponseCursor::Pending(resume_chunk), Some(pending)) =
                (attempt.cursor, &attempt.pending_flush_chunk)
            {
                let reproduced = chunks
                    .get(resume_chunk)
                    .and_then(ServerPendingChunkIdentity::from_message);
                if reproduced.as_ref() != Some(pending) {
                    return Err(MergeSidecarError::FlushIdentityMismatch(
                        "rematerialization changed a retained source's current chunk identity",
                    ));
                }
            }
            let ServerResponseCursor::Pending(resume_chunk) = attempt.cursor else {
                continue;
            };
            if !Self::attempt_has_writable_materialization_route(source, attempt) {
                writer_unavailable_attempts.push(source.clone());
                continue;
            }
            if remaining_global_sessions == 0
                || self.source_outbound_count(source) >= self.limits.outbound_sessions_per_source
                || self
                    .source_outbound_bytes(source)
                    .saturating_add(response_len)
                    > self.limits.outbound_bytes_per_source
            {
                capacity_rejected_attempts.push(source.clone());
                continue;
            }
            remaining_global_sessions -= 1;
            admitted_attempts.push((source.clone(), attempt.reply_route.clone(), resume_chunk));
        }
        if admitted_attempts.is_empty() && capacity_rejected_attempts.is_empty() {
            let gate = self
                .server_request_gates
                .get_mut(&key)
                .expect("validated server request gate remains present");
            // A successful old-writer receipt may have completed the source
            // represented by this exact authorization while terminating local
            // materialization was in flight. That callback is a consumed
            // no-op. A still-pending authorization with no writable route
            // releases every response reservation; an authenticated draining
            // route is retryable, while a fully inactive route fails closed.
            if exact_attempt_completed {
                Self::park_authorized_server_request_attempts(gate, now);
                return Ok(());
            }
            Self::release_authorized_server_request_attempts(gate);
            return Err(if selected_route_is_draining {
                // The exact authenticated delivery still exists, but its
                // writer timed out while Kura materialization was in flight.
                // Classify this like transient output pressure so the lane
                // parks the durable cursor instead of fail-stopping.
                MergeSidecarError::Capacity("outbound response budget")
            } else {
                MergeSidecarError::UnsolicitedResponse
            });
        }
        if admitted_attempts.is_empty()
            || self.global_outbound_bytes().saturating_add(response_len)
                > self.outbound_byte_capacity
        {
            let gate = self
                .server_request_gates
                .get_mut(&key)
                .expect("validated server request gate remains present");
            // Capacity pressure is transient. Release terminating lookup
            // authority, but retain a retryable semantic gate so the exact
            // authenticated delivery can make progress once an older response
            // relinquishes its source reservation.
            Self::release_authorized_server_request_attempts(gate);
            return Err(MergeSidecarError::Capacity("outbound response budget"));
        }
        let gate = self
            .server_request_gates
            .get_mut(&key)
            .expect("validated server request gate remains present");
        // One authorized lookup satisfies every pending, writable source already
        // admitted to this semantic gate. A partitioned source may still fail
        // to acquire an outbound session; retain its bounded route history and
        // source-local cursor so it can retry after capacity returns. A
        // completed source remains terminal across connection tenures while
        // this semantic gate exists.
        Self::park_authorized_server_request_attempts(gate, now);
        for (source, _, _) in &admitted_attempts {
            let attempt = gate
                .attempts
                .get_mut(source)
                .expect("admitted output source remains in its semantic gate");
            attempt.materialization_retryable = false;
            attempt.inserted = now;
        }
        for source in capacity_rejected_attempts {
            let attempt = gate
                .attempts
                .get_mut(&source)
                .expect("capacity-rejected source remains in its semantic gate");
            attempt.materialization_retryable =
                matches!(attempt.cursor, ServerResponseCursor::Pending(_));
        }
        for source in writer_unavailable_attempts {
            let attempt = gate
                .attempts
                .get_mut(&source)
                .expect("writer-unavailable source remains in its semantic gate");
            attempt.materialization_retryable =
                matches!(attempt.cursor, ServerResponseCursor::Pending(_));
        }
        let mut attempts = BTreeMap::new();
        for (source, reply_route, resume_chunk) in admitted_attempts {
            self.outbound_order.push_back((key.clone(), source.clone()));
            attempts.insert(
                source,
                OutboundAttempt {
                    reply_route,
                    next_chunk: resume_chunk,
                    in_flight_chunk: None,
                    queued: true,
                },
            );
        }
        let replaced = self.outbound.insert(
            key,
            OutboundTransfer {
                request,
                response_len,
                chunks,
                attempts,
            },
        );
        debug_assert!(replaced.is_none());
        Ok(())
    }

    /// Select at most `limit` response chunks in deterministic session order.
    ///
    /// The returned boolean reports whether the durable gate cursor or pending
    /// writer identity changed. Production callers must persist such a change
    /// before handing any returned post to exact output.
    fn drain_outbound_chunks_inner(
        &mut self,
        limit: usize,
        now: Instant,
    ) -> (Vec<MergeSidecarPost>, bool) {
        let mut posts = Vec::new();
        let mut lifecycle_changed = false;
        while posts.len() < limit {
            let Some((key, source)) = self.outbound_order.pop_front() else {
                break;
            };
            let mut completed = false;
            let mut unwritable = false;
            let mut identity_mismatch = false;
            let cursor;
            let mut emitted_chunk_identity = None;
            let retained_chunk_identity = self
                .server_request_gates
                .get(&key)
                .and_then(|gate| gate.attempts.get(&source))
                .and_then(|attempt| attempt.pending_flush_chunk.clone());
            if let Some(transfer) = self.outbound.get_mut(&key) {
                let request = &transfer.request;
                let Some(attempt) = transfer.attempts.get_mut(&source) else {
                    debug_assert!(false, "outbound response order lost its source attempt");
                    continue;
                };
                attempt.queued = false;
                if !Self::outbound_attempt_has_writable_route(&source, attempt) {
                    unwritable = true;
                    cursor = ServerResponseCursor::Pending(
                        attempt.in_flight_chunk.unwrap_or(attempt.next_chunk),
                    );
                } else {
                    let count = transfer.chunks.len();
                    let index = attempt.in_flight_chunk.unwrap_or(attempt.next_chunk);
                    if index >= count {
                        completed = true;
                        cursor = ServerResponseCursor::Complete;
                    } else {
                        let message = Arc::clone(
                            transfer
                                .chunks
                                .get(index)
                                .expect("bounded sidecar cursor names a cached chunk"),
                        );
                        let identity = ServerPendingChunkIdentity::from_message(&message)
                            .expect("outbound response contains only certified chunks");
                        if retained_chunk_identity
                            .as_ref()
                            .is_some_and(|pending| pending != &identity)
                        {
                            // Never replace an older writer-flush witness with
                            // divergent rematerialized bytes. Park this source
                            // and preserve the exact marker so its genuine late
                            // receipt can still advance; subsequent lookup also
                            // fails closed on the same mismatch.
                            identity_mismatch = true;
                        } else {
                            emitted_chunk_identity = Some(identity);
                            posts.push(MergeSidecarPost {
                                peer: request.requester.clone(),
                                reply_route: attempt.reply_route.clone(),
                                message,
                            });
                            attempt.in_flight_chunk = Some(index);
                        }
                        cursor = ServerResponseCursor::Pending(index);
                    }
                }
            } else {
                debug_assert!(false, "outbound response order lost its transfer");
                continue;
            }

            if let Some(gate_attempt) = self
                .server_request_gates
                .get_mut(&key)
                .and_then(|gate| gate.attempts.get_mut(&source))
            {
                let cursor_before = gate_attempt.cursor;
                let pending_flush_before = gate_attempt.pending_flush_chunk.clone();
                gate_attempt.cursor = cursor;
                if completed {
                    gate_attempt.pending_flush_chunk = None;
                } else if let Some(identity) = emitted_chunk_identity {
                    gate_attempt.pending_flush_chunk = Some(identity);
                }
                lifecycle_changed |= gate_attempt.cursor != cursor_before
                    || gate_attempt.pending_flush_chunk != pending_flush_before;
                if unwritable || identity_mismatch || completed {
                    gate_attempt.materialization_authorized = false;
                    gate_attempt.authorized_materialization_route = None;
                    gate_attempt.materialization_retryable =
                        unwritable && matches!(cursor, ServerResponseCursor::Pending(_));
                    gate_attempt.inserted = now;
                }
            }
            if unwritable || identity_mismatch || completed {
                let transfer = self
                    .outbound
                    .get_mut(&key)
                    .expect("serviced outbound transfer remains present");
                transfer.attempts.remove(&source);
                if transfer.attempts.is_empty() {
                    self.outbound.remove(&key);
                }
            }
        }
        (posts, lifecycle_changed)
    }

    fn outbound_drain_requires_lifecycle_commit(&self, limit: usize) -> bool {
        let mut emitted = 0;
        for (key, source) in &self.outbound_order {
            if emitted >= limit {
                break;
            }
            let Some(transfer) = self.outbound.get(key) else {
                continue;
            };
            let Some(outbound_attempt) = transfer.attempts.get(source) else {
                continue;
            };
            let Some(gate_attempt) = self
                .server_request_gates
                .get(key)
                .and_then(|gate| gate.attempts.get(source))
            else {
                continue;
            };
            let retained = gate_attempt.pending_flush_chunk.clone();
            let index = outbound_attempt
                .in_flight_chunk
                .unwrap_or(outbound_attempt.next_chunk);
            let (cursor, pending, emitted_post) =
                if !Self::outbound_attempt_has_writable_route(source, outbound_attempt) {
                    (ServerResponseCursor::Pending(index), retained, false)
                } else if index >= transfer.chunks.len() {
                    (ServerResponseCursor::Complete, None, false)
                } else {
                    let identity = ServerPendingChunkIdentity::from_message(
                        transfer
                            .chunks
                            .get(index)
                            .expect("bounded sidecar cursor names a cached chunk"),
                    )
                    .expect("outbound response contains only certified chunks");
                    if retained
                        .as_ref()
                        .is_some_and(|pending| pending != &identity)
                    {
                        (ServerResponseCursor::Pending(index), retained, false)
                    } else {
                        (ServerResponseCursor::Pending(index), Some(identity), true)
                    }
                };
            if gate_attempt.cursor != cursor || gate_attempt.pending_flush_chunk != pending {
                return true;
            }
            if emitted_post {
                emitted += 1;
            }
        }
        false
    }

    /// Emit at most `limit` response chunks after durably publishing their
    /// pending writer identities and non-regressing source cursors.
    pub(crate) fn drain_outbound_chunks_durable(
        &mut self,
        limit: usize,
        now: Instant,
    ) -> Result<Vec<MergeSidecarPost>, MergeSidecarError> {
        self.reclaim_inactive_outbound_attempts(now)?;
        if self.outbound_drain_requires_lifecycle_commit(limit) {
            self.preflight_lifecycle_mutation()?;
        }
        let (posts, lifecycle_changed) = self.drain_outbound_chunks_inner(limit, now);
        if lifecycle_changed {
            self.persist_lifecycle_state()?;
        }
        Ok(posts)
    }

    #[cfg(test)]
    fn drain_outbound_chunks(&mut self, limit: usize, now: Instant) -> Vec<MergeSidecarPost> {
        self.drain_outbound_chunks_inner(limit, now).0
    }

    /// Advance one source cursor after its exact peer writer flushes the current chunk.
    ///
    /// Actor admission alone is insufficient. Duplicate or late flush receipts
    /// after conservative teardown are harmless. A receipt which still names
    /// retained state must match the immutable transfer, source, chunk index,
    /// and fixed chunk count exactly.
    pub(crate) fn acknowledge_outbound_chunk(
        &mut self,
        admission: &CertifiedMergeSidecarChunkAdmission,
        now: Instant,
    ) -> Result<bool, MergeSidecarError> {
        if !admission.projection_matches_identity(&admission.flush_identity) {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "retained actor identity differs from its immutable projection",
            ));
        }
        let occurrence = reliable_flush_application_occurrence_projection(admission)?;
        let worker_trace =
            admission
                .confirmed_worker_trace
                .ok_or(MergeSidecarError::FlushIdentityMismatch(
                    "writer flush admission has no accepted worker transition",
                ))?;
        let checked_worker = check_production_reliable_flush_worker_transition(worker_trace)
            .ok_or(MergeSidecarError::FlushIdentityMismatch(
                "accepted worker transition differs from the lane occurrence",
            ))?;
        let checked_link =
            check_production_reliable_flush_link_transition(worker_trace, occurrence).ok_or(
                MergeSidecarError::FlushIdentityMismatch(
                    "accepted worker transition differs from the lane occurrence",
                ),
            )?;
        let worker_trace = checked_worker.into_projection();
        let (linked_worker, linked_occurrence) = checked_link.into_projection();
        if linked_worker != worker_trace || linked_occurrence != occurrence {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "checked worker flush token changed its exact occurrence",
            ));
        }
        let projection = admission.projection();
        let chunk_index = usize::try_from(projection.chunk_index).map_err(|_| {
            MergeSidecarError::FlushIdentityMismatch("chunk index is not representable")
        })?;
        let expected_chunk_cursor_after =
            chunk_index
                .checked_add(1)
                .ok_or(MergeSidecarError::FlushIdentityMismatch(
                    "chunk cursor overflowed",
                ))?;
        let count = usize::try_from(projection.chunk_count).map_err(|_| {
            MergeSidecarError::FlushIdentityMismatch("chunk count is not representable")
        })?;
        if count == 0
            || projection.message_cursor_before != 0
            || projection.message_cursor_after != 1
            || projection.chunk_cursor_before != chunk_index
            || projection.chunk_cursor_after != expected_chunk_cursor_after
            || expected_chunk_cursor_after > count
        {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "response or per-source cursor changed before acknowledgement",
            ));
        }

        let gate = match preflight_reliable_flush_gate(self, admission, chunk_index)? {
            ReliableFlushGatePreflight::ConsumeWithoutMutation => {
                let _ = admission.flush_identity.claim_writer_flush_once();
                return Ok(false);
            }
            ReliableFlushGatePreflight::Ready(gate) => gate,
        };
        let outbound =
            match preflight_reliable_flush_outbound(self, admission, &gate, chunk_index, count)? {
                ReliableFlushOutboundPreflight::RejectWithoutClaim => return Ok(false),
                ReliableFlushOutboundPreflight::Ready(outbound) => outbound,
            };
        let plan = finish_reliable_flush_application_plan(
            self,
            gate,
            outbound,
            occurrence,
            expected_chunk_cursor_after,
            count,
        )?;
        let prospective_observation = predict_reliable_flush_application(&plan, now);
        let prospective_application =
            reliable_flush_application_projection(&plan, &prospective_observation, now);
        let Some(checked_application) =
            check_production_reliable_flush_application_transition(prospective_application)
        else {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "prospective writer flush application failed its source-lane gate",
            ));
        };
        let Some(checked_link) =
            check_production_reliable_flush_link_transition(worker_trace, prospective_application)
        else {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "prospective writer flush application disconnected from its worker transition",
            ));
        };
        let prospective_application = checked_application.into_projection();
        let (linked_worker, linked_application) = checked_link.into_projection();
        if linked_worker != worker_trace || linked_application != prospective_application {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "checked writer flush token changed its exact occurrence",
            ));
        }
        self.preflight_lifecycle_mutation()?;

        // This is the only linearization point. Every fallible identity,
        // cursor, route, shared-state, scalar, and durable-generation check
        // completed above.
        if !admission.flush_identity.claim_writer_flush_once() {
            return Ok(false);
        }
        apply_reliable_flush_application(self, &plan, now);
        let observation = observe_reliable_flush_application(self, &plan);
        let application = reliable_flush_application_projection(&plan, &observation, now);
        if application != prospective_application {
            // The production caller holds `ConsensusFailStopOperation`; this
            // internal post-CAS invariant error drops that incomplete guard,
            // permanently closes exact output, and requires process restart.
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "writer flush application diverged from its checked prospective transition",
            ));
        }
        self.persist_lifecycle_state()?;
        Ok(true)
    }

    /// Remove deferred carriers that the actor no longer retains as pending.
    ///
    /// Active fetch state has no wall-clock lifetime: a valid pending carrier
    /// must remain recoverable across an arbitrarily long holder outage.  The
    /// actor supplies the authoritative pending set so superseded and committed
    /// carriers still release their bounded reservations deterministically.
    pub(crate) fn retain_pending_blocks(
        &mut self,
        pending_blocks: &BTreeSet<HashOf<BlockHeader>>,
        committed_height: u64,
    ) -> Result<(), MergeSidecarError> {
        let closes_request_stream = self.inbound.values().any(|assembly| {
            assembly.current.is_some()
                && assembly.deferred.values().all(|carrier| {
                    carrier.height <= committed_height || !pending_blocks.contains(&carrier.hash)
                })
        });
        if closes_request_stream {
            self.preflight_lifecycle_mutation()?;
        }
        for assembly in self.inbound.values_mut() {
            assembly.deferred.retain(|hash, carrier| {
                carrier.height > committed_height && pending_blocks.contains(hash)
            });
        }
        let retired = self
            .inbound
            .iter()
            .filter(|(_, assembly)| assembly.deferred.is_empty())
            .filter_map(|(_, assembly)| {
                assembly.current.as_ref().map(|attempt| {
                    (
                        attempt.holder.clone(),
                        attempt.stream_epoch,
                        attempt.semantic_sequence,
                    )
                })
            })
            .collect::<Vec<_>>();
        self.inbound
            .retain(|_, assembly| !assembly.deferred.is_empty());
        for (holder, stream_epoch, semantic_sequence) in retired {
            self.close_request_sequence(&holder, stream_epoch, semantic_sequence);
        }
        self.persist_lifecycle_state()
    }

    /// Rotate stalled holders and emit bounded, indefinitely retried requests.
    pub(crate) fn tick_bounded(
        &mut self,
        requester: &PeerId,
        now: Instant,
        limit: usize,
    ) -> Result<Vec<MergeSidecarPost>, MergeSidecarError> {
        self.prune_server_gates(now)?;
        let timed_out: Vec<_> = self
            .inbound
            .iter()
            .filter(|(_, assembly)| {
                assembly.current.as_ref().is_some_and(|attempt| {
                    now.saturating_duration_since(attempt.last_progress_at)
                        >= retry_timeout(self.limits.request_timeout, assembly.attempts)
                })
            })
            .map(|(hash, _)| *hash)
            .collect();
        if !timed_out.is_empty() {
            self.preflight_lifecycle_mutation()?;
        }
        let mut closed = Vec::new();
        for hash in &timed_out {
            if let Some(assembly) = self.inbound.get_mut(hash) {
                if let Some(attempt) = assembly.current.take() {
                    closed.push((
                        attempt.holder,
                        attempt.stream_epoch,
                        attempt.semantic_sequence,
                    ));
                }
                assembly.chunks.clear();
                assembly.received_bytes = 0;
                assembly.complete_pending_validation = false;
            }
        }
        let closed_any = !closed.is_empty();
        for (holder, stream_epoch, semantic_sequence) in closed {
            self.close_request_sequence(&holder, stream_epoch, semantic_sequence);
        }
        if closed_any {
            // A newly timed-out fetch has exhausted its current holder and
            // must rotate without spending the caller's sole bounded slot on
            // administrative stream closure. At most one such retry may
            // preempt a due Close: if another retry times out before the Close
            // runs, the retained debt forces Close service first.
            if self.timeout_retry_close_deferred {
                self.tick_close_next = true;
            } else {
                self.tick_close_next = false;
                self.timeout_retry_close_deferred = true;
            }
            self.persist_lifecycle_state()?;
        }
        let idle_keys = self
            .inbound
            .iter()
            .filter(|(_, assembly)| {
                assembly.current.is_none() && !assembly.complete_pending_validation
            })
            .map(|(hash, _)| *hash)
            .collect::<Vec<_>>();
        let start = self.inbound_cursor.map_or(0, |cursor| {
            idle_keys.partition_point(|candidate| *candidate <= cursor)
        });
        let mut idle = idle_keys[start..]
            .iter()
            .chain(&idle_keys[..start])
            .copied()
            .collect::<VecDeque<_>>();
        let mut close_responders = self.due_close_responders(now);
        let mut posts = Vec::new();
        let mut lifecycle_changed = false;
        while posts.len() < limit {
            let request_ready = !idle.is_empty() || !close_responders.is_empty();
            let response_ready = !self.outbound_order.is_empty();
            if !request_ready && !response_ready {
                break;
            }
            let contended = request_ready && response_ready;
            let response_first = response_ready && (!request_ready || self.tick_response_next);
            let mut emitted = false;

            if response_first {
                if self.outbound_drain_requires_lifecycle_commit(1) {
                    self.preflight_lifecycle_mutation()?;
                }
                let (mut drained, changed) = self.drain_outbound_chunks_inner(1, now);
                lifecycle_changed |= changed;
                if let Some(post) = drained.pop() {
                    posts.push(post);
                    emitted = true;
                    if contended {
                        self.tick_response_next = false;
                    }
                }
            } else if let Some(post) =
                self.begin_request_or_close(requester, &mut idle, &mut close_responders, now)?
            {
                posts.push(post);
                emitted = true;
                if contended {
                    self.tick_response_next = true;
                }
            }

            // A nominally ready class can become ineligible while bounded
            // per-peer reservations are inspected. Preserve useful capacity
            // by trying the other class without advancing its fairness turn.
            if !emitted && response_first {
                if let Some(post) =
                    self.begin_request_or_close(requester, &mut idle, &mut close_responders, now)?
                {
                    posts.push(post);
                    emitted = true;
                }
            } else if !emitted {
                if response_ready && self.outbound_drain_requires_lifecycle_commit(1) {
                    self.preflight_lifecycle_mutation()?;
                }
                let (mut drained, changed) = self.drain_outbound_chunks_inner(1, now);
                lifecycle_changed |= changed;
                if let Some(post) = drained.pop() {
                    posts.push(post);
                    emitted = true;
                }
            }

            if !emitted {
                break;
            }
        }
        if lifecycle_changed {
            self.persist_lifecycle_state()?;
        }
        Ok(posts)
    }

    #[cfg(test)]
    fn inbound_len(&self) -> usize {
        self.inbound.len()
    }
}

/// Exact context in which a local merge signature is permitted.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub(crate) struct MergeSigningContextV1 {
    /// Merge epoch being signed.
    pub(crate) epoch_id: u64,
    /// Merge view being signed.
    pub(crate) view: u64,
    /// Exact global carrier height for this signing decision.
    pub(crate) carrier_height: u64,
    /// Exact canonical parent of the intended global carrier.
    pub(crate) parent_hash: HashOf<BlockHeader>,
    /// Exact ordered merge-committee roster hash.
    pub(crate) validator_set_hash: HashOf<Vec<PeerId>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSigningGuardRecordV2 {
    version: u8,
    context: MergeSigningContextV1,
    message_digest: Hash,
    candidate_hash: Hash,
    candidate_encoded_len: u64,
    candidate_bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct MergeSigningHighWaterV2 {
    version: u8,
    committed_epoch: u64,
    committed_carrier_height: u64,
}

/// Crash-safe local merge-signature anti-equivocation journal.
///
/// The record is atomically published and fsynced before signature generation.
/// A crash can therefore leave a harmless recorded-but-not-emitted decision,
/// never an emitted signature without its durable guard.
#[derive(Debug)]
pub(crate) struct MergeSigningGuard {
    directory: PathBuf,
    committed_epoch: u64,
    committed_carrier_height: u64,
    limits: MergeSigningGuardLimits,
}

impl MergeSigningGuard {
    /// Open the guard under the Kura root and fail closed on malformed records.
    #[cfg(test)]
    pub(crate) fn open(store_root: &Path) -> Result<Self, MergeSidecarError> {
        Self::open_with_committed_frontier(store_root, 0, 0, MergeSigningGuardLimits::defaults())
    }

    /// Open and reconcile the guard against the exact latest globally ordered
    /// merge epoch recovered from canonical Kura/state.
    #[cfg(test)]
    pub(crate) fn open_with_committed_epoch(
        store_root: &Path,
        committed_epoch: u64,
    ) -> Result<Self, MergeSidecarError> {
        Self::open_with_committed_frontier(
            store_root,
            committed_epoch,
            0,
            MergeSigningGuardLimits::defaults(),
        )
    }

    /// Open against the exact globally finalized merge epoch and carrier height.
    pub(crate) fn open_with_committed_frontier(
        store_root: &Path,
        committed_epoch: u64,
        committed_carrier_height: u64,
        limits: MergeSigningGuardLimits,
    ) -> Result<Self, MergeSidecarError> {
        Self::reject_legacy_journals(store_root)?;
        let directory = store_root.join(SIGNING_GUARD_DIR);
        ensure_regular_directory(&directory)?;
        // The first record is not crash-safe unless the directory entry itself
        // is durable in the Kura root before any signature can be emitted.
        sync_directory(store_root)?;
        Self::guard_directory_bytes(&directory, limits.max_total_bytes)?;
        Self::reconcile_temps(&directory, limits)?;
        let durable_high_water = Self::read_high_water(&directory, limits.max_record_bytes)?
            .unwrap_or(MergeSigningHighWaterV2 {
                version: SIGNING_GUARD_VERSION,
                committed_epoch: 0,
                committed_carrier_height: 0,
            });
        if committed_epoch < durable_high_water.committed_epoch
            || committed_carrier_height < durable_high_water.committed_carrier_height
        {
            return Err(MergeSidecarError::SigningGuard(format!(
                "canonical committed frontier epoch={committed_epoch} height={committed_carrier_height} regressed below durable signing high-water epoch={} height={}",
                durable_high_water.committed_epoch, durable_high_water.committed_carrier_height,
            )));
        }
        let mut guard = Self {
            directory,
            committed_epoch: durable_high_water.committed_epoch,
            committed_carrier_height: durable_high_water.committed_carrier_height,
            limits,
        };
        guard.validate_all()?;
        guard.advance_committed_frontier(committed_epoch, committed_carrier_height)?;
        Ok(guard)
    }

    fn reject_legacy_journals(store_root: &Path) -> Result<(), MergeSidecarError> {
        for legacy in LEGACY_SIGNING_GUARD_DIRS {
            let path = store_root.join(legacy);
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(MergeSidecarError::SigningGuard(error.to_string()));
                }
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(MergeSidecarError::SigningGuard(format!(
                        "unsafe legacy merge-signing journal {}",
                        path.display()
                    )));
                }
                Ok(_) => {
                    return Err(MergeSidecarError::SigningGuard(format!(
                        "legacy merge-signing journal {} requires authenticated candidate-body recovery",
                        path.display()
                    )));
                }
            }
        }
        Ok(())
    }

    fn high_water_path(directory: &Path) -> PathBuf {
        directory.join(SIGNING_GUARD_HIGH_WATER_FILE)
    }

    fn high_water_temp_path(directory: &Path) -> PathBuf {
        directory.join(SIGNING_GUARD_HIGH_WATER_TEMP)
    }

    fn remove_regular_temp_if_present(
        path: &Path,
        artifact: &str,
        max_record_bytes: usize,
    ) -> Result<(), MergeSidecarError> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(MergeSidecarError::SigningGuard(error.to_string())),
        };
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > max_record_bytes as u64
        {
            return Err(MergeSidecarError::SigningGuard(format!(
                "unsafe {artifact} signing-guard temp {}",
                path.display()
            )));
        }
        fs::remove_file(path).map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))
    }

    fn guard_directory_bytes(
        directory: &Path,
        max_total_bytes: usize,
    ) -> Result<usize, MergeSidecarError> {
        let mut total = 0_usize;
        for item in fs::read_dir(directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let metadata = fs::symlink_metadata(item.path())
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let bytes = usize::try_from(metadata.len()).map_err(|_| {
                MergeSidecarError::SigningGuard(
                    "signing-guard artifact length is not representable".to_owned(),
                )
            })?;
            total = total.checked_add(bytes).ok_or_else(|| {
                MergeSidecarError::SigningGuard(
                    "signing-guard aggregate byte count overflowed".to_owned(),
                )
            })?;
            if total > max_total_bytes {
                return Err(MergeSidecarError::SigningGuard(
                    "signing-guard aggregate bytes exceed hard limit".to_owned(),
                ));
            }
        }
        Ok(total)
    }

    fn decode_high_water(
        path: &Path,
        max_record_bytes: usize,
    ) -> Result<MergeSigningHighWaterV2, MergeSidecarError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > max_record_bytes as u64
        {
            return Err(MergeSidecarError::SigningGuard(format!(
                "unsafe signing-guard high-water file {}",
                path.display()
            )));
        }
        let bytes =
            fs::read(path).map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let high_water = norito::decode_from_bytes::<MergeSigningHighWaterV2>(&bytes)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let canonical = norito::to_bytes(&high_water)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if canonical != bytes || high_water.version != SIGNING_GUARD_VERSION {
            return Err(MergeSidecarError::SigningGuard(
                "non-canonical or unsupported signing-guard high-water".to_owned(),
            ));
        }
        Ok(high_water)
    }

    fn read_high_water(
        directory: &Path,
        max_record_bytes: usize,
    ) -> Result<Option<MergeSigningHighWaterV2>, MergeSidecarError> {
        let path = Self::high_water_path(directory);
        if !path.exists() {
            return Ok(None);
        }
        Self::decode_high_water(&path, max_record_bytes).map(Some)
    }

    fn reconcile_temps(
        directory: &Path,
        limits: MergeSigningGuardLimits,
    ) -> Result<(), MergeSidecarError> {
        Self::guard_directory_bytes(directory, limits.max_total_bytes)?;
        let high_water_temp = Self::high_water_temp_path(directory);
        // The canonical committed epoch supplied by Kura/state is the
        // authority on restart. A bounded regular temp may be partial at any
        // pre-rename crash boundary, so it is safe to discard before
        // re-publishing the canonical high-water. Symlinks and other artifact
        // types remain fail-closed.
        Self::remove_regular_temp_if_present(
            &high_water_temp,
            "high-water",
            limits.max_record_bytes,
        )?;

        for item in fs::read_dir(directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == SIGNING_GUARD_HIGH_WATER_FILE
                || name == SIGNING_GUARD_HIGH_WATER_TEMP
                || name.ends_with(&format!(".{SIGNING_GUARD_RECORD_EXT}"))
            {
                continue;
            }
            if !name.ends_with(&format!(".{SIGNING_GUARD_TEMP_EXT}")) {
                return Err(MergeSidecarError::SigningGuard(format!(
                    "unknown file in signing-guard directory: {}",
                    path.display()
                )));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || metadata.len() > limits.max_record_bytes as u64
            {
                return Err(MergeSidecarError::SigningGuard(
                    "unsafe signing-guard record temp".to_owned(),
                ));
            }
            let suffix = format!(".{SIGNING_GUARD_TEMP_EXT}");
            let stem = name.strip_suffix(&suffix).unwrap_or_default();
            if stem.len() != Hash::LENGTH * 2 || !stem.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(MergeSidecarError::SigningGuard(
                    "unknown signing-guard temp filename".to_owned(),
                ));
            }
            // A record temp is written before publication and signature
            // generation. It is therefore safe to remove whether or not the
            // identical final file exists.
            fs::remove_file(&path)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        }
        sync_directory(directory)?;
        Ok(())
    }

    fn context_hash(context: &MergeSigningContextV1) -> Hash {
        let bytes = norito::to_bytes(context)
            .expect("merge signing context must have a canonical Norito encoding");
        Hash::new_from_chunks(&[SIGNING_CONTEXT_DOMAIN, bytes.as_slice()])
    }

    fn record_path(&self, context: &MergeSigningContextV1) -> PathBuf {
        self.directory.join(format!(
            "{}.{}",
            Self::context_hash(context),
            SIGNING_GUARD_RECORD_EXT
        ))
    }

    fn decode_record_candidate(
        record: &MergeSigningGuardRecordV2,
    ) -> Result<MergeLedgerCandidate, MergeSidecarError> {
        let encoded_len = usize::try_from(record.candidate_encoded_len).map_err(|_| {
            MergeSidecarError::SigningGuard(
                "merge-signing candidate length is not representable".to_owned(),
            )
        })?;
        if encoded_len == 0
            || encoded_len > MAX_MERGE_LEDGER_ENTRY_BYTES
            || record.candidate_bytes.len() != encoded_len
        {
            return Err(MergeSidecarError::SigningGuard(
                "merge-signing candidate exceeds or differs from its exact byte bound".to_owned(),
            ));
        }
        let candidate = norito::decode_from_bytes::<MergeLedgerCandidate>(&record.candidate_bytes)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let canonical = candidate.canonical_bytes();
        if !candidate.has_current_version()
            || canonical != record.candidate_bytes
            || candidate.canonical_hash() != record.candidate_hash
            || candidate.epoch_id != record.context.epoch_id
            || candidate.view != record.context.view
            || candidate.carrier_height != record.context.carrier_height
            || candidate.carrier_parent_hash != record.context.parent_hash
        {
            return Err(MergeSidecarError::SigningGuard(
                "merge-signing candidate is non-canonical or differs from its durable context"
                    .to_owned(),
            ));
        }
        Ok(candidate)
    }

    fn read_record(
        path: &Path,
        max_record_bytes: usize,
    ) -> Result<MergeSigningGuardRecordV2, MergeSidecarError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > max_record_bytes as u64
        {
            return Err(MergeSidecarError::SigningGuard(format!(
                "unsafe signing-guard record {}",
                path.display()
            )));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        file.read_to_end(&mut bytes)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let record = norito::decode_from_bytes::<MergeSigningGuardRecordV2>(&bytes)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let canonical = norito::to_bytes(&record)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if canonical != bytes || record.version != SIGNING_GUARD_VERSION {
            return Err(MergeSidecarError::SigningGuard(
                "non-canonical or unsupported signing-guard record".to_owned(),
            ));
        }
        Self::decode_record_candidate(&record)?;
        Ok(record)
    }

    fn validate_all(&self) -> Result<(), MergeSidecarError> {
        let mut count = 0_usize;
        let mut total_bytes = 0_usize;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == SIGNING_GUARD_HIGH_WATER_FILE {
                let high_water = Self::decode_high_water(&path, self.limits.max_record_bytes)?;
                if high_water.committed_epoch != self.committed_epoch {
                    return Err(MergeSidecarError::SigningGuard(
                        "signing-guard high-water changed during validation".to_owned(),
                    ));
                }
                if high_water.committed_carrier_height != self.committed_carrier_height {
                    return Err(MergeSidecarError::SigningGuard(
                        "signing-guard carrier high-water changed during validation".to_owned(),
                    ));
                }
                continue;
            }
            if !name.ends_with(&format!(".{SIGNING_GUARD_RECORD_EXT}")) {
                return Err(MergeSidecarError::SigningGuard(format!(
                    "unknown file in signing-guard directory: {}",
                    path.display()
                )));
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let record_bytes = usize::try_from(metadata.len()).map_err(|_| {
                MergeSidecarError::SigningGuard(
                    "signing-guard record length is not representable".to_owned(),
                )
            })?;
            total_bytes = total_bytes.checked_add(record_bytes).ok_or_else(|| {
                MergeSidecarError::SigningGuard(
                    "signing-guard aggregate byte count overflowed".to_owned(),
                )
            })?;
            if total_bytes > self.limits.max_total_bytes {
                return Err(MergeSidecarError::SigningGuard(
                    "signing-guard aggregate bytes exceed hard limit".to_owned(),
                ));
            }
            count = count.saturating_add(1);
            if count > self.limits.max_records {
                return Err(MergeSidecarError::SigningGuard(
                    "signing-guard record count exceeds hard limit".to_owned(),
                ));
            }
            let record = Self::read_record(&path, self.limits.max_record_bytes)?;
            if self.record_path(&record.context) != path {
                return Err(MergeSidecarError::SigningGuard(
                    "signing-guard record path/context mismatch".to_owned(),
                ));
            }
            // A crash after the high-water rename/fsync but before record GC
            // can leave an otherwise valid stale decision. Startup completes
            // that monotonic GC in `advance_committed_frontier`; the durable
            // high-water already makes the context permanently unsignable.
        }
        Ok(())
    }

    /// Advance the durable globally committed merge-epoch high-water and only
    /// then garbage-collect signing decisions that can no longer be requested.
    #[cfg(test)]
    pub(crate) fn advance_committed_epoch(
        &mut self,
        committed_epoch: u64,
    ) -> Result<(), MergeSidecarError> {
        self.advance_committed_frontier(committed_epoch, self.committed_carrier_height)
    }

    /// Advance both irrevocable merge epoch and global carrier-height frontiers.
    pub(crate) fn advance_committed_frontier(
        &mut self,
        committed_epoch: u64,
        committed_carrier_height: u64,
    ) -> Result<(), MergeSidecarError> {
        if committed_epoch < self.committed_epoch
            || committed_carrier_height < self.committed_carrier_height
        {
            return Err(MergeSidecarError::SigningGuard(
                "attempted to regress merge signing high-water".to_owned(),
            ));
        }
        if committed_epoch > self.committed_epoch
            || committed_carrier_height > self.committed_carrier_height
        {
            let record = MergeSigningHighWaterV2 {
                version: SIGNING_GUARD_VERSION,
                committed_epoch,
                committed_carrier_height,
            };
            let bytes = norito::to_bytes(&record)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let temp = Self::high_water_temp_path(&self.directory);
            Self::remove_regular_temp_if_present(
                &temp,
                "high-water",
                self.limits.max_record_bytes,
            )?;
            {
                let total_bytes =
                    Self::guard_directory_bytes(&self.directory, self.limits.max_total_bytes)?;
                if total_bytes
                    .checked_add(bytes.len())
                    .is_none_or(|total| total > self.limits.max_total_bytes)
                {
                    return Err(MergeSidecarError::SigningGuard(
                        "signing-guard aggregate bytes reached hard limit".to_owned(),
                    ));
                }
                let mut file = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temp)
                    .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
                file.write_all(&bytes)
                    .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
                file.sync_all()
                    .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            }
            fs::rename(&temp, Self::high_water_path(&self.directory))
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            sync_directory(&self.directory)?;
            self.committed_epoch = committed_epoch;
            self.committed_carrier_height = committed_carrier_height;
        }

        let mut removed = false;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == SIGNING_GUARD_HIGH_WATER_FILE {
                continue;
            }
            if !name.ends_with(&format!(".{SIGNING_GUARD_RECORD_EXT}")) {
                return Err(MergeSidecarError::SigningGuard(format!(
                    "unexpected file during signing-guard GC: {}",
                    path.display()
                )));
            }
            let record = Self::read_record(&path, self.limits.max_record_bytes)?;
            if record.context.epoch_id <= self.committed_epoch
                || record.context.carrier_height <= self.committed_carrier_height
            {
                fs::remove_file(&path)
                    .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
                removed = true;
            }
        }
        if removed {
            sync_directory(&self.directory)?;
        }
        Ok(())
    }

    /// Durably authorize one digest, returning an error for a conflicting digest.
    pub(crate) fn authorized_digest(
        &self,
        context: &MergeSigningContextV1,
    ) -> Result<Option<Hash>, MergeSidecarError> {
        if context.epoch_id <= self.committed_epoch
            || context.carrier_height <= self.committed_carrier_height
        {
            return Ok(None);
        }
        let path = self.record_path(context);
        if !path.exists() {
            return Ok(None);
        }
        let record = Self::read_record(&path, self.limits.max_record_bytes)?;
        if &record.context != context {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record context/path mismatch".to_owned(),
            ));
        }
        Ok(Some(record.message_digest))
    }

    /// Recover the exact candidate bytes and decoded body durably paired with a digest.
    pub(crate) fn authorized_candidate(
        &self,
        context: &MergeSigningContextV1,
    ) -> Result<Option<(Hash, MergeLedgerCandidate, Vec<u8>)>, MergeSidecarError> {
        if context.epoch_id <= self.committed_epoch
            || context.carrier_height <= self.committed_carrier_height
        {
            return Ok(None);
        }
        let path = self.record_path(context);
        if !path.exists() {
            return Ok(None);
        }
        let record = Self::read_record(&path, self.limits.max_record_bytes)?;
        if &record.context != context {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record context/path mismatch".to_owned(),
            ));
        }
        let candidate = Self::decode_record_candidate(&record)?;
        Ok(Some((
            record.message_digest,
            candidate,
            record.candidate_bytes,
        )))
    }

    /// Durably authorize one exact candidate and digest, returning an error for
    /// any conflicting body, digest, or context.
    pub(crate) fn authorize(
        &self,
        context: MergeSigningContextV1,
        message_digest: Hash,
        candidate: &MergeLedgerCandidate,
    ) -> Result<(), MergeSidecarError> {
        if context.epoch_id <= self.committed_epoch
            || context.carrier_height <= self.committed_carrier_height
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        if !candidate.has_current_version()
            || candidate.epoch_id != context.epoch_id
            || candidate.view != context.view
            || candidate.carrier_height != context.carrier_height
            || candidate.carrier_parent_hash != context.parent_hash
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let candidate_bytes = candidate.canonical_bytes();
        if candidate_bytes.is_empty() || candidate_bytes.len() > MAX_MERGE_LEDGER_ENTRY_BYTES {
            return Err(MergeSidecarError::SigningGuard(
                "merge-signing candidate exceeds the shared full-entry byte limit".to_owned(),
            ));
        }
        let candidate_encoded_len = u64::try_from(candidate_bytes.len()).map_err(|_| {
            MergeSidecarError::SigningGuard(
                "merge-signing candidate length is not representable".to_owned(),
            )
        })?;
        let candidate_hash = candidate.canonical_hash();
        let record = MergeSigningGuardRecordV2 {
            version: SIGNING_GUARD_VERSION,
            context,
            message_digest,
            candidate_hash,
            candidate_encoded_len,
            candidate_bytes,
        };
        // Re-run the same canonical decoder used during startup before any
        // bytes become eligible for publication.
        Self::decode_record_candidate(&record)?;
        let bytes = norito::to_bytes(&record)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if bytes.len() > self.limits.max_record_bytes {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record exceeds hard byte limit".to_owned(),
            ));
        }
        let path = self.record_path(&record.context);
        if path.exists() {
            let existing = Self::read_record(&path, self.limits.max_record_bytes)?;
            return if existing == record {
                Ok(())
            } else {
                Err(MergeSidecarError::LocalSigningEquivocation)
            };
        }
        let mut count = 0_usize;
        let total_bytes =
            Self::guard_directory_bytes(&self.directory, self.limits.max_total_bytes)?;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == SIGNING_GUARD_HIGH_WATER_FILE {
                continue;
            }
            if name.ends_with(&format!(".{SIGNING_GUARD_RECORD_EXT}")) {
                let metadata = fs::symlink_metadata(item.path())
                    .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(MergeSidecarError::SigningGuard(
                        "unsafe signing-guard record during authorization".to_owned(),
                    ));
                }
                count = count.saturating_add(1);
                if count >= self.limits.max_records {
                    break;
                }
            } else {
                return Err(MergeSidecarError::SigningGuard(format!(
                    "unknown file in signing-guard directory: {}",
                    item.path().display()
                )));
            }
        }
        if count >= self.limits.max_records {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record count reached hard limit".to_owned(),
            ));
        }
        if total_bytes
            .checked_add(bytes.len())
            .is_none_or(|total| total > self.limits.max_total_bytes)
        {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard aggregate bytes reached hard limit".to_owned(),
            ));
        }
        let temp = path.with_extension("norito.tmp");
        Self::remove_regular_temp_if_present(
            &temp,
            "candidate-record",
            self.limits.max_record_bytes,
        )?;
        {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            file.write_all(&bytes)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            file.sync_all()
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        }
        if path.exists() {
            let _ = fs::remove_file(&temp);
            let existing = Self::read_record(&path, self.limits.max_record_bytes)?;
            return if existing == record {
                Ok(())
            } else {
                Err(MergeSidecarError::LocalSigningEquivocation)
            };
        }
        fs::rename(&temp, &path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        sync_directory(&self.directory)?;
        Ok(())
    }
}

fn ensure_regular_directory(path: &Path) -> Result<(), MergeSidecarError> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(MergeSidecarError::SigningGuard(format!(
                "unsafe signing-guard directory {}",
                path.display()
            )));
        }
    } else {
        fs::create_dir_all(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), MergeSidecarError> {
    let directory = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
    directory
        .sync_all()
        .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::merge::{MergeQuorumCertificate, MergeSignerProof};

    #[test]
    fn runtime_limit_constructors_reject_degenerate_and_overflowing_geometry() {
        use iroha_config::parameters::defaults::sumeragi as defaults;

        let valid = MergeSidecarLimits::defaults();
        assert!(MergeSidecarTransport::with_limits(1, valid).is_ok());
        assert!(MergeSidecarTransport::with_limits(usize::MAX, valid).is_err());

        let sidecar_limits = |inbound_sessions,
                              inbound_sessions_per_peer,
                              inbound_bytes,
                              inbound_bytes_per_peer,
                              deferred_blocks,
                              request_timeout,
                              outbound_sessions,
                              outbound_bytes,
                              request_gates| {
            MergeSidecarLimits::new(
                NonZeroUsize::new(inbound_sessions).expect("non-zero fixture"),
                NonZeroUsize::new(inbound_sessions_per_peer).expect("non-zero fixture"),
                NonZeroUsize::new(inbound_bytes).expect("non-zero fixture"),
                NonZeroUsize::new(inbound_bytes_per_peer).expect("non-zero fixture"),
                NonZeroUsize::new(deferred_blocks).expect("non-zero fixture"),
                defaults::V2_MERGE_SIDECAR_FUTURE_BLOCK_DISTANCE,
                request_timeout,
                NonZeroUsize::new(outbound_sessions).expect("non-zero fixture"),
                NonZeroUsize::new(outbound_bytes).expect("non-zero fixture"),
                NonZeroUsize::new(request_gates).expect("non-zero fixture"),
            )
        };
        let minimum_inbound = 2 * MAX_MERGE_LEDGER_ENTRY_BYTES;
        assert!(
            sidecar_limits(
                1,
                1,
                minimum_inbound,
                minimum_inbound,
                2,
                Duration::from_secs(1),
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                3,
                minimum_inbound,
                minimum_inbound,
                2,
                Duration::from_secs(1),
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                2,
                minimum_inbound - 1,
                minimum_inbound - 1,
                2,
                Duration::from_secs(1),
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                2,
                minimum_inbound,
                minimum_inbound,
                1,
                Duration::from_secs(1),
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                2,
                minimum_inbound,
                minimum_inbound,
                2,
                Duration::ZERO,
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                2,
                minimum_inbound,
                minimum_inbound,
                2,
                Duration::from_secs(1),
                1,
                MAX_MERGE_LEDGER_ENTRY_BYTES - 1,
                1,
            )
            .is_err()
        );
        assert!(
            sidecar_limits(
                2,
                2,
                minimum_inbound,
                minimum_inbound,
                2,
                Duration::from_secs(1),
                2,
                MAX_MERGE_LEDGER_ENTRY_BYTES,
                1,
            )
            .is_err()
        );

        let metadata_headroom =
            iroha_config::parameters::defaults::sumeragi::V2_MERGE_SIGNING_GUARD_METADATA_HEADROOM_BYTES;
        let minimum_record = MAX_MERGE_LEDGER_ENTRY_BYTES + metadata_headroom;
        assert!(
            MergeSigningGuardLimits::new(
                NonZeroUsize::new(1).expect("non-zero fixture"),
                NonZeroUsize::new(minimum_record - 1).expect("non-zero fixture"),
                NonZeroUsize::new(minimum_record + metadata_headroom).expect("non-zero fixture"),
            )
            .is_err()
        );
        assert!(
            MergeSigningGuardLimits::new(
                NonZeroUsize::new(1).expect("non-zero fixture"),
                NonZeroUsize::new(minimum_record).expect("non-zero fixture"),
                NonZeroUsize::new(minimum_record + metadata_headroom - 1)
                    .expect("non-zero fixture"),
            )
            .is_err()
        );
        assert!(
            MergeSigningGuardLimits::new(
                NonZeroUsize::new(1).expect("non-zero fixture"),
                NonZeroUsize::MAX,
                NonZeroUsize::MAX,
            )
            .is_err()
        );
    }

    #[test]
    fn server_capacity_geometry_separates_streams_gates_and_attempts() {
        let limits = MergeSidecarLimits::defaults();
        let roster_capacity = 3;
        let source_capacity = 2;
        let transport = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            roster_capacity,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("bounded split server geometry");
        let expected_gates = roster_capacity * limits.inbound_sessions_per_peer;
        assert_eq!(transport.server_stream_capacity, roster_capacity);
        assert_eq!(transport.server_request_gate_capacity, expected_gates);
        assert_eq!(
            transport.server_request_attempt_capacity,
            expected_gates * source_capacity
        );
        assert!(
            MergeSidecarTransport::with_limits_and_server_stream_capacity(
                source_capacity,
                limits,
                0,
                unbound_test_merge_sidecar_roster_digest(),
            )
            .is_err()
        );
        assert!(
            MergeSidecarTransport::with_limits_and_server_stream_capacity(
                source_capacity,
                limits,
                MAX_CERTIFIED_MERGE_SERVER_STREAMS + 1,
                unbound_test_merge_sidecar_roster_digest(),
            )
            .is_err()
        );
    }

    #[test]
    fn authenticated_source_quota_rejects_origin_churn_and_preserves_other_source() {
        let (_, _, _, base, now) = start_session(1, 1);
        let responder = base.responder.clone();
        let hub_a = peer(b"source quota hub a");
        let hub_b = peer(b"source quota hub b");
        let first_requester = peer(b"source quota first requester");
        let second_requester = peer(b"source quota second requester");
        let source_capacity = 2;
        let mut limits = MergeSidecarLimits::defaults();
        limits.outbound_sessions_per_source = 1;
        limits.server_request_gates_per_source = 2;
        let mut transport = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            2,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("bounded authenticated-source geometry");
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), source_capacity);

        for sequence in 1..=limits.server_request_gates_per_source {
            let mut request = routed_server_request(
                &base,
                first_requester.clone(),
                b"source quota first request",
                1,
            );
            request.semantic_sequence =
                semantic_sequence(u64::try_from(sequence).expect("bounded sequence"));
            request.bind_canonical_request_id();
            let route = routes.mint(first_requester.clone());
            transport
                .admit_server_request(&first_requester, &request, Some(&route), &responder, now)
                .expect("authenticated source stays inside its gate quota");
            let selected = transport
                .next_server_request_materialization(now)
                .expect("select bounded lookup")
                .expect("one retryable request is selectable");
            transport.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
        }
        let mut over_pair = routed_server_request(
            &base,
            first_requester.clone(),
            b"source quota over request",
            1,
        );
        over_pair.semantic_sequence = semantic_sequence(3);
        over_pair.bind_canonical_request_id();
        let over_route = routes.mint(first_requester.clone());
        assert!(matches!(
            transport.admit_server_request(
                &first_requester,
                &over_pair,
                Some(&over_route),
                &responder,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));

        let origin_churn = routed_server_request(
            &base,
            second_requester.clone(),
            b"source quota origin churn request",
            1,
        );
        let same_hub_route = routes.mint(second_requester.clone());
        assert!(matches!(
            transport.admit_server_request(
                &second_requester,
                &origin_churn,
                Some(&same_hub_route),
                &responder,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));

        let other_hub_route = routes.mint_via(second_requester.clone(), hub_b);
        transport
            .admit_server_request(
                &second_requester,
                &origin_churn,
                Some(&other_hub_route),
                &responder,
                now,
            )
            .expect("independent authenticated source retains its own gate quota");
        let source_a = ServerRequestSource::Authenticated(over_route.source_key());
        let source_b = ServerRequestSource::Authenticated(other_hub_route.source_key());
        assert_eq!(
            transport.source_gate_count(&source_a),
            limits.server_request_gates_per_source
        );
        assert_eq!(transport.source_gate_count(&source_b), 1);
        assert_eq!(transport.server_request_gates.len(), 3);
        assert_eq!(transport.server_gate_attempt_count(), 3);
    }

    fn peer(label: &[u8]) -> PeerId {
        PeerId::new(
            KeyPair::try_from_seed(label.to_vec(), Algorithm::BlsNormal)
                .expect("derive test key")
                .public_key()
                .clone(),
        )
    }

    fn read_lifecycle_pair(
        transport: &MergeSidecarTransport,
    ) -> (
        MergeSidecarLifecycleSnapshotV3,
        MergeSidecarLifecycleRootHighWaterV3,
    ) {
        transport
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns its lifecycle journal")
            .load_pair_strict()
            .expect("read an exact lifecycle state/root pair")
    }

    fn install_lifecycle_pair(
        journal: &MergeSidecarLifecycleJournal,
        mut snapshot: MergeSidecarLifecycleSnapshotV3,
    ) -> MergeSidecarLifecycleRootHighWaterV3 {
        snapshot.payload_hash = HashOf::new(&snapshot.payload);
        let marker = MergeSidecarLifecycleRootHighWaterV3::new(&snapshot);
        fs::write(
            journal.state_path_for_generation(snapshot.payload.root_generation),
            norito::to_bytes(&snapshot).expect("encode lifecycle test state"),
        )
        .expect("install lifecycle test state");
        MergeSidecarLifecycleJournal::sync_directory(&journal.directory)
            .expect("sync lifecycle test state");
        fs::write(
            journal.root_high_water_path(),
            norito::to_bytes(&marker).expect("encode lifecycle test root high-water"),
        )
        .expect("install lifecycle test root high-water");
        MergeSidecarLifecycleJournal::sync_directory(&journal.store_root)
            .expect("sync lifecycle test root high-water");
        marker
    }

    fn stream_epoch(value: u64) -> CertifiedMergeSidecarStreamEpochV1 {
        CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(value).expect("test stream epoch must be non-zero"),
        )
    }

    fn service_generation(value: u64) -> CertifiedMergeSidecarServiceGenerationV1 {
        CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(value).expect("test service generation must be non-zero"),
        )
    }

    fn semantic_sequence(value: u64) -> CertifiedMergeSidecarSemanticSequenceV1 {
        CertifiedMergeSidecarSemanticSequenceV1(
            NonZeroU64::new(value).expect("test semantic sequence must be non-zero"),
        )
    }

    #[test]
    fn semantic_sequence_norito_decode_rejects_zero() {
        let mut bytes = semantic_sequence(1).encode();
        let payload = bytes
            .len()
            .checked_sub(std::mem::size_of::<u64>())
            .expect("encoded semantic sequence contains its u64 payload");
        bytes[payload..].fill(0);

        assert!(matches!(
            CertifiedMergeSidecarSemanticSequenceV1::decode(&mut bytes.as_slice()),
            Err(norito::core::Error::InvalidNonZero)
        ));
    }

    fn successor_stream_epoch(
        epoch: CertifiedMergeSidecarStreamEpochV1,
    ) -> CertifiedMergeSidecarStreamEpochV1 {
        stream_epoch(
            epoch
                .get()
                .checked_add(1)
                .expect("test stream epoch has a successor"),
        )
    }

    fn generation_hint_for_request(
        request: &CertifiedMergeSidecarRequestV1,
        current_generation: CertifiedMergeSidecarServiceGenerationV1,
    ) -> CertifiedMergeSidecarGenerationHintV1 {
        let mut hint = CertifiedMergeSidecarGenerationHintV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            observed_generation: request.service_generation,
            current_generation,
            observed_message_hash: HashOf::new(request).into(),
            hint_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: request.requester.clone(),
            responder: request.responder.clone(),
        };
        hint.bind_canonical_hint_id();
        hint
    }

    fn close_for_request(request: &CertifiedMergeSidecarRequestV1) -> CertifiedMergeSidecarCloseV1 {
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: request.version,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: request.requester.clone(),
            responder: request.responder.clone(),
        };
        close.bind_canonical_close_id();
        close
    }

    fn signing_candidate(context: &MergeSigningContextV1, label: &[u8]) -> MergeLedgerCandidate {
        MergeLedgerCandidate {
            version: MergeLedgerCandidate::VERSION,
            epoch_id: context.epoch_id,
            view: context.view,
            carrier_height: context.carrier_height,
            carrier_parent_hash: context.parent_hash,
            lane_catalog_hash: Hash::new_from_chunks(&[b"catalog", label]),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new_from_chunks(&[b"incarnations", label]),
            activation_root: Hash::new_from_chunks(&[b"activations", label]),
            lane_snapshots: Vec::new(),
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
            queue_plan_admissions: Vec::new(),
            global_state_root: Hash::new_from_chunks(&[b"state", label]),
        }
    }

    fn reference(encoded_len: usize, holders: usize) -> CertifiedMergeLedgerReference {
        let validator_set = (0..holders)
            .map(|index| peer(format!("holder-{index}").as_bytes()))
            .collect::<Vec<_>>();
        let bitmap_len = validator_set.len().div_ceil(8);
        let mut bitmap = vec![0_u8; bitmap_len];
        for index in 0..validator_set.len() {
            bitmap[index / 8] |= 1 << (index % 8);
        }
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"entry")),
            encoded_len: encoded_len as u64,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                3,
                7,
                2,
                HashOf::from_untyped_unchecked(Hash::new(b"carrier-parent")),
                Hash::new(b"chain"),
                1,
                HashOf::new(&validator_set),
                validator_set,
                bitmap,
                Vec::<MergeSignerProof>::new(),
                vec![0; 96],
                Hash::new(b"message"),
            ),
        }
    }

    fn start_session(
        len: usize,
        holders: usize,
    ) -> (
        MergeSidecarTransport,
        PeerId,
        CertifiedMergeLedgerReference,
        CertifiedMergeSidecarRequestV1,
        Instant,
    ) {
        let now = Instant::now();
        let requester = peer(b"requester");
        let reference = reference(len, holders);
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(b"block"));
        let mut transport = MergeSidecarTransport::new();
        let post = transport
            .defer_block(block_hash, 2, 0, reference.clone(), &requester, 1, now)
            .expect("defer")
            .expect("request");
        let CertifiedMergeSidecarMessage::Request(request) = Arc::unwrap_or_clone(post.message)
        else {
            panic!("expected request")
        };
        (transport, requester, reference, request, now)
    }

    fn chunks(
        request: &CertifiedMergeSidecarRequestV1,
        bytes: &[u8],
    ) -> Vec<CertifiedMergeSidecarChunkV1> {
        let count = bytes.len().div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        bytes
            .chunks(MAX_CERTIFIED_MERGE_CHUNK_BYTES)
            .enumerate()
            .map(|(index, chunk)| CertifiedMergeSidecarChunkV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation: request.service_generation,
                stream_epoch: request.stream_epoch,
                semantic_sequence: request.semantic_sequence,
                request_id: request.request_id,
                entry_hash: request.entry_hash,
                encoded_len: request.encoded_len,
                epoch_id: request.epoch_id,
                reference_digest: request.reference_digest,
                requester: request.requester.clone(),
                responder: request.responder.clone(),
                chunk_index: index as u32,
                chunk_count: count as u32,
                bytes: chunk.to_vec(),
            })
            .collect()
    }

    fn reply_chunk_admission(post: &MergeSidecarPost) -> CertifiedMergeSidecarChunkAdmission {
        reply_chunk_admission_at_attempt(post, 0)
    }

    fn reply_chunk_admission_at_attempt(
        post: &MergeSidecarPost,
        reply_writer_timeout_attempt: u8,
    ) -> CertifiedMergeSidecarChunkAdmission {
        let route = post
            .reply_route
            .as_ref()
            .expect("response chunk must retain its authenticated reply route");
        let CertifiedMergeSidecarMessage::Chunk(_) = post.message.as_ref() else {
            panic!("expected a certified merge-sidecar response chunk")
        };
        let canonical_post = Post {
            data: crate::NetworkMessage::CertifiedMergeSidecar(Arc::clone(&post.message)),
            peer_id: post.peer.clone(),
            priority: Priority::High,
        };
        let (mut flush_control, flush_ack) = NetworkReplyFlushAckTestFixture::for_reply_at_attempt(
            &canonical_post,
            route,
            reply_writer_timeout_attempt,
        );
        assert!(flush_control.flush(), "publish exact test writer flush");
        let mut admission = CertifiedMergeSidecarChunkAdmission::from_admitted_reply(
            &canonical_post,
            route,
            0,
            1,
            flush_ack.identity(),
        )
        .expect("bind exact admitted response chunk");
        let trace = crate::sumeragi::v2_worker::reliable_flush_trace_projection(
            &admission,
            iroha_p2p::network::NetworkReplyFlushAckStatus::Flushed,
            1,
            0,
            0,
            1,
            1,
        )
        .expect("project a successful worker flush for the test admission");
        assert!(production_reliable_flush_trace_refines_outbound_ownership_kernel(trace));
        admission
            .bind_confirmed_worker_trace(trace)
            .expect("bind the kernel-accepted test worker transition once");
        admission
    }

    fn acknowledge_reply_chunk(
        server: &mut MergeSidecarTransport,
        post: &MergeSidecarPost,
        now: Instant,
    ) -> bool {
        server
            .acknowledge_outbound_chunk(&reply_chunk_admission(post), now)
            .expect("acknowledge exact admitted response chunk")
    }

    fn routed_server_request(
        base: &CertifiedMergeSidecarRequestV1,
        requester: PeerId,
        _request_label: &[u8],
        encoded_len: usize,
    ) -> CertifiedMergeSidecarRequestV1 {
        let mut request = base.clone();
        request.requester = requester;
        request.semantic_sequence = semantic_sequence(1);
        request.closed_through = 0;
        request.encoded_len = encoded_len as u64;
        request.bind_canonical_request_id();
        request
    }

    #[test]
    fn responder_roster_digest_is_order_and_duplicate_independent() {
        let first = peer(b"canonical roster first");
        let second = peer(b"canonical roster second");
        let replacement = peer(b"canonical roster replacement");
        assert_eq!(
            canonical_merge_sidecar_roster_digest(
                &[second.clone(), first.clone(), second.clone(),]
            ),
            canonical_merge_sidecar_roster_digest(&[first.clone(), second.clone()])
        );
        assert_ne!(
            canonical_merge_sidecar_roster_digest(&[first.clone(), second]),
            canonical_merge_sidecar_roster_digest(&[first, replacement])
        );
    }

    #[test]
    fn same_roster_identity_preserves_server_state_and_changed_size_rolls_once() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let local_peer = request.responder.clone();
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let initial_roster = vec![requester.clone()];
        let grown_roster = vec![
            requester.clone(),
            peer(b"grown roster second"),
            peer(b"grown roster third"),
        ];
        let shrunk_roster = vec![requester.clone(), peer(b"shrunk roster replacement")];
        let initial_digest = canonical_merge_sidecar_roster_digest(&initial_roster);
        let grown_digest = canonical_merge_sidecar_roster_digest(&grown_roster);
        let shrunk_digest = canonical_merge_sidecar_roster_digest(&shrunk_roster);
        let mut transport = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            initial_roster.len(),
            initial_digest.clone(),
        )
        .expect("construct the initial roster geometry");
        assert!(matches!(
            transport
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("admit one initial-roster request"),
            ServerRequestAdmission::Materialize
        ));

        let mut transport = transport
            .rehydrate_with_exact_geometry(
                source_capacity,
                limits,
                initial_roster.len(),
                initial_digest,
                now,
            )
            .expect("an equal roster identity retains process-local state");
        assert_eq!(
            transport.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(transport.server_streams.len(), 1);
        assert_eq!(transport.server_request_gates.len(), 1);

        transport.cancel_unmaterialized_server_request(&requester, &request);
        assert!(matches!(
            transport
                .transition_server_service_generation(grown_roster.len(), grown_digest.clone(),),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        let close = close_for_request(&request);
        transport
            .admit_server_close(&requester, &close, None, &local_peer)
            .expect("the exact close makes the old roster stream terminal");
        assert_eq!(transport.drain_closed_server_prefixes().len(), 1);
        transport.confirm_closed_server_prefix_handoff();
        let transport = transport
            .rehydrate_with_exact_geometry(
                source_capacity,
                limits,
                grown_roster.len(),
                grown_digest,
                now,
            )
            .expect("growing to a different roster advances one generation");
        assert_eq!(transport.server_service_generation, service_generation(2));
        assert_eq!(transport.server_stream_capacity, grown_roster.len());
        assert_eq!(
            transport.server_request_gate_capacity,
            grown_roster.len() * limits.inbound_sessions_per_peer
        );
        assert_eq!(
            transport.server_request_attempt_capacity,
            grown_roster.len() * limits.inbound_sessions_per_peer * source_capacity
        );
        assert!(transport.server_streams.is_empty());
        assert!(transport.server_request_gates.is_empty());

        let mut transport = transport;
        assert_eq!(transport.drain_closed_server_prefixes().len(), 1);
        transport.confirm_closed_server_prefix_handoff();
        let transport = transport
            .rehydrate_with_exact_geometry(
                source_capacity,
                limits,
                shrunk_roster.len(),
                shrunk_digest.clone(),
                now,
            )
            .expect("shrinking to another roster advances exactly once");
        assert_eq!(transport.server_service_generation, service_generation(3));
        assert_eq!(transport.server_stream_capacity, shrunk_roster.len());
        assert_eq!(
            transport.server_request_gate_capacity,
            shrunk_roster.len() * limits.inbound_sessions_per_peer
        );
        assert_eq!(
            transport.server_request_attempt_capacity,
            shrunk_roster.len() * limits.inbound_sessions_per_peer * source_capacity
        );

        assert!(matches!(
            transport
                .rehydrate_with_exact_geometry(source_capacity, limits, 1, shrunk_digest, now,),
            Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height roster capacity drift"
            ))
        ));
    }

    #[test]
    fn same_cardinality_roster_replacement_reclaims_inactive_output_and_preserves_requester_state()
    {
        let (_, _, _, base, now) = start_session(1, 1);
        let local_peer = base.responder.clone();
        let stable = peer(b"same-cardinality stable roster peer");
        let retained_requester = stable.clone();
        let retired_roster_peer = peer(b"same-cardinality retired roster peer");
        let replacement = peer(b"same-cardinality replacement roster peer");
        let old_roster = vec![stable.clone(), retired_roster_peer];
        let new_roster = vec![stable, replacement];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        assert_ne!(old_digest, new_digest);

        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            old_roster.len(),
            old_digest,
        )
        .expect("construct the old same-cardinality roster");

        let requester_side_reference = reference(1, 1);
        let requester_side_key = (
            requester_side_reference.entry_hash,
            certified_merge_reference_digest(&requester_side_reference),
        );
        let requester_side_local_peer = peer(b"requester-side local peer");
        let requester_side_post = server
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"same-cardinality requester-side block")),
                2,
                0,
                requester_side_reference,
                &requester_side_local_peer,
                1,
                now,
            )
            .expect("retain an inbound requester-side assembly")
            .expect("emit its first request");
        let CertifiedMergeSidecarMessage::Request(requester_side_request) =
            Arc::unwrap_or_clone(requester_side_post.message)
        else {
            panic!("requester-side state emits a request")
        };
        let requester_streams_before = server
            .lifecycle_snapshot()
            .expect("snapshot requester-side lifecycle")
            .payload
            .request_streams;
        let next_stream_epoch_before = server.next_stream_epoch;

        let request = routed_server_request(
            &base,
            retained_requester.clone(),
            b"same-cardinality old request",
            1,
        );
        let hub = peer(b"same-cardinality reply hub");
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub, source_capacity);
        let route = routes.mint(retained_requester.clone());
        assert!(matches!(
            server
                .admit_server_request(
                    &retained_requester,
                    &request,
                    Some(&route),
                    &local_peer,
                    now,
                )
                .expect("admit output under the old roster"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0x51], now)
            .expect("retain one active old-roster output");
        assert_eq!(server.outbound.len(), 1);
        assert!(routes.retire(&route));
        assert!(matches!(
            server.transition_server_service_generation(new_roster.len(), new_digest.clone(),),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(server.outbound.len(), 1);

        let close = close_for_request(&request);
        server
            .admit_server_close(&retained_requester, &close, None, &local_peer)
            .expect("an authenticated close terminally releases inactive old-roster output");
        assert_eq!(server.drain_closed_server_prefixes().len(), 1);
        server.confirm_closed_server_prefix_handoff();

        let mut transitioned = server
            .rehydrate_with_exact_geometry(
                source_capacity,
                limits,
                new_roster.len(),
                new_digest.clone(),
                now,
            )
            .expect("closed inactive output permits the roster transition");
        assert_eq!(
            transitioned.server_service_generation,
            service_generation(2)
        );
        assert_eq!(transitioned.server_roster_digest, new_digest);
        assert_eq!(transitioned.server_stream_capacity, old_roster.len());
        assert!(transitioned.server_streams.is_empty());
        assert!(transitioned.server_request_gates.is_empty());
        assert!(transitioned.outbound.is_empty());
        assert!(transitioned.outbound_order.is_empty());
        assert_eq!(transitioned.next_stream_epoch, next_stream_epoch_before);
        assert_eq!(
            transitioned
                .lifecycle_snapshot()
                .expect("snapshot transitioned requester-side lifecycle")
                .payload
                .request_streams,
            requester_streams_before
        );
        let retained_inbound = transitioned
            .inbound
            .get(&requester_side_key)
            .expect("roster transition preserves the inbound assembly");
        assert_eq!(
            retained_inbound.current.as_ref().map(|attempt| attempt.id),
            Some(requester_side_request.request_id)
        );

        assert_eq!(
            transitioned.drain_closed_server_prefixes(),
            vec![CertifiedMergeSidecarClosedPrefix {
                requester: retained_requester.clone(),
                service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
                stream_epoch: request.stream_epoch,
                closed_through: request.semantic_sequence.get(),
            }]
        );
        let mut stale_routes =
            NetworkReplyRouteTestFixture::new(peer(b"roster transition stale reply hub"));
        let stale_route = stale_routes.mint(retained_requester.clone());
        let admission = transitioned
            .admit_server_request(
                &retained_requester,
                &request,
                Some(&stale_route),
                &local_peer,
                now,
            )
            .expect("the old-roster request receives the successor fence");
        let ServerRequestAdmission::GenerationHint(post) = admission else {
            panic!("stale old-roster traffic must receive a generation Hint")
        };
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = post.message.as_ref() else {
            panic!("stale old-roster traffic must not allocate output")
        };
        assert_eq!(hint.observed_generation, request.service_generation);
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(hint.observed_message_hash, HashOf::new(&request).into());
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&stale_route))
        );
        assert!(transitioned.server_streams.is_empty());
        assert!(transitioned.server_request_gates.is_empty());
        assert!(transitioned.outbound.is_empty());
    }

    #[test]
    fn roster_transition_rejects_authorized_or_active_output() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let local_peer = request.responder.clone();
        let old_roster = vec![requester.clone()];
        let new_roster = vec![peer(b"active-output replacement requester")];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
            peer(b"active-output reply hub"),
            source_capacity,
        );
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            old_roster.len(),
            old_digest.clone(),
        )
        .expect("construct the active-output roster fixture");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                .expect("authorize one old-roster lookup"),
            ServerRequestAdmission::Materialize
        ));

        assert!(matches!(
            server.transition_server_service_generation(new_roster.len(), new_digest.clone()),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(server.server_roster_digest, old_digest);
        assert_eq!(server.server_request_gates.len(), 1);
        assert!(server.outbound.is_empty());

        server
            .enqueue_response(request, Some(route), vec![0x61], now)
            .expect("replace lookup authority with active output");
        assert!(matches!(
            server.transition_server_service_generation(new_roster.len(), new_digest.clone()),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(server.server_roster_digest, old_digest);
        assert_eq!(server.outbound.len(), 1);
        assert!(matches!(
            server.transition_server_service_generation_after_exact_output_fence(
                new_roster.len(),
                new_digest,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("requires a durable lifecycle journal")
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(server.server_roster_digest, old_digest);
        assert_eq!(server.server_request_gates.len(), 1);
        assert_eq!(server.outbound.len(), 1);
    }

    #[test]
    fn durable_roster_replacement_restores_prior_geometry_then_fences_once() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let retained_requester = peer(b"durable retained roster requester");
        let old_roster = vec![
            retained_requester.clone(),
            peer(b"durable retired roster requester"),
        ];
        let new_roster = vec![
            retained_requester.clone(),
            peer(b"durable replacement roster requester"),
            peer(b"durable grown roster requester"),
        ];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let mut server = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            old_roster.len(),
            old_digest.clone(),
        )
        .expect("open the old durable roster");

        let requester_side_local_peer = peer(b"durable roster requester-side local peer");
        let requester_side_post = server
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"durable roster requester-side block")),
                2,
                0,
                reference(1, 1),
                &requester_side_local_peer,
                1,
                now,
            )
            .expect("persist requester-side lifecycle")
            .expect("emit the requester-side occurrence");
        let CertifiedMergeSidecarMessage::Request(requester_side_request) =
            Arc::unwrap_or_clone(requester_side_post.message)
        else {
            panic!("requester-side lifecycle emits a request")
        };
        server
            .release_unsent_request(&requester_side_request)
            .expect("persist a terminal requester-side occurrence");
        let requester_streams_before = server
            .lifecycle_snapshot()
            .expect("snapshot requester-side lifecycle before restart")
            .payload
            .request_streams;

        let (_, _, _, base, _) = start_session(1, 1);
        let local_peer = base.responder.clone();
        let old_request = routed_server_request(
            &base,
            retained_requester.clone(),
            b"durable old-roster request",
            1,
        );
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
            peer(b"durable roster reply hub"),
            source_capacity,
        );
        let route = routes.mint(retained_requester.clone());
        assert!(matches!(
            server
                .admit_server_request(
                    &retained_requester,
                    &old_request,
                    Some(&route),
                    &local_peer,
                    now,
                )
                .expect("persist the old-roster request gate"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist all old-roster lifecycle state");
        let predecessor_snapshot = server
            .lifecycle_snapshot()
            .expect("snapshot active predecessor ownership");
        let predecessor_root_path = server.lifecycle_root_high_water_path_for_test();
        let predecessor_root =
            fs::read(&predecessor_root_path).expect("read the active predecessor V3 root");
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            new_roster.len(),
            new_digest.clone(),
        )
        .expect("restart fences active predecessor ownership into the certified new roster");
        assert_eq!(restarted.server_service_generation, service_generation(2));
        assert_eq!(restarted.server_roster_digest, new_digest.clone());
        assert_eq!(restarted.server_stream_capacity, new_roster.len());
        assert!(restarted.server_streams.is_empty());
        assert!(restarted.server_request_gates.is_empty());
        assert!(restarted.outbound.is_empty());
        assert!(restarted.outbound_order.is_empty());
        assert!(restarted.pending_server_closures.is_empty());
        assert!(!restarted.server_closure_handoff_pending);
        let successor_snapshot = restarted
            .lifecycle_snapshot()
            .expect("snapshot the new roster");
        assert_ne!(
            successor_snapshot, predecessor_snapshot,
            "changed-roster restart publishes a new durable generation fence"
        );
        assert_ne!(
            fs::read(&predecessor_root_path).expect("read successor V3 root"),
            predecessor_root
        );
        assert_eq!(
            successor_snapshot.payload.request_streams,
            requester_streams_before
        );
        assert_eq!(
            restarted
                .lifecycle_journal
                .as_ref()
                .expect("restarted transport retains its journal")
                .load()
                .expect("load the roster-transition snapshot")
                .expect("the roster-transition snapshot is durable"),
            successor_snapshot
        );
        let (_, protocol_max_attempts) = MergeSidecarTransport::derive_server_request_capacities(
            source_capacity,
            limits,
            MAX_CERTIFIED_MERGE_SERVER_STREAMS,
        )
        .expect("derive the protocol-maximum request geometry");
        assert_eq!(
            restarted
                .lifecycle_journal
                .as_ref()
                .expect("restarted transport retains its journal")
                .max_snapshot_bytes,
            MergeSidecarTransport::lifecycle_max_snapshot_bytes_for_attempt_capacity(
                protocol_max_attempts,
            )
            .expect("derive the protocol-maximum lifecycle bound")
        );
        assert!(
            restarted.drain_closed_server_prefixes().is_empty(),
            "forced fencing must not forge requester-authenticated close prefixes"
        );

        let admission = restarted
            .admit_server_request(
                &retained_requester,
                &old_request,
                Some(&route),
                &local_peer,
                now,
            )
            .expect("stale durable old-roster traffic receives a Hint");
        let ServerRequestAdmission::GenerationHint(post) = admission else {
            panic!("stale predecessor traffic must receive the successor fence")
        };
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = post.message.as_ref() else {
            panic!("successor fence must be encoded as a GenerationHint")
        };
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(hint.observed_message_hash, HashOf::new(&old_request).into());
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|retained| retained.same_delivery(&route)),
            "stale traffic retains its exact triggering reply delivery"
        );
        assert!(restarted.server_streams.is_empty());
        assert!(restarted.server_request_gates.is_empty());
        drop(restarted);

        let restarted_again = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            new_roster.len(),
            new_digest,
        )
        .expect("an equal-roster restart must not roll again");
        assert_eq!(
            restarted_again.server_service_generation,
            service_generation(2)
        );
        assert_eq!(
            restarted_again
                .lifecycle_snapshot()
                .expect("snapshot the equal-roster restart")
                .payload
                .request_streams,
            requester_streams_before
        );
    }

    #[test]
    fn durable_same_roster_capacity_upgrade_is_monotonic_and_preserves_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let (_, requester, _, request, _) = start_session(1, 1);
        let responder = request.responder.clone();
        let roster = vec![
            requester.clone(),
            peer(b"capacity-upgrade roster second"),
            peer(b"capacity-upgrade roster third"),
            peer(b"capacity-upgrade roster fourth"),
        ];
        let roster_digest = canonical_merge_sidecar_roster_digest(&roster);
        let predecessor_capacity = roster.len();
        let upgraded_capacity = predecessor_capacity
            .checked_add(MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)
            .expect("two-committee responder geometry is representable");
        let mut predecessor = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            predecessor_capacity,
            roster_digest.clone(),
        )
        .expect("open the predecessor V3 roster geometry");
        assert!(matches!(
            predecessor
                .admit_server_request(&requester, &request, None, &responder, now)
                .expect("persist one predecessor responder stream"),
            ServerRequestAdmission::Materialize
        ));
        predecessor
            .persist_lifecycle_state()
            .expect("persist the predecessor V3 snapshot");
        let predecessor_snapshot = predecessor
            .lifecycle_snapshot()
            .expect("snapshot the predecessor geometry");
        assert_eq!(
            predecessor_snapshot.payload.geometry.server_stream_capacity,
            u64::try_from(predecessor_capacity).expect("test capacity fits u64")
        );
        drop(predecessor);

        let upgraded = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            upgraded_capacity,
            roster_digest.clone(),
        )
        .expect("same-roster restart monotonically expands the V3 geometry");
        assert_eq!(
            upgraded.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            "a capacity-only migration must not create a responder generation"
        );
        assert_eq!(upgraded.server_roster_digest, roster_digest.clone());
        assert_eq!(upgraded.server_stream_capacity, upgraded_capacity);
        assert_eq!(
            upgraded.server_request_gate_capacity,
            upgraded_capacity * limits.inbound_sessions_per_peer
        );
        assert_eq!(
            upgraded.server_request_attempt_capacity,
            upgraded_capacity * limits.inbound_sessions_per_peer * source_capacity
        );
        assert_eq!(upgraded.server_streams.len(), 1);
        assert!(upgraded.server_streams.contains_key(&requester));
        assert_eq!(upgraded.server_request_gates.len(), 1);
        assert!(
            upgraded
                .server_request_gates
                .contains_key(&(requester.clone(), request.request_id))
        );
        let upgraded_snapshot = upgraded
            .lifecycle_snapshot()
            .expect("snapshot the upgraded geometry");
        assert_eq!(
            upgraded_snapshot.payload.root_generation,
            predecessor_snapshot
                .payload
                .root_generation
                .checked_add(1)
                .expect("test lifecycle generation remains representable")
        );
        assert_eq!(
            upgraded_snapshot.payload.geometry.server_stream_capacity,
            u64::try_from(upgraded_capacity).expect("test capacity fits u64")
        );
        assert_eq!(
            upgraded_snapshot.payload.server_streams, predecessor_snapshot.payload.server_streams,
            "monotonic expansion preserves semantic responder ownership"
        );
        assert_eq!(
            upgraded_snapshot.payload.server_request_gates,
            predecessor_snapshot.payload.server_request_gates,
            "monotonic expansion preserves exact request ownership"
        );
        assert_eq!(
            upgraded
                .lifecycle_journal
                .as_ref()
                .expect("upgraded transport retains its journal")
                .load()
                .expect("load the upgraded V3 pair")
                .expect("the upgraded V3 snapshot is durable"),
            upgraded_snapshot
        );
        drop(upgraded);

        assert!(matches!(
            MergeSidecarTransport::open_durable_with_server_stream_capacity(
                temp.path(),
                source_capacity,
                limits,
                predecessor_capacity,
                roster_digest.clone(),
            ),
            Err(MergeSidecarError::Capacity(
                "merge-sidecar retained-height roster capacity drift"
            ))
        ));
        let reopened = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            upgraded_capacity,
            roster_digest,
        )
        .expect("reopen the durable upgraded geometry after rejected shrink");
        assert_eq!(
            reopened
                .lifecycle_snapshot()
                .expect("snapshot the reopened upgraded geometry"),
            upgraded_snapshot,
            "a rejected shrink must not rewrite the durable upgraded snapshot"
        );
    }

    #[test]
    fn durable_exact_output_fence_clears_unconfirmed_debt_without_forging_close() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let (_, requester, _, request, _) = start_session(1, 1);
        let responder = request.responder.clone();
        let old_roster = vec![requester.clone()];
        let new_roster = vec![peer(b"authority-fenced replacement requester")];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
            peer(b"authority-fenced reply hub"),
            source_capacity,
        );
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            old_roster.len(),
            old_digest,
        )
        .expect("open authority-fenced durable responder");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now,)
                .expect("retain one active predecessor occurrence"),
            ServerRequestAdmission::Materialize
        ));
        server.record_server_closure(
            &requester,
            request.service_generation,
            request.stream_epoch,
            request.semantic_sequence.get(),
        );
        server.server_closure_handoff_pending = true;

        server
            .transition_server_service_generation_after_exact_output_fence(
                new_roster.len(),
                new_digest.clone(),
            )
            .expect("durable exact-output fence supersedes requester-controlled debt");
        assert_eq!(server.server_service_generation, service_generation(2));
        assert_eq!(server.server_roster_digest, new_digest);
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert!(server.pending_server_closures.is_empty());
        assert!(!server.server_closure_handoff_pending);
        assert!(
            server.drain_closed_server_prefixes().is_empty(),
            "supersession cannot convert an unconfirmed occurrence into authenticated closure"
        );

        let ServerRequestAdmission::GenerationHint(post) = server
            .admit_server_request(&requester, &request, Some(&route), &responder, now)
            .expect("stale occurrence receives the durable successor fence")
        else {
            panic!("stale predecessor request must receive a GenerationHint")
        };
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|retained| retained.same_delivery(&route))
        );
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = post.message.as_ref() else {
            panic!("successor fence must be a GenerationHint")
        };
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(hint.observed_generation, request.service_generation);
    }

    #[test]
    fn durable_exact_output_fence_rejects_service_generation_exhaustion_before_mutation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let old_roster = vec![peer(b"maximum service generation requester")];
        let new_roster = vec![peer(b"maximum service generation replacement")];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let mut server = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            old_roster.len(),
            old_digest.clone(),
        )
        .expect("open maximum service-generation fixture");
        server.server_service_generation = service_generation(u64::MAX);
        server
            .persist_lifecycle_state()
            .expect("persist the maximum service generation");
        let state_path = server.lifecycle_journal_state_path_for_test();
        let root_path = server.lifecycle_root_high_water_path_for_test();
        let state_bytes = fs::read(&state_path).expect("read maximum-generation state");
        let root_bytes = fs::read(&root_path).expect("read maximum-generation root");

        assert!(matches!(
            server.transition_server_service_generation_after_exact_output_fence(
                new_roster.len(),
                new_digest,
            ),
            Err(MergeSidecarError::Capacity(
                "server service generation exhausted"
            ))
        ));
        assert_eq!(
            server.server_service_generation,
            service_generation(u64::MAX)
        );
        assert_eq!(server.server_roster_digest, old_digest);
        assert_eq!(
            fs::read(state_path).expect("reread unchanged maximum-generation state"),
            state_bytes
        );
        assert_eq!(
            fs::read(root_path).expect("reread unchanged maximum-generation root"),
            root_bytes
        );
    }

    #[test]
    fn durable_roster_transition_failure_preserves_predecessor_then_commits_successor() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let old_roster = vec![peer(b"atomic old roster requester")];
        let new_roster = vec![peer(b"atomic new roster requester")];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let mut server = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            old_roster.len(),
            old_digest.clone(),
        )
        .expect("open the durable atomic roster fixture");
        let (_, _, _, base, now) = start_session(1, 1);
        let responder = base.responder.clone();
        let active_request = routed_server_request(
            &base,
            old_roster[0].clone(),
            b"atomic active predecessor request",
            1,
        );
        assert!(matches!(
            server
                .admit_server_request(&old_roster[0], &active_request, None, &responder, now,)
                .expect("persist active predecessor ownership"),
            ServerRequestAdmission::Materialize
        ));
        let predecessor_snapshot = server
            .lifecycle_snapshot()
            .expect("snapshot the predecessor roster");
        let state_temp_path = server
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal")
            .temp_path();
        server.obstruct_lifecycle_journal_temp_for_test();

        assert!(matches!(
            server.transition_server_service_generation_after_exact_output_fence(
                new_roster.len(),
                new_digest.clone(),
            ),
            Err(MergeSidecarError::LifecycleJournal(_))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(server.server_roster_digest, old_digest);
        assert_eq!(server.server_streams.len(), 1);
        assert_eq!(server.server_request_gates.len(), 1);
        assert!(matches!(
            server
                .lifecycle_journal
                .as_ref()
                .expect("failed transition retains its journal")
                .load(),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unsafe lifecycle journal temp artifact")
        ));
        fs::remove_dir(state_temp_path).expect("remove the injected state obstruction");
        assert_eq!(
            server
                .lifecycle_journal
                .as_ref()
                .expect("failed transition retains its journal")
                .load()
                .expect("load the durable predecessor")
                .expect("the predecessor snapshot remains durable"),
            predecessor_snapshot
        );
        drop(server);

        let restarted = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            new_roster.len(),
            new_digest.clone(),
        )
        .expect("restart from the predecessor and commit the complete successor");
        let successor_snapshot = restarted
            .lifecycle_snapshot()
            .expect("snapshot the successor roster");
        assert_eq!(
            successor_snapshot.payload.server_service_generation,
            service_generation(2)
        );
        assert_eq!(
            successor_snapshot.payload.geometry.server_roster_digest,
            new_digest
        );
        assert!(successor_snapshot.payload.server_streams.is_empty());
        assert!(successor_snapshot.payload.server_request_gates.is_empty());
        assert!(restarted.pending_server_closures.is_empty());
        assert_eq!(
            restarted
                .lifecycle_journal
                .as_ref()
                .expect("restarted transition retains its journal")
                .load()
                .expect("load the durable successor")
                .expect("the successor snapshot is durable"),
            successor_snapshot
        );
    }

    #[test]
    fn durable_roster_change_rejects_non_roster_geometry_drift() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let old_roster = vec![peer(b"geometry-drift old roster requester")];
        let new_roster = vec![peer(b"geometry-drift new roster requester")];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let mut server = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            source_capacity,
            limits,
            old_roster.len(),
            old_digest,
        )
        .expect("open the durable geometry-drift fixture");
        let journal = server
            .lifecycle_journal
            .as_mut()
            .expect("durable fixture owns its journal");
        let snapshot = journal
            .load()
            .expect("load the valid roster snapshot")
            .expect("the initialized journal has a snapshot");
        let mut snapshot = snapshot;
        snapshot.payload.geometry.runtime.future_block_distance = snapshot
            .payload
            .geometry
            .runtime
            .future_block_distance
            .checked_add(1)
            .expect("test geometry remains representable");
        snapshot.payload_hash = HashOf::new(&snapshot.payload);
        journal
            .persist_next(snapshot)
            .expect("persist canonical non-roster geometry corruption");
        drop(server);

        assert!(matches!(
            MergeSidecarTransport::open_durable_with_server_stream_capacity(
                temp.path(),
                source_capacity,
                limits,
                new_roster.len(),
                new_digest,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("non-roster geometry drift")
        ));
    }

    #[test]
    fn holder_derivation_rejects_noncanonical_qc_rosters_and_bitmaps() {
        let canonical = reference(1, 3);
        assert_eq!(
            certified_merge_sidecar_holders(&canonical).expect("canonical holders"),
            canonical.merge_qc.validator_set.clone()
        );

        let mut wrong_reference_version = canonical.clone();
        wrong_reference_version.version += 1;
        assert!(matches!(
            certified_merge_sidecar_holders(&wrong_reference_version),
            Err(MergeSidecarError::UnsupportedVersion(_))
        ));

        let mut wrong_hash_version = canonical.clone();
        wrong_hash_version.merge_qc.validator_set_hash_version += 1;
        assert!(matches!(
            certified_merge_sidecar_holders(&wrong_hash_version),
            Err(MergeSidecarError::MalformedReference(_))
        ));

        let mut wrong_roster_hash = canonical.clone();
        wrong_roster_hash.merge_qc.validator_set_hash = HashOf::new(&Vec::<PeerId>::new());
        assert!(matches!(
            certified_merge_sidecar_holders(&wrong_roster_hash),
            Err(MergeSidecarError::MalformedReference(_))
        ));

        let mut duplicate_roster = canonical.clone();
        let duplicate = duplicate_roster.merge_qc.validator_set[0].clone();
        duplicate_roster.merge_qc.validator_set[1] = duplicate;
        duplicate_roster.merge_qc.validator_set_hash =
            HashOf::new(&duplicate_roster.merge_qc.validator_set);
        assert!(matches!(
            certified_merge_sidecar_holders(&duplicate_roster),
            Err(MergeSidecarError::MalformedReference(_))
        ));

        let mut wrong_bitmap_len = canonical.clone();
        wrong_bitmap_len.merge_qc.signers_bitmap.clear();
        assert!(matches!(
            certified_merge_sidecar_holders(&wrong_bitmap_len),
            Err(MergeSidecarError::MalformedReference(_))
        ));

        let mut nonzero_padding = canonical;
        nonzero_padding.merge_qc.signers_bitmap[0] |= 0b1000_0000;
        assert!(matches!(
            certified_merge_sidecar_holders(&nonzero_padding),
            Err(MergeSidecarError::MalformedReference(_))
        ));
    }

    #[test]
    fn unsolicited_and_wrong_sender_chunks_are_rejected() {
        let now = Instant::now();
        let request = peer(b"requester");
        let responder = peer(b"responder");
        let mut transport = MergeSidecarTransport::new();
        let chunk = CertifiedMergeSidecarChunkV1 {
            version: 1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch: stream_epoch(1),
            semantic_sequence: semantic_sequence(1),
            request_id: Hash::new(b"request"),
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"entry")),
            encoded_len: 1,
            epoch_id: 1,
            reference_digest: Hash::new(b"reference"),
            requester: request,
            responder: responder.clone(),
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![1],
        };
        assert_eq!(
            transport.ingest_chunk(&responder, chunk, now),
            Err(MergeSidecarError::UnsolicitedResponse)
        );

        let (mut transport, _, _, request, now) = start_session(1, 2);
        let mut chunk = chunks(&request, &[1]).remove(0);
        let attacker = peer(b"attacker");
        chunk.responder = attacker.clone();
        assert_eq!(
            transport.ingest_chunk(&attacker, chunk, now),
            Err(MergeSidecarError::UnexpectedResponder)
        );
    }

    #[test]
    fn request_id_hash_length_and_epoch_mismatches_are_rejected() {
        let (mut transport, _, _, request, now) = start_session(1, 2);
        let responder = request.responder.clone();
        let base = chunks(&request, &[9]).remove(0);

        let mut wrong = base.clone();
        wrong.request_id = Hash::new(b"wrong-request");
        assert_eq!(
            transport.ingest_chunk(&responder, wrong, now),
            Err(MergeSidecarError::RequestIdMismatch)
        );
        let mut wrong_stream_epoch = base.clone();
        wrong_stream_epoch.stream_epoch = successor_stream_epoch(base.stream_epoch);
        assert_eq!(
            transport.ingest_chunk(&responder, wrong_stream_epoch, now),
            Err(MergeSidecarError::MetadataMismatch)
        );
        for mutate in [0_u8, 1, 2] {
            let mut wrong = base.clone();
            match mutate {
                0 => wrong.entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong-entry")),
                1 => wrong.encoded_len = 2,
                _ => wrong.epoch_id += 1,
            }
            assert!(transport.ingest_chunk(&responder, wrong, now).is_err());
        }
    }

    #[test]
    fn request_id_binds_occurrence_but_excludes_monotonic_close_floor() {
        let (_, _, _, mut base, _) = start_session(1, 1);
        base.semantic_sequence = semantic_sequence(2);
        base.bind_canonical_request_id();
        let mut variants = Vec::new();

        let mut generation = base.clone();
        generation.service_generation = service_generation(2);
        variants.push(generation);

        let mut epoch = base.clone();
        epoch.stream_epoch = successor_stream_epoch(base.stream_epoch);
        variants.push(epoch);

        let mut sequence = base.clone();
        sequence.semantic_sequence = semantic_sequence(3);
        variants.push(sequence);

        let ids = variants
            .iter()
            .map(CertifiedMergeSidecarRequestV1::canonical_request_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), variants.len());
        assert!(
            ids.iter().all(|request_id| request_id != &base.request_id),
            "generation, stream epoch, and sequence each identify a distinct occurrence"
        );
        let mut close_floor = base.clone();
        close_floor.closed_through = 1;
        assert_eq!(
            close_floor.canonical_request_id(),
            base.request_id,
            "the cumulative close floor may advance without rematerializing the occurrence"
        );
        assert_eq!(base.request_id, base.canonical_request_id());
    }

    #[test]
    fn oversized_counts_payloads_and_duplicate_chunks_are_rejected() {
        let (mut transport, _, _, request, now) =
            start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1, 2);
        let responder = request.responder.clone();
        let bytes = vec![7_u8; MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1];
        let mut all = chunks(&request, &bytes);

        let mut wrong_count = all[0].clone();
        wrong_count.chunk_count = (MAX_CERTIFIED_MERGE_CHUNKS + 1) as u32;
        assert!(matches!(
            transport.ingest_chunk(&responder, wrong_count, now),
            Err(MergeSidecarError::InvalidChunk(_))
        ));
        let mut oversized = all[0].clone();
        oversized.bytes.push(0);
        assert!(matches!(
            transport.ingest_chunk(&responder, oversized, now),
            Err(MergeSidecarError::InvalidChunk(_))
        ));
        assert!(matches!(
            transport.ingest_chunk(&responder, all.remove(0).clone(), now),
            Ok(ChunkIngestOutcome::Accepted)
        ));
        assert_eq!(
            transport.ingest_chunk(&responder, chunks(&request, &bytes).remove(0), now),
            Err(MergeSidecarError::DuplicateChunk(0))
        );
    }

    #[test]
    fn out_of_order_chunks_complete_and_release_exact_deferred_block() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES * 2 + 17;
        let (mut transport, requester, reference, request, now) = start_session(len, 3);
        let responder = request.responder.clone();
        let bytes = (0..len)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        let all = chunks(&request, &bytes);
        assert!(matches!(
            transport.ingest_chunk(&responder, all[2].clone(), now),
            Ok(ChunkIngestOutcome::Accepted)
        ));
        assert!(matches!(
            transport.ingest_chunk(&responder, all[0].clone(), now),
            Ok(ChunkIngestOutcome::Accepted)
        ));
        let complete = transport
            .ingest_chunk(&responder, all[1].clone(), now)
            .expect("final chunk");
        let ChunkIngestOutcome::Complete(complete) = complete else {
            panic!("expected complete")
        };
        assert_eq!(complete.reference, reference);
        assert_eq!(complete.bytes, bytes);
        let (deferred, retry) = transport
            .finish_completed(
                reference.entry_hash,
                certified_merge_reference_digest(&reference),
                true,
                &requester,
                now,
            )
            .expect("persist completed request lifecycle");
        assert_eq!(deferred.len(), 1);
        assert!(retry.is_none());
        assert_eq!(transport.inbound_len(), 0);
    }

    #[test]
    fn failed_response_rotation_persists_close_before_sequence_exhaustion() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let requester = peer(b"exhausted rotation requester");
        let reference = reference(1, 1);
        let key = (
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
        );
        let mut transport = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open durable rotation fixture");
        let post = transport
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"exhausted rotation block")),
                2,
                0,
                reference,
                &requester,
                1,
                now,
            )
            .expect("admit durable request")
            .expect("emit durable request");
        let CertifiedMergeSidecarMessage::Request(request) = Arc::unwrap_or_clone(post.message)
        else {
            panic!("rotation fixture emits a request")
        };
        transport
            .request_streams
            .get_mut(&request.responder)
            .expect("request stream exists")
            .next_sequence = u64::MAX;
        transport
            .persist_lifecycle_state()
            .expect("persist the exhausted stream frontier");

        assert!(matches!(
            transport.finish_completed(key.0, key.1, false, &requester, now),
            Err(MergeSidecarError::Capacity(
                "semantic request sequence exhausted"
            ))
        ));
        assert!(
            transport.inbound[&key].current.is_none(),
            "the failed occurrence is durably idle after release"
        );
        let memory = transport
            .lifecycle_snapshot()
            .expect("snapshot released occurrence in memory");
        let durable = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal")
            .load()
            .expect("load released occurrence")
            .expect("released occurrence has a durable snapshot");
        assert_eq!(durable, memory);
        let stream = memory
            .payload
            .request_streams
            .iter()
            .find(|stream| stream.responder == request.responder)
            .expect("released stream remains durable");
        assert_eq!(stream.next_sequence, u64::MAX);
        assert_eq!(stream.closed_through, u64::MAX);
    }

    #[test]
    fn timeout_rotates_to_another_qc_holder() {
        let (mut transport, requester, reference, first, now) = start_session(1, 3);
        let posts = transport
            .tick_bounded(&requester, now + REQUEST_TIMEOUT, usize::MAX)
            .expect("rotate timed-out request");
        let next = posts
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("rotated request");
        assert_ne!(next.request_id, first.request_id);
        assert_ne!(next.responder, first.responder);
        assert_eq!(next.entry_hash, reference.entry_hash);
    }

    #[test]
    fn all_holders_can_withhold_past_session_horizon_then_recovery_resumes_block() {
        let (mut transport, requester, reference, mut request, started_at) = start_session(1, 3);
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(b"block"));
        let pending_blocks = BTreeSet::from([block_hash]);
        let mut now = started_at;

        // Ignore enough requests to cross the former five-minute session TTL.
        // Each timeout rotates the exact QC holder and backs off to a bounded
        // maximum interval; the deferred carrier identity must remain present.
        for attempt in 1..=8 {
            now += retry_timeout(REQUEST_TIMEOUT, attempt);
            transport
                .retain_pending_blocks(&pending_blocks, 1)
                .expect("persist retained carrier lifecycle");
            request = transport
                .tick_bounded(&requester, now, usize::MAX)
                .expect("service retained request")
                .into_iter()
                .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                    CertifiedMergeSidecarMessage::Request(request) => Some(request),
                    CertifiedMergeSidecarMessage::Close(_)
                    | CertifiedMergeSidecarMessage::CloseAck(_)
                    | CertifiedMergeSidecarMessage::GenerationHint(_)
                    | CertifiedMergeSidecarMessage::Chunk(_) => None,
                })
                .expect("withheld holder must be retried indefinitely");
        }
        assert!(now.duration_since(started_at) > Duration::from_secs(5 * 60));
        assert_eq!(transport.inbound_len(), 1);

        let responder = request.responder.clone();
        let complete = transport
            .ingest_chunk(&responder, chunks(&request, &[1]).remove(0), now)
            .expect("eventually available holder response");
        assert!(matches!(complete, ChunkIngestOutcome::Complete(_)));
        let (deferred, retry) = transport
            .finish_completed(
                reference.entry_hash,
                certified_merge_reference_digest(&reference),
                true,
                &requester,
                now,
            )
            .expect("persist completed request lifecycle");
        assert_eq!(deferred, vec![(block_hash, 2, 0)]);
        assert!(retry.is_none());
        assert_eq!(transport.inbound_len(), 0);
    }

    #[test]
    fn pending_pruning_keeps_only_authoritative_live_carrier_identities() {
        let (mut transport, requester, reference, request, now) = start_session(1, 2);
        let original = HashOf::from_untyped_unchecked(Hash::new(b"block"));
        let replacement = HashOf::from_untyped_unchecked(Hash::new(b"replacement-block"));
        transport
            .defer_block(replacement, 2, 1, reference.clone(), &requester, 1, now)
            .expect("share exact sidecar session");

        transport
            .retain_pending_blocks(&BTreeSet::from([replacement]), 1)
            .expect("persist replacement carrier lifecycle");
        let responder = request.responder.clone();
        assert!(matches!(
            transport
                .ingest_chunk(&responder, chunks(&request, &[1]).remove(0), now)
                .expect("complete retained fetch"),
            ChunkIngestOutcome::Complete(_)
        ));
        let (deferred, _) = transport
            .finish_completed(
                reference.entry_hash,
                certified_merge_reference_digest(&reference),
                true,
                &requester,
                now,
            )
            .expect("persist completed replacement lifecycle");
        assert_eq!(deferred, vec![(replacement, 2, 1)]);
        assert!(!deferred.iter().any(|(hash, _, _)| *hash == original));

        let (mut transport, _, _, _, _) = start_session(1, 2);
        transport
            .retain_pending_blocks(&BTreeSet::from([original]), 2)
            .expect("persist retired carrier lifecycle");
        assert_eq!(transport.inbound_len(), 0);
    }

    #[test]
    fn progressive_max_size_response_does_not_expire_while_chunks_advance() {
        let len = MAX_MERGE_LEDGER_ENTRY_BYTES;
        let (_, requester, _, request, started_at) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"progressive response hub"));
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        server
            .admit_server_request(&requester, &request, Some(&route), &local_peer, started_at)
            .expect("admit exact authenticated request");
        server
            .enqueue_response(request, Some(route.clone()), vec![0x5A; len], started_at)
            .expect("queue protocol-sized response");

        let expected_chunks = len.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        let mut seen = 0usize;
        for tick in 0..expected_chunks {
            let elapsed_secs = u64::try_from(tick).expect("chunk count fits u64") * 2;
            let now = started_at + Duration::from_secs(elapsed_secs);
            let post = server
                .drain_outbound_chunks(8, now)
                .pop()
                .expect("one source owns one in-flight response chunk");
            let CertifiedMergeSidecarMessage::Chunk(chunk) = post.message.as_ref() else {
                panic!("outbound response emitted a request")
            };
            assert_eq!(usize::try_from(chunk.chunk_index).unwrap(), seen);
            assert!(acknowledge_reply_chunk(&mut server, &post, now));
            seen += 1;
            if seen == expected_chunks {
                assert!(now.duration_since(started_at) > Duration::from_secs(30));
                break;
            }
        }
        assert_eq!(seen, expected_chunks);
    }

    #[test]
    fn exact_active_delivery_retry_preserves_decreasing_chunk_rank() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub = peer(b"exact retry hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit first exact request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0xA5; len], now)
            .expect("queue bounded response");
        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("emit first chunk");
        assert!(matches!(
            first.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0
        ));

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("deduplicate the exact active delivery"),
            ServerRequestAdmission::Existing
        ));
        assert!(
            server.drain_outbound_chunks(1, now).is_empty(),
            "an exact duplicate must neither reset nor requeue the in-flight chunk"
        );
        assert!(acknowledge_reply_chunk(&mut server, &first, now));
        let continued = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("continue the response from its next fixed boundary");
        assert!(matches!(
            &continued,
            MergeSidecarPost {
                reply_route: Some(emitted),
                message,
                ..
            } if emitted.same_delivery(&route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &continued, now));
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
    }

    #[test]
    fn alternate_source_progress_and_reconnect_preserve_independent_cursors() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"independent cursor hub a");
        let hub_b = peer(b"independent cursor hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a.clone());
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit request through source A"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0xA5; len], now)
            .expect("queue bounded response");
        let first_a = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("source A starts at chunk zero");
        assert!(matches!(
            &first_a,
            MergeSidecarPost {
                message,
                ..
            } if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &first_a, now));

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("attach independent source B to shared bytes"),
            ServerRequestAdmission::Existing
        ));
        assert_eq!(
            server.outbound[&(requester.clone(), request.request_id)]
                .attempts
                .len(),
            2
        );

        assert!(routes.retire(&route_a));
        let first_b = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("source B starts independently at chunk zero");
        assert!(matches!(
            &first_b,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&route_b)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0)
        ));
        assert!(
            Arc::ptr_eq(&first_a.message, &first_b.message),
            "independent sources must share the materialized chunk-zero carrier"
        );
        assert!(acknowledge_reply_chunk(&mut server, &first_b, now));

        let reconnected_a = routes.mint_via(requester.clone(), hub_a);
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&reconnected_a), &local_peer, now,)
                .expect("reattach source A at its retained source cursor"),
            ServerRequestAdmission::Existing
        ));
        let continued = server.drain_outbound_chunks(2, now);
        assert!(matches!(
            continued.as_slice(),
            [
                MergeSidecarPost {
                    reply_route: Some(route_b_post),
                    message: message_b,
                    ..
                },
                MergeSidecarPost {
                    reply_route: Some(route_a_post),
                    message: message_a,
                    ..
                }
            ] if route_b_post.same_delivery(&route_b)
                && route_a_post.same_delivery(&reconnected_a)
                && matches!(message_b.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
                && matches!(message_a.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(
            Arc::ptr_eq(&continued[0].message, &continued[1].message),
            "independent source cursors must share the same cached chunk carrier"
        );
        assert!(
            !Arc::ptr_eq(&first_b.message, &continued[0].message),
            "distinct fixed-boundary chunks must retain distinct carriers"
        );
        assert!(acknowledge_reply_chunk(&mut server, &continued[0], now));
        assert!(acknowledge_reply_chunk(&mut server, &continued[1], now));
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
    }

    #[test]
    fn height_rollover_retries_only_each_sources_current_in_flight_chunk() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"height rollover hub a");
        let hub_b = peer(b"height rollover hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit source A"),
            ServerRequestAdmission::Materialize
        ));
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("attach source B"),
            ServerRequestAdmission::Existing
        ));
        server
            .enqueue_response(request, Some(route_a.clone()), vec![0xC7; len], now)
            .expect("materialize shared response");

        let first = server.drain_outbound_chunks(2, now);
        let first_a = first
            .iter()
            .find(|post| {
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&route_a))
            })
            .expect("source A receives chunk zero");
        let first_b = first
            .iter()
            .find(|post| {
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&route_b))
            })
            .expect("source B receives chunk zero");
        assert!(matches!(
            first_b.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0
        ));
        assert!(acknowledge_reply_chunk(&mut server, first_a, now));

        let second_a = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("source A advances to chunk one");
        assert!(matches!(
            &second_a,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&route_a)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert_eq!(
            server.outbound[&key].attempts[&source_a].in_flight_chunk,
            Some(1)
        );
        assert_eq!(
            server.outbound[&key].attempts[&source_b].in_flight_chunk,
            Some(0)
        );

        let mut rehydrated = server
            .rehydrate_with_exact_geometry(
                DEFAULT_REPLY_SOURCE_CAPACITY,
                MergeSidecarLimits::defaults(),
                MAX_CERTIFIED_MERGE_SEMANTIC_PEERS,
                unbound_test_merge_sidecar_roster_digest(),
                now,
            )
            .expect("retain exact source geometry across height rollover");
        assert_eq!(rehydrated.outbound[&key].attempts[&source_a].next_chunk, 1);
        assert_eq!(rehydrated.outbound[&key].attempts[&source_b].next_chunk, 0);
        assert_eq!(
            rehydrated.outbound[&key].attempts[&source_a].in_flight_chunk,
            None
        );
        assert_eq!(
            rehydrated.outbound[&key].attempts[&source_b].in_flight_chunk,
            None
        );

        let retried = rehydrated.drain_outbound_chunks(2, now);
        assert_eq!(retried.len(), 2);
        assert!(retried.iter().any(|post| {
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&route_a))
                && matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1
                )
        }));
        assert!(retried.iter().any(|post| {
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&route_b))
                && matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0
                )
        }));
    }

    #[test]
    fn cached_sidecar_payload_objects_scale_with_chunks_not_sources() {
        let source_count = DEFAULT_REPLY_SOURCE_CAPACITY;
        let response_len = MAX_CERTIFIED_MERGE_CHUNK_BYTES * 2 + 1;
        let (_, requester, _, request, now) = start_session(response_len, 3);
        let local_peer = request.responder.clone();
        let hubs = (0..source_count)
            .map(|index| peer(format!("cached chunk hub {index}").as_bytes()))
            .collect::<Vec<_>>();
        let mut routes = NetworkReplyRouteTestFixture::new(hubs[0].clone());
        let reply_routes = hubs
            .into_iter()
            .map(|hub| routes.mint_via(requester.clone(), hub))
            .collect::<Vec<_>>();
        let mut server = MergeSidecarTransport::new();

        for (index, route) in reply_routes.iter().enumerate() {
            let admission = server
                .admit_server_request(&requester, &request, Some(route), &local_peer, now)
                .expect("admit one independent authenticated source");
            if index == 0 {
                assert!(matches!(admission, ServerRequestAdmission::Materialize));
            } else {
                assert!(matches!(admission, ServerRequestAdmission::Existing));
            }
        }
        server
            .enqueue_response(
                request,
                Some(reply_routes[0].clone()),
                vec![0xA7; response_len],
                now,
            )
            .expect("materialize one shared response");

        let expected_chunks = response_len.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        let mut unique_payloads = BTreeSet::new();
        let mut emitted = 0usize;
        for chunk_index in 0..expected_chunks {
            let posts = server.drain_outbound_chunks(source_count, now);
            assert_eq!(posts.len(), source_count);
            let first = &posts[0].message;
            for post in &posts {
                assert!(
                    Arc::ptr_eq(first, &post.message),
                    "all source-local cursors at one boundary must share one carrier"
                );
                assert!(matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if usize::try_from(chunk.chunk_index).ok() == Some(chunk_index)
                ));
                unique_payloads.insert(Arc::as_ptr(&post.message) as usize);
                let admission = reply_chunk_admission(post);
                let strong_count = Arc::strong_count(&post.message);
                assert!(admission.matches_materialized_chunk(&post.message));
                assert_eq!(
                    Arc::strong_count(&post.message),
                    strong_count,
                    "matching must borrow the cached carrier without retaining another owner"
                );
                assert!(
                    server
                        .acknowledge_outbound_chunk(&admission, now)
                        .expect("acknowledge the cached response")
                );
                emitted += 1;
            }
        }

        assert_eq!(emitted, source_count * expected_chunks);
        assert_eq!(unique_payloads.len(), expected_chunks);
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn sidecar_admission_matches_the_cached_arc_without_changing_ownership() {
        let response = vec![0x5A; 64];
        let (_, requester, _, request, now) = start_session(response.len(), 1);
        let local_peer = request.responder.clone();
        let hub = peer(b"cached admission hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
        let route = routes.mint_via(requester.clone(), hub);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit one authenticated source"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route), response, now)
            .expect("materialize one cached response");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("drain cached response chunk");
        let admission = reply_chunk_admission(&post);
        let strong_count = Arc::strong_count(&post.message);

        assert!(admission.matches_materialized_chunk(&post.message));
        assert_eq!(
            Arc::strong_count(&post.message),
            strong_count,
            "matching must borrow the cached carrier without retaining another owner"
        );

        let mut altered = post.message.as_ref().clone();
        let CertifiedMergeSidecarMessage::Chunk(chunk) = &mut altered else {
            panic!("response fixture must be a chunk")
        };
        chunk.bytes[0] ^= 0xFF;
        assert!(!admission.matches_materialized_chunk(&Arc::new(altered)));
        assert!(
            server
                .acknowledge_outbound_chunk(&admission, now)
                .expect("acknowledge the cached response")
        );
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn sidecar_flush_refinement_advances_only_exact_source_chunk() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"flush refinement hub a");
        let hub_b = peer(b"flush refinement hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts exact shared materialization"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0xA6; len], now)
            .expect("materialize one shared two-chunk response");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("source B attaches its independent cursor to shared bytes"),
            ServerRequestAdmission::Existing
        ));
        let first = server.drain_outbound_chunks(2, now);
        let first_a = first
            .iter()
            .find(|post| {
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&route_a))
            })
            .expect("source A receives chunk zero");
        let first_b = first
            .iter()
            .find(|post| {
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&route_b))
            })
            .expect("source B receives chunk zero");
        let admission_a = reply_chunk_admission(first_a);
        let admission_b = reply_chunk_admission(first_b);
        assert_eq!(server.outbound[&key].response_len, len);
        assert_eq!(server.outbound[&key].attempts[&source_a].next_chunk, 0);
        assert_eq!(server.outbound[&key].attempts[&source_b].next_chunk, 0);

        let mut missing_worker_trace = admission_a.clone();
        missing_worker_trace.confirmed_worker_trace = None;
        let mut disconnected_worker_trace = admission_a.clone();
        let disconnected_delivery_ordinal = disconnected_worker_trace
            .confirmed_worker_trace
            .expect("test admission carries its accepted worker transition")
            .delivery_ordinal_low
            .wrapping_add(1);
        disconnected_worker_trace
            .confirmed_worker_trace
            .as_mut()
            .expect("test admission carries its accepted worker transition")
            .delivery_ordinal_low = disconnected_delivery_ordinal;
        assert!(
            production_reliable_flush_trace_refines_outbound_ownership_kernel(
                disconnected_worker_trace
                    .confirmed_worker_trace
                    .expect("mutated trace remains present")
            ),
            "the mutation must preserve the worker leaf while breaking only the two-phase link"
        );
        let mut disconnected_source_owner = admission_a.clone();
        disconnected_source_owner
            .confirmed_worker_trace
            .as_mut()
            .expect("test admission carries its accepted worker transition")
            .source_key_identity
            .word0 ^= 1;
        let mut disconnected_delivery_route = admission_a.clone();
        disconnected_delivery_route
            .confirmed_worker_trace
            .as_mut()
            .expect("test admission carries its accepted worker transition")
            .delivery_route_identity
            .word0 ^= 1;
        let mut disconnected_writer_occurrence = admission_a.clone();
        disconnected_writer_occurrence
            .confirmed_worker_trace
            .as_mut()
            .expect("test admission carries its accepted worker transition")
            .writer_occurrence_identity
            .word0 ^= 1;
        let mut disconnected_timeout_attempt = admission_a.clone();
        let timeout_attempt = disconnected_timeout_attempt
            .confirmed_worker_trace
            .expect("test admission carries its accepted worker transition")
            .reply_writer_timeout_attempt
            .saturating_add(1);
        disconnected_timeout_attempt
            .confirmed_worker_trace
            .as_mut()
            .expect("test admission carries its accepted worker transition")
            .reply_writer_timeout_attempt = timeout_attempt;
        for (label, disconnected) in [
            ("source owner", &disconnected_source_owner),
            ("delivery route", &disconnected_delivery_route),
            ("writer occurrence", &disconnected_writer_occurrence),
            ("timeout attempt", &disconnected_timeout_attempt),
        ] {
            assert!(
                production_reliable_flush_trace_refines_outbound_ownership_kernel(
                    disconnected
                        .confirmed_worker_trace
                        .expect("mutated trace remains present")
                ),
                "the {label} mutation must preserve the worker leaf while breaking the link"
            );
        }
        let mut wrong_bind = admission_a.clone();
        wrong_bind.confirmed_worker_trace = None;
        let wrong_bind_trace = disconnected_writer_occurrence
            .confirmed_worker_trace
            .expect("mutated trace remains present");
        assert!(matches!(
            wrong_bind.bind_confirmed_worker_trace(wrong_bind_trace),
            Err(MergeSidecarError::FlushIdentityMismatch(_))
        ));
        let mut source_owner_projection_mismatch = admission_a.clone();
        source_owner_projection_mismatch
            .projection
            .source_key_identity = Hash::new(b"foreign process-local source owner");
        let mut delivery_route_projection_mismatch = admission_a.clone();
        delivery_route_projection_mismatch
            .projection
            .delivery_route_identity = Hash::new(b"substituted exact delivery route");
        let mut writer_occurrence_projection_mismatch = admission_a.clone();
        writer_occurrence_projection_mismatch
            .projection
            .writer_occurrence_identity = Hash::new(b"rebuilt writer completion claim");
        let mut source_mismatch = admission_a.clone();
        source_mismatch.projection.authenticated_source =
            admission_b.projection.authenticated_source.clone();
        source_mismatch.source_key = admission_b.source_key.clone();
        let mut tenure_mismatch = admission_a.clone();
        tenure_mismatch.projection.connection_tenure_ordinal = tenure_mismatch
            .projection
            .connection_tenure_ordinal
            .saturating_add(1);
        let mut delivery_mismatch = admission_a.clone();
        delivery_mismatch.projection.delivery_ordinal = delivery_mismatch
            .projection
            .delivery_ordinal
            .saturating_add(1);
        let mut ticket_mismatch = admission_a.clone();
        ticket_mismatch.projection.ticket_id =
            ticket_mismatch.projection.ticket_id.saturating_add(1);
        let mut ticket_rank_mismatch = admission_a.clone();
        ticket_rank_mismatch.projection.ticket_rank = ticket_rank_mismatch
            .projection
            .ticket_rank
            .saturating_add(1);
        let mut ticket_topic_mismatch = admission_a.clone();
        ticket_topic_mismatch.projection.ticket_topic = Topic::Consensus;
        let mut timeout_attempt_mismatch = admission_a.clone();
        timeout_attempt_mismatch
            .projection
            .reply_writer_timeout_attempt = timeout_attempt_mismatch
            .projection
            .reply_writer_timeout_attempt
            .saturating_add(1);
        let mut ticket_digest_mismatch = admission_a.clone();
        ticket_digest_mismatch.projection.canonical_request_digest =
            Hash::new(b"mutated actor request digest");
        let mut stream_charge_mismatch = admission_a.clone();
        stream_charge_mismatch.projection.stream_wire_bytes = stream_charge_mismatch
            .projection
            .stream_wire_bytes
            .saturating_add(1);
        let mut payload_mismatch = admission_a.clone();
        payload_mismatch.projection.payload_digest = Hash::new(b"mutated sidecar payload");
        let mut request_mismatch = admission_a.clone();
        request_mismatch.projection.request_id = Hash::new(b"mutated sidecar request");
        let mut chunk_mismatch = admission_a.clone();
        chunk_mismatch.projection.chunk_index = 1;
        let mut cursor_mismatch = admission_a.clone();
        cursor_mismatch.projection.chunk_cursor_after = 2;
        let mut message_cursor_mismatch = admission_a.clone();
        message_cursor_mismatch.projection.message_cursor_after = 2;

        for (label, mismatched) in [
            ("missing worker trace", missing_worker_trace),
            ("disconnected worker trace", disconnected_worker_trace),
            ("disconnected source owner", disconnected_source_owner),
            ("disconnected delivery route", disconnected_delivery_route),
            (
                "disconnected writer occurrence",
                disconnected_writer_occurrence,
            ),
            ("disconnected timeout attempt", disconnected_timeout_attempt),
            ("source-owner projection", source_owner_projection_mismatch),
            (
                "delivery-route projection",
                delivery_route_projection_mismatch,
            ),
            (
                "writer-occurrence projection",
                writer_occurrence_projection_mismatch,
            ),
            ("source", source_mismatch),
            ("tenure", tenure_mismatch),
            ("delivery", delivery_mismatch),
            ("ticket", ticket_mismatch),
            ("ticket rank", ticket_rank_mismatch),
            ("ticket topic", ticket_topic_mismatch),
            ("timeout attempt", timeout_attempt_mismatch),
            ("ticket digest", ticket_digest_mismatch),
            ("stream charge", stream_charge_mismatch),
            ("payload", payload_mismatch),
            ("request", request_mismatch),
            ("chunk", chunk_mismatch),
            ("chunk cursor", cursor_mismatch),
            ("message cursor", message_cursor_mismatch),
        ] {
            assert_ne!(
                server.acknowledge_outbound_chunk(&mismatched, now),
                Ok(true),
                "{label} substitution must not advance any source"
            );
            assert_eq!(
                server.outbound[&key].attempts[&source_a].next_chunk, 0,
                "{label} substitution changed source A"
            );
            assert_eq!(
                server.outbound[&key].attempts[&source_b].next_chunk, 0,
                "{label} substitution changed source B"
            );
        }

        let exact_source_a_route = server.outbound[&key].attempts[&source_a]
            .reply_route
            .clone()
            .expect("source A retains its exact writer route");
        server
            .outbound
            .get_mut(&key)
            .expect("shared response remains materialized")
            .attempts
            .get_mut(&source_a)
            .expect("source A remains retained")
            .reply_route = Some(route_b.clone());
        assert_eq!(
            server.acknowledge_outbound_chunk(&admission_a, now),
            Ok(false),
            "a post-marker foreign route must fail before consuming the writer claim"
        );
        server
            .outbound
            .get_mut(&key)
            .expect("shared response remains materialized")
            .attempts
            .get_mut(&source_a)
            .expect("source A remains retained")
            .reply_route = Some(exact_source_a_route);
        server
            .outbound_order
            .push_back((key.clone(), source_a.clone()));
        server
            .outbound_order
            .push_back((key.clone(), source_a.clone()));
        assert!(matches!(
            server.acknowledge_outbound_chunk(&admission_a, now),
            Err(MergeSidecarError::FlushIdentityMismatch(_))
        ));
        server
            .outbound_order
            .retain(|(queued_key, queued_source)| queued_key != &key || queued_source != &source_a);

        assert!(
            server
                .acknowledge_outbound_chunk(&admission_a, now)
                .expect("the same exact source A claim remains usable after preflight rejection")
        );
        assert_eq!(server.outbound[&key].attempts[&source_a].next_chunk, 1);
        assert_eq!(server.outbound[&key].attempts[&source_b].next_chunk, 0);
        assert_eq!(
            server.outbound[&key].attempts[&source_b].in_flight_chunk,
            Some(0),
            "source A completion must not erase source B's current chunk"
        );
        assert!(
            server
                .acknowledge_outbound_chunk(&admission_b, now)
                .expect("exact source B acknowledgement")
        );
        assert_eq!(server.outbound[&key].attempts[&source_a].next_chunk, 1);
        assert_eq!(server.outbound[&key].attempts[&source_b].next_chunk, 1);
    }

    #[test]
    fn sidecar_flush_admission_retains_timeout_attempt_identity() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"timeout-attempt sidecar hub"));
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit the timeout-attempt source"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route), vec![0xA7], now)
            .expect("materialize one timeout-attempt response");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("drain the timeout-attempt response");

        let admission = reply_chunk_admission_at_attempt(&post, 2);
        assert_eq!(admission.projection.reply_writer_timeout_attempt, 2);
        assert!(admission.matches_ack_identity(&admission.flush_identity));
        let trace = admission
            .confirmed_worker_trace
            .expect("fixture binds its exact worker trace");
        let application = reliable_flush_application_occurrence_projection(&admission)
            .expect("project the exact lane occurrence");
        assert_eq!(trace.reply_writer_timeout_attempt, 2);
        assert_eq!(application.reply_writer_timeout_attempt, 2);
        assert!(production_reliable_flush_two_phase_link_kernel(
            trace,
            application
        ));

        let mut mismatched = admission.clone();
        mismatched.projection.reply_writer_timeout_attempt = 3;
        assert!(
            !mismatched.matches_ack_identity(&mismatched.flush_identity),
            "a sidecar admission must reject a substituted timeout generation"
        );

        let service_generation = CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(29).expect("non-zero service generation"),
        );
        let stream_epoch =
            CertifiedMergeSidecarStreamEpochV1(NonZeroU64::new(73).expect("non-zero stream epoch"));
        let sequence = semantic_sequence(11);
        let mut coordinate_admission = admission.clone();
        coordinate_admission.projection.service_generation = service_generation;
        coordinate_admission.projection.stream_epoch = stream_epoch;
        coordinate_admission.projection.semantic_sequence = sequence;
        let mut coordinate_worker = coordinate_admission
            .confirmed_worker_trace
            .expect("fixture binds its exact worker trace");
        coordinate_worker.service_generation = service_generation.get();
        coordinate_worker.stream_epoch = stream_epoch.get();
        coordinate_worker.semantic_sequence = sequence.get();

        let mut coordinate_application =
            reliable_flush_application_occurrence_projection(&coordinate_admission)
                .expect("project the distinct lane occurrence");
        assert_eq!(coordinate_application.service_generation, 29);
        assert_eq!(coordinate_application.stream_epoch, 73);
        assert_eq!(coordinate_application.semantic_sequence, 11);
        assert_eq!(coordinate_application.marker_service_generation, 29);
        assert_eq!(coordinate_application.marker_stream_epoch, 73);
        assert_eq!(coordinate_application.marker_semantic_sequence, 11);

        let mut marker = ServerPendingChunkIdentity::from_message(&post.message)
            .expect("the response fixture contains one sidecar chunk");
        marker.service_generation = CertifiedMergeSidecarServiceGenerationV1(
            NonZeroU64::new(31).expect("non-zero divergent service generation"),
        );
        marker.stream_epoch = CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(79).expect("non-zero divergent stream epoch"),
        );
        marker.semantic_sequence = semantic_sequence(13);
        project_reliable_flush_marker(&mut coordinate_application, &marker);
        assert_eq!(coordinate_application.service_generation, 29);
        assert_eq!(coordinate_application.stream_epoch, 73);
        assert_eq!(coordinate_application.semantic_sequence, 11);
        assert_eq!(coordinate_application.marker_service_generation, 31);
        assert_eq!(coordinate_application.marker_stream_epoch, 79);
        assert_eq!(coordinate_application.marker_semantic_sequence, 13);
        assert!(
            !production_reliable_flush_two_phase_link_kernel(
                coordinate_worker,
                coordinate_application
            ),
            "a retained marker from another durable occurrence must break the two-phase link"
        );

        marker.service_generation = service_generation;
        marker.stream_epoch = stream_epoch;
        marker.semantic_sequence = sequence;
        project_reliable_flush_marker(&mut coordinate_application, &marker);
        assert!(
            production_reliable_flush_two_phase_link_kernel(
                coordinate_worker,
                coordinate_application
            ),
            "pairwise-distinct matching coordinates must retain the exact two-phase link"
        );
    }

    #[test]
    fn reused_actor_ordinals_under_different_tenures_are_rejected_atomically() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"ordinal collision hub a");
        let hub_b = peer(b"ordinal collision hub b");
        let hub_c = peer(b"connection ordinal collision hub c");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit the original authenticated source"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0xA6; len], now)
            .expect("queue immutable response bytes");
        let in_flight = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand source A its current chunk");
        assert!(matches!(
            in_flight.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0
        ));

        let forged_delivery_ordinal = routes
            .forge_equal_ordinal_different_tenure(&route_a, requester.clone(), hub_b)
            .expect("forge an actor-global ordinal collision for the adversarial test");
        assert!(route_a.equal_ordinal_different_tenure(&forged_delivery_ordinal));
        let forged_connection_ordinal = routes
            .forge_equal_connection_ordinal_different_tenure(&route_a, requester.clone(), hub_c)
            .expect("forge an actor-global connection ordinal collision for the adversarial test");
        assert!(route_a.equal_connection_ordinal_different_tenure(&forged_connection_ordinal));
        let forged_sources = [
            ServerRequestSource::Authenticated(forged_delivery_ordinal.source_key()),
            ServerRequestSource::Authenticated(forged_connection_ordinal.source_key()),
        ];
        assert!(forged_sources.iter().all(|source| source != &source_a));

        let key = (requester.clone(), request.request_id);
        for forged in [&forged_delivery_ordinal, &forged_connection_ordinal] {
            let gate_attempts_before = server.server_gate_attempt_count();
            let outbound_attempts_before = server.outbound_attempt_count();
            let outbound_bytes_before = server.global_outbound_bytes();
            let outbound_order_before = server.outbound_order.len();
            assert!(matches!(
                server.admit_server_request(&requester, &request, Some(forged), &local_peer, now,),
                Err(MergeSidecarError::UnsolicitedResponse)
            ));

            assert_eq!(server.server_gate_attempt_count(), gate_attempts_before);
            assert_eq!(server.outbound_attempt_count(), outbound_attempts_before);
            assert_eq!(server.global_outbound_bytes(), outbound_bytes_before);
            assert_eq!(server.outbound_order.len(), outbound_order_before);
        }
        let gate = &server.server_request_gates[&key];
        assert_eq!(gate.attempts.len(), 1);
        assert!(
            forged_sources
                .iter()
                .all(|source| !gate.attempts.contains_key(source))
        );
        let gate_attempt = &gate.attempts[&source_a];
        assert!(
            gate_attempt
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&route_a))
        );
        assert_eq!(gate_attempt.cursor, ServerResponseCursor::Pending(0));
        let transfer = &server.outbound[&key];
        assert_eq!(transfer.attempts.len(), 1);
        assert!(
            forged_sources
                .iter()
                .all(|source| !transfer.attempts.contains_key(source))
        );
        let attempt = &transfer.attempts[&source_a];
        assert!(
            attempt
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&route_a))
        );
        assert_eq!(attempt.next_chunk, 0);
        assert_eq!(attempt.in_flight_chunk, Some(0));
        assert!(!attempt.queued);

        assert!(acknowledge_reply_chunk(&mut server, &in_flight, now));
        let continued = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("rejected alternate source cannot disturb source A progress");
        assert!(matches!(
            &continued,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&route_a)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &continued, now));
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn inactive_source_reclamation_releases_budget_and_reconnect_rematerializes() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub = peer(b"source teardown hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let source = ServerRequestSource::Authenticated(route.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit initial source"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0xA4; len], now)
            .expect("queue initial response");
        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand chunk zero to exact output");
        assert!(matches!(
            first.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0
        ));
        assert!(acknowledge_reply_chunk(&mut server, &first, now));
        assert_eq!(server.source_outbound_count(&source), 1);
        assert_eq!(server.source_outbound_bytes(&source), len);

        assert!(routes.retire(&route));
        assert!(
            server
                .tick_bounded(&local_peer, now, 0)
                .expect("reclaim retired source")
                .is_empty()
        );
        assert!(!server.outbound.contains_key(&key));
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.source_outbound_count(&source), 0);
        assert_eq!(server.source_outbound_bytes(&source), 0);
        assert_eq!(
            server.server_request_gates[&key].attempts[&source].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert!(server.server_request_gates[&key].attempts[&source].materialization_retryable);

        let reconnect_at = now + Duration::from_secs(301);
        let reconnected = routes.mint(requester.clone());
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&reconnected),
                    &local_peer,
                    reconnect_at,
                )
                .expect("delayed reconnect reacquires fair materialization authority"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request,
                Some(reconnected.clone()),
                vec![0xA4; len],
                reconnect_at,
            )
            .expect("rematerialize identical bytes at the retained cursor");
        let continued = server
            .drain_outbound_chunks(1, reconnect_at)
            .pop()
            .expect("reconnect resumes after acknowledged source progress");
        assert!(matches!(
            &continued,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&reconnected)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(
            &mut server,
            &continued,
            reconnect_at
        ));
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.source_outbound_count(&source), 0);
        assert_eq!(server.source_outbound_bytes(&source), 0);
    }

    #[test]
    fn later_delivery_preserves_the_current_source_cursor() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"later delivery hub"));
        let first_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now)
                .expect("admit first delivery"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(first_route.clone()),
                vec![0x5A; len],
                now,
            )
            .expect("queue response bytes");
        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand the current chunk to exact output");
        assert!(matches!(
            &first,
            MergeSidecarPost {
                message,
                ..
            } if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0)
        ));

        let later_route = routes
            .redeliver(&first_route)
            .expect("mint later delivery on the retained tenure");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("update only this source delivery"),
            ServerRequestAdmission::Existing
        ));
        assert!(
            acknowledge_reply_chunk(&mut server, &first, now),
            "an actor receipt from an earlier delivery remains valid on the same tenure"
        );
        let continued = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("admission advances to the next fixed chunk");
        assert!(matches!(
            &continued,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&later_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &continued, now));
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"in-flight delivery hub"));
        let first_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now)
                .expect("admit first delivery"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(first_route.clone()),
                vec![0xD4; len],
                now,
            )
            .expect("queue response bytes");
        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand chunk zero to exact output");

        let later_route = routes
            .redeliver(&first_route)
            .expect("mint later delivery on the retained tenure");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("rebind only this source delivery"),
            ServerRequestAdmission::Existing
        ));
        assert!(
            server.drain_outbound_chunks(1, now).is_empty(),
            "a same-tenure redelivery cannot emit the in-flight current chunk twice"
        );

        assert!(acknowledge_reply_chunk(&mut server, &first, now));
        let next = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("writer flush schedules the next fixed chunk");
        assert!(matches!(
            &next,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&later_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &next, now));
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
    }

    #[test]
    fn late_old_exact_item_receipt_completes_reconnected_attempt_once() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"receipt tenure hub"));
        let old_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now)
                .expect("admit old tenure"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(old_route.clone()),
                vec![0xC7; len],
                now,
            )
            .expect("queue old-tenure response");
        let old_zero = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("emit old-tenure chunk zero");
        assert!(acknowledge_reply_chunk(&mut server, &old_zero, now));
        let old_one = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand old-tenure chunk one to exact output");
        let late_old_receipt = reply_chunk_admission(&old_one);

        assert!(routes.retire(&old_route));
        let reconnected = routes.mint(requester.clone());
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&reconnected), &local_peer, now)
                .expect("reconnect makes the retained cursor retryable"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(reconnected.clone()),
                vec![0xC7; len],
                now,
            )
            .expect("reconnect rematerializes identical bytes at the retained cursor");
        let new_one = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("new tenure retries the rematerialized current chunk");
        assert!(matches!(
            &new_one,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_tenure(&reconnected)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1)
        ));
        assert!(
            server
                .acknowledge_outbound_chunk(&late_old_receipt, now)
                .expect("the old successful flush completes the same source item once")
        );
        assert!(
            server.drain_outbound_chunks(1, now).is_empty(),
            "the old receipt cancels the queued reconnect retry"
        );
        assert!(
            !acknowledge_reply_chunk(&mut server, &new_one, now),
            "the queued reconnect receipt is terminal after the old exact item wins"
        );
        assert!(server.outbound.is_empty());

        {
            let (_, requester, _, request, now) = start_session(1, 3);
            let local_peer = request.responder.clone();
            let mut routes =
                NetworkReplyRouteTestFixture::new(peer(b"receipt reconnect-before-redrain hub"));
            let old_route = routes.mint(requester.clone());
            let mut server = MergeSidecarTransport::new();
            assert!(matches!(
                server
                    .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now,)
                    .expect("admit the overlapping old tenure"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(request.clone(), Some(old_route.clone()), vec![0xC8], now)
                .expect("materialize the overlapping response");
            let old_post = server
                .drain_outbound_chunks(1, now)
                .pop()
                .expect("hand the old current item to exact output");
            let old_receipt = reply_chunk_admission(&old_post);
            let reconnected = routes.mint(requester.clone());
            assert!(matches!(
                server
                    .admit_server_request(
                        &requester,
                        &request,
                        Some(&reconnected),
                        &local_peer,
                        now,
                    )
                    .expect("overlapping reconnect requeues the retained current item"),
                ServerRequestAdmission::Existing
            ));
            assert!(
                server
                    .acknowledge_outbound_chunk(&old_receipt, now)
                    .expect("the old successful flush wins before reconnect redrain")
            );
            assert!(
                !server
                    .acknowledge_outbound_chunk(&old_receipt, now)
                    .expect("the pre-redrain receipt is consumed once")
            );
            assert!(server.outbound.is_empty());
            assert!(server.outbound_order.is_empty());
        }

        {
            let (_, requester, _, request, now) = start_session(1, 3);
            let local_peer = request.responder.clone();
            let mut routes =
                NetworkReplyRouteTestFixture::new(peer(b"receipt prune-before-reconnect hub"));
            let old_route = routes.mint(requester.clone());
            let source = ServerRequestSource::Authenticated(old_route.source_key());
            let key = (requester.clone(), request.request_id);
            let mut server = MergeSidecarTransport::new();
            assert!(matches!(
                server
                    .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now,)
                    .expect("admit the prune-race source"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(request.clone(), Some(old_route.clone()), vec![0xD1], now)
                .expect("materialize the prune-race response");
            let old_post = server
                .drain_outbound_chunks(1, now)
                .pop()
                .expect("hand the final old-tenure chunk to exact output");
            let old_receipt = reply_chunk_admission(&old_post);
            assert!(routes.retire(&old_route));
            assert!(
                server
                    .tick_bounded(&local_peer, now, 0)
                    .expect("reclaim retired source")
                    .is_empty()
            );
            assert!(!server.outbound.contains_key(&key));
            assert!(server.outbound_order.is_empty());
            assert_eq!(
                server.server_request_gates[&key].attempts[&source].cursor,
                ServerResponseCursor::Pending(0)
            );
            assert!(
                server.acknowledge_outbound_chunk(&old_receipt, now).expect(
                    "the durable marker accepts the old successful flush after reclamation"
                )
            );
            assert!(server.outbound.is_empty());
            assert_eq!(
                server.server_request_gates[&key].attempts[&source].cursor,
                ServerResponseCursor::Complete
            );
            assert!(
                server.server_request_gates[&key].attempts[&source]
                    .pending_flush_chunk
                    .is_none()
            );
            assert!(
                !server
                    .acknowledge_outbound_chunk(&old_receipt, now)
                    .expect("the exact old receipt is consumed only once")
            );
            let reconnected = routes.mint(requester.clone());
            assert!(matches!(
                server
                    .admit_server_request(
                        &requester,
                        &request,
                        Some(&reconnected),
                        &local_peer,
                        now,
                    )
                    .expect("the completed source remains terminal after reconnect"),
                ServerRequestAdmission::Existing
            ));
        }

        {
            let (_, _, _, base, now) = start_session(1, 3);
            let local_peer = base.responder.clone();
            let requester_a = peer(b"receipt rematerialization origin a");
            let requester_b = peer(b"receipt rematerialization origin b");
            let request_a = routed_server_request(
                &base,
                requester_a.clone(),
                b"receipt rematerialization request a",
                1,
            );
            let request_b = routed_server_request(
                &base,
                requester_b.clone(),
                b"receipt rematerialization request b",
                1,
            );
            let hub_a = peer(b"receipt rematerialization hub a");
            let hub_b = peer(b"receipt rematerialization hub b");
            let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
            let route_a = routes.mint_via(requester_a.clone(), hub_a);
            let route_b = routes.mint_via(requester_b.clone(), hub_b);
            let key_a = (requester_a.clone(), request_a.request_id);
            let key_b = (requester_b.clone(), request_b.request_id);
            let mut server = MergeSidecarTransport::new();
            for (requester, request, route, byte) in [
                (&requester_a, &request_a, &route_a, 0xE1),
                (&requester_b, &request_b, &route_b, 0xE2),
            ] {
                assert!(matches!(
                    server
                        .admit_server_request(requester, request, Some(route), &local_peer, now,)
                        .expect("admit an independent rematerialization source"),
                    ServerRequestAdmission::Materialize
                ));
                server
                    .enqueue_response(request.clone(), Some(route.clone()), vec![byte], now)
                    .expect("materialize one independent response");
            }
            let posts = server.drain_outbound_chunks(2, now);
            let old_a = posts
                .iter()
                .find(|post| post.peer == requester_a)
                .expect("source A owns its old writer item");
            let sibling_b = posts
                .iter()
                .find(|post| post.peer == requester_b)
                .expect("source B owns an independent writer item");
            let old_a_receipt = reply_chunk_admission(old_a);
            let sibling_b_receipt = reply_chunk_admission(sibling_b);

            assert!(routes.retire(&route_a));
            assert!(
                server
                    .tick_bounded(&local_peer, now, 0)
                    .expect("reclaim retired source")
                    .is_empty()
            );
            assert!(!server.outbound.contains_key(&key_a));
            assert!(server.outbound.contains_key(&key_b));
            let reconnected_a = routes.mint(requester_a.clone());
            assert!(matches!(
                server
                    .admit_server_request(
                        &requester_a,
                        &request_a,
                        Some(&reconnected_a),
                        &local_peer,
                        now,
                    )
                    .expect("reconnect makes source A retryable"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(
                    request_a.clone(),
                    Some(reconnected_a.clone()),
                    vec![0xE1],
                    now,
                )
                .expect("source A rematerializes identical response bytes");
            let retry_a = server
                .drain_outbound_chunks(1, now)
                .pop()
                .expect("reconnect retries source A's rematerialized current item");
            assert!(matches!(
                &retry_a,
                MergeSidecarPost {
                    reply_route: Some(route),
                    ..
                } if route.same_delivery(&reconnected_a)
            ));
            assert!(
                server
                    .acknowledge_outbound_chunk(&old_a_receipt, now)
                    .expect("old flush wins before the reconnect retry completes")
            );
            assert!(
                !server
                    .acknowledge_outbound_chunk(&old_a_receipt, now)
                    .expect("the pre-rematerialization receipt advances exactly once")
            );
            assert!(
                !acknowledge_reply_chunk(&mut server, &retry_a, now),
                "the queued reconnect retry is terminal after the old flush wins"
            );
            assert!(!server.outbound.contains_key(&key_a));
            assert!(server.outbound.contains_key(&key_b));
            assert!(
                server
                    .acknowledge_outbound_chunk(&sibling_b_receipt, now)
                    .expect("source B progresses independently after source A completes")
            );
            assert!(!server.outbound.contains_key(&key_b));
            assert!(server.outbound_order.is_empty());
        }
    }

    #[test]
    fn later_delivery_during_materialization_keeps_exact_authorized_route() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"pending delivery hub"));
        let admitted_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&admitted_route),
                    &local_peer,
                    now,
                )
                .expect("start one semantic materialization"),
            ServerRequestAdmission::Materialize
        ));

        let later_route = routes
            .redeliver(&admitted_route)
            .expect("mint later delivery while local work is pending");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("coalesce the later delivery into pending work"),
            ServerRequestAdmission::Existing
        ));
        let key = (requester.clone(), request.request_id);
        let source = ServerRequestSource::Authenticated(admitted_route.source_key());
        let attempt = &server.server_request_gates[&key].attempts[&source];
        assert!(attempt.materialization_authorized);
        assert!(
            attempt
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&admitted_route))
        );
        assert!(
            attempt
                .authorized_materialization_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&admitted_route))
        );
        let authorized_route = attempt
            .authorized_materialization_route
            .clone()
            .expect("authorized route");
        server
            .enqueue_response(request, Some(admitted_route), vec![0x7A], now)
            .expect("the original work authorization remains consumable");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("materialized output keeps the exact authorized delivery route");
        assert!(matches!(
            &post,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&authorized_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0 && chunk.bytes.as_slice() == [0x7A])
        ));
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn writable_reconnect_during_materialization_keeps_exact_authorized_tenure() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"materialization reconnect hub"));
        let admitted_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&admitted_route),
                    &local_peer,
                    now,
                )
                .expect("authorize one immutable materialization"),
            ServerRequestAdmission::Materialize
        ));
        let reconnected = routes.mint(requester.clone());
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&reconnected), &local_peer, now)
                .expect("new tenure coalesces behind the already-running materialization"),
            ServerRequestAdmission::Existing
        ));
        let key = (requester.clone(), request.request_id);
        let source = ServerRequestSource::Authenticated(reconnected.source_key());
        let attempt = &server.server_request_gates[&key].attempts[&source];
        assert_eq!(attempt.cursor, ServerResponseCursor::Pending(0));
        assert!(attempt.materialization_authorized);
        assert!(
            attempt
                .authorized_materialization_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&admitted_route))
        );
        assert!(
            attempt
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&admitted_route))
        );
        server
            .enqueue_response(request, Some(admitted_route.clone()), vec![0x6C], now)
            .expect("the original authorization may finish without a second Kura lookup");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("finished bytes emit only on the exact authorized tenure");
        assert!(matches!(
            &post,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&admitted_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0 && chunk.bytes.as_slice() == [0x6C])
        ));
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
    }

    #[test]
    fn equal_sequence_with_different_semantic_identity_is_rejected_before_materialization() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("admit first exact request"),
            ServerRequestAdmission::Materialize
        ));
        let mut conflicting = request;
        conflicting.reference_digest = Hash::new(b"conflicting sidecar reference");
        conflicting.bind_canonical_request_id();
        assert!(matches!(
            server.admit_server_request(&requester, &conflicting, None, &local_peer, now,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert_eq!(server.server_request_gates.len(), 1);
    }

    #[test]
    fn request_stream_close_floor_advances_only_over_a_contiguous_terminal_prefix() {
        let mut stream = RequestStreamState::new(stream_epoch(1));
        let (first, first_floor) = stream.allocate().expect("allocate sequence one");
        let (second, second_floor) = stream.allocate().expect("allocate sequence two");
        let (third, third_floor) = stream.allocate().expect("allocate sequence three");
        assert_eq!((first.get(), second.get(), third.get()), (1, 2, 3));
        assert_eq!((first_floor, second_floor, third_floor), (0, 0, 0));

        stream.close(second);
        assert_eq!(stream.closed_through, 0);
        stream.close(first);
        assert_eq!(stream.closed_through, 2);
        stream.close(third);
        assert_eq!(stream.closed_through, 3);
        assert!(stream.open_sequences.is_empty());
    }

    #[test]
    fn authenticated_close_floor_retires_covered_output_and_rejects_replay_or_regression() {
        let (_, requester, _, first, now) = start_session(1, 3);
        let local_peer = first.responder.clone();
        let first_key = (requester.clone(), first.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &first, None, &local_peer, now)
                .expect("admit sequence one"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(first.clone(), None, vec![0x41], now)
            .expect("retain sequence-one output");
        assert!(server.outbound.contains_key(&first_key));

        let mut second = first.clone();
        second.semantic_sequence = semantic_sequence(2);
        second.closed_through = 1;
        second.bind_canonical_request_id();
        let second_key = (requester.clone(), second.request_id);
        assert_ne!(
            second.request_id, first.request_id,
            "every lifecycle occurrence has a distinct canonical identity"
        );
        assert!(matches!(
            server
                .admit_server_request(&requester, &second, None, &local_peer, now)
                .expect("the next occurrence authenticates the cumulative close floor"),
            ServerRequestAdmission::Materialize
        ));
        assert_eq!(
            server
                .server_streams
                .get(&requester)
                .map(|stream| stream.closed_through),
            Some(1)
        );
        assert!(!server.outbound.contains_key(&first_key));
        assert!(!server.server_request_gates.contains_key(&first_key));
        assert_eq!(
            server.server_request_gates[&second_key]
                .semantic_sequence
                .get(),
            2,
            "the new occurrence owns a distinct exact gate"
        );
        assert!(server.drain_outbound_chunks(usize::MAX, now).is_empty());

        assert!(matches!(
            server.admit_server_request(&requester, &first, None, &local_peer, now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));

        let mut regressed = second;
        regressed.semantic_sequence = semantic_sequence(3);
        regressed.closed_through = 0;
        regressed.bind_canonical_request_id();
        assert!(matches!(
            server.admit_server_request(&requester, &regressed, None, &local_peer, now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert_eq!(
            server
                .server_streams
                .get(&requester)
                .map(|stream| stream.closed_through),
            Some(1)
        );
    }

    #[test]
    fn same_occurrence_advances_piggybacked_floor_without_rematerializing_current_output() {
        let (_, requester, _, first, now) = start_session(1, 3);
        let responder = first.responder.clone();
        let first_key = (requester.clone(), first.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &first, None, &responder, now)
                .expect("admit the first occurrence"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(first.clone(), None, vec![0x31], now)
            .expect("materialize the first occurrence");

        let mut current = first.clone();
        current.semantic_sequence = semantic_sequence(2);
        current.closed_through = 0;
        current.bind_canonical_request_id();
        let current_key = (requester.clone(), current.request_id);
        assert!(matches!(
            server
                .admit_server_request(&requester, &current, None, &responder, now)
                .expect("admit the current occurrence"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(current.clone(), None, vec![0x32], now)
            .expect("materialize the current occurrence once");
        let current_chunk = Arc::clone(&server.outbound[&current_key].chunks[0]);
        let current_chunk_ptr = Arc::as_ptr(&current_chunk);
        let current_source = ServerRequestSource::Synthetic(requester.clone());
        let current_attempt = &server.outbound[&current_key].attempts[&current_source];
        let current_attempt_state = (
            current_attempt.next_chunk,
            current_attempt.in_flight_chunk,
            current_attempt.queued,
        );

        let mut advanced = current.clone();
        advanced.closed_through = 1;
        assert_eq!(
            advanced.canonical_request_id(),
            current.request_id,
            "the cumulative close floor is excluded from occurrence identity"
        );
        assert!(matches!(
            server
                .admit_server_request(&requester, &advanced, None, &responder, now)
                .expect("advance the floor on the exact retained occurrence"),
            ServerRequestAdmission::Existing
        ));

        assert!(!server.server_request_gates.contains_key(&first_key));
        assert!(!server.outbound.contains_key(&first_key));
        assert_eq!(
            server.server_streams[&requester].closed_through, 1,
            "the same occurrence advances the authenticated stream floor"
        );
        assert_eq!(
            server.server_request_gates[&current_key].request, advanced,
            "the gate retains the latest whole-message hash and floor"
        );
        assert_eq!(
            server.server_request_gates[&current_key].request_hash,
            HashOf::new(&advanced)
        );
        assert_eq!(server.outbound[&current_key].request, advanced);
        let retained_attempt = &server.outbound[&current_key].attempts[&current_source];
        assert_eq!(
            (
                retained_attempt.next_chunk,
                retained_attempt.in_flight_chunk,
                retained_attempt.queued,
            ),
            current_attempt_state
        );
        assert_eq!(
            Arc::as_ptr(&server.outbound[&current_key].chunks[0]),
            current_chunk_ptr,
            "advancing only the floor must not rematerialize response chunks"
        );
        assert_eq!(
            server.drain_closed_server_prefixes(),
            vec![CertifiedMergeSidecarClosedPrefix {
                requester,
                service_generation: current.service_generation,
                stream_epoch: current.stream_epoch,
                closed_through: 1,
            }]
        );
    }

    #[test]
    fn delayed_same_payload_flush_cannot_advance_the_successor_occurrence() {
        let (_, requester, _, first, now) = start_session(1, 3);
        let local_peer = first.responder.clone();
        let hub = peer(b"same-payload delayed flush hub");
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(hub, DEFAULT_REPLY_SOURCE_CAPACITY);
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &first, Some(&route), &local_peer, now,)
                .expect("admit the first payload occurrence"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(first.clone(), Some(route.clone()), vec![0x41], now)
            .expect("materialize first occurrence");
        let old_post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand the first occurrence to exact output");
        let old_admission = reply_chunk_admission(&old_post);

        let mut successor = first.clone();
        successor.semantic_sequence = semantic_sequence(
            first
                .semantic_sequence
                .get()
                .checked_add(1)
                .expect("test semantic sequence has a successor"),
        );
        successor.closed_through = first.semantic_sequence.get();
        successor.bind_canonical_request_id();
        assert_ne!(successor.request_id, first.request_id);
        assert!(matches!(
            server
                .admit_server_request(&requester, &successor, Some(&route), &local_peer, now,)
                .expect("the successor closes and replaces old output"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(successor.clone(), Some(route.clone()), vec![0x41], now)
            .expect("materialize the successor occurrence");
        let successor_post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand the successor occurrence to exact output");
        let successor_key = (requester.clone(), successor.request_id);
        let source = ServerRequestSource::Authenticated(route.source_key());
        let before = server.server_request_gates[&successor_key].attempts[&source]
            .pending_flush_chunk
            .clone();

        assert!(
            !server
                .acknowledge_outbound_chunk(&old_admission, now)
                .expect("the covered old occurrence is a consumed no-op")
        );
        let successor_attempt = &server.server_request_gates[&successor_key].attempts[&source];
        assert_eq!(successor_attempt.cursor, ServerResponseCursor::Pending(0));
        assert_eq!(successor_attempt.pending_flush_chunk, before);
        assert!(
            acknowledge_reply_chunk(&mut server, &successor_post, now),
            "only the exact successor flush advances its cursor"
        );
    }

    #[test]
    fn standalone_close_retries_until_exact_ack_then_terminates() {
        let (mut client, requester, _, request, now) = start_session(1, 3);
        let responder = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &responder, now)
                .expect("server observes sequence one"),
            ServerRequestAdmission::Materialize
        ));
        client.close_request_sequence(&responder, request.stream_epoch, request.semantic_sequence);

        let close_post = client
            .tick_bounded(&requester, now, 1)
            .expect("schedule standalone close")
            .pop()
            .expect("terminal local work emits a standalone close");
        let CertifiedMergeSidecarMessage::Close(close) = Arc::unwrap_or_clone(close_post.message)
        else {
            panic!("close work must not be encoded as a data request")
        };
        assert_eq!(close_post.peer, responder);
        assert!(close_post.reply_route.is_none());

        let mut reply_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            requester.clone(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
        );
        let reply_route = reply_routes.mint(requester.clone());
        let ack_post = server
            .admit_server_close(&requester, &close, Some(&reply_route), &responder)
            .expect("server applies the authenticated close");
        assert!(
            ack_post
                .reply_route
                .as_ref()
                .is_some_and(|retained| retained.same_delivery(&reply_route))
        );
        assert_eq!(
            server.drain_closed_server_prefixes(),
            vec![CertifiedMergeSidecarClosedPrefix {
                requester: requester.clone(),
                service_generation: request.service_generation,
                stream_epoch: request.stream_epoch,
                closed_through: request.semantic_sequence.get(),
            }]
        );
        let CertifiedMergeSidecarMessage::CloseAck(ack) = Arc::unwrap_or_clone(ack_post.message)
        else {
            panic!("standalone close must produce an explicit ACK")
        };
        assert!(
            client
                .acknowledge_close(&responder, &ack, &requester)
                .expect("accept exact close ACK")
        );
        assert!(
            client
                .tick_bounded(&requester, now + REQUEST_TIMEOUT, 1)
                .expect("service acknowledged close stream")
                .into_iter()
                .all(|post| !matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Close(_)
                )),
            "an exact ACK terminates local close retry work"
        );

        let duplicate_ack = server
            .admit_server_close(&requester, &close, Some(&reply_route), &responder)
            .expect("an exact close retry remains idempotent");
        assert!(
            duplicate_ack
                .reply_route
                .as_ref()
                .is_some_and(|retained| retained.same_delivery(&reply_route))
        );
        assert!(server.drain_closed_server_prefixes().is_empty());
        let CertifiedMergeSidecarMessage::CloseAck(duplicate_ack) =
            Arc::unwrap_or_clone(duplicate_ack.message)
        else {
            unreachable!("idempotent close returns the same ACK kind")
        };
        assert!(
            !client
                .acknowledge_close(&responder, &duplicate_ack, &requester)
                .expect("duplicate ACK is a harmless no-op")
        );
    }

    #[test]
    fn close_covers_allocated_but_unsent_sequence_after_requester_recovery() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let responder = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &responder, now)
                .expect("server observes sequence one"),
            ServerRequestAdmission::Materialize
        ));
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request
                .semantic_sequence
                .get()
                .checked_add(1)
                .expect("test semantic sequence has a successor"),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        close.bind_canonical_close_id();
        let ack = server
            .admit_server_close(&requester, &close, None, &responder)
            .expect("the authenticated requester owns its crash-recovered close floor");
        assert!(matches!(
            ack.message.as_ref(),
            CertifiedMergeSidecarMessage::CloseAck(ack)
                if ack.service_generation == close.service_generation
                    && ack.stream_epoch == close.stream_epoch
                    && ack.closed_through == close.closed_through
        ));
        assert_eq!(
            server
                .server_streams
                .get(&requester)
                .map(|stream| (stream.closed_through, stream.highest_sequence)),
            Some((close.closed_through, close.closed_through))
        );
        assert_eq!(
            server
                .pending_server_closures
                .get(&requester)
                .map(|prefix| prefix.closed_through),
            Some(close.closed_through)
        );
        assert!(server.server_request_gates.is_empty());
    }

    #[test]
    fn higher_stream_epoch_atomically_retires_old_output_and_rejects_stale_control() {
        let (_, requester, _, first, now) = start_session(1, 3);
        let responder = first.responder.clone();
        let first_key = (requester.clone(), first.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &first, None, &responder, now)
                .expect("admit the first stream incarnation"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(first.clone(), None, vec![0xA1], now)
            .expect("retain first-incarnation response output");
        assert!(server.outbound.contains_key(&first_key));

        let mut successor = first.clone();
        successor.stream_epoch = successor_stream_epoch(first.stream_epoch);
        successor.semantic_sequence = semantic_sequence(1);
        successor.closed_through = 0;
        successor.bind_canonical_request_id();
        let successor_key = (requester.clone(), successor.request_id);
        assert_ne!(successor.request_id, first.request_id);
        assert!(matches!(
            server
                .admit_server_request(&requester, &successor, None, &responder, now)
                .expect("a higher authenticated epoch supersedes old ownership"),
            ServerRequestAdmission::Materialize
        ));
        assert!(!server.server_request_gates.contains_key(&first_key));
        assert!(!server.outbound.contains_key(&first_key));
        assert!(server.server_request_gates.contains_key(&successor_key));
        assert_eq!(
            server.server_streams.get(&requester).copied(),
            Some(ServerStreamState {
                stream_epoch: successor.stream_epoch,
                closed_through: 0,
                highest_sequence: 1,
            })
        );
        assert_eq!(
            server.drain_closed_server_prefixes(),
            vec![CertifiedMergeSidecarClosedPrefix {
                requester: requester.clone(),
                service_generation: first.service_generation,
                stream_epoch: first.stream_epoch,
                closed_through: first.semantic_sequence.get(),
            }]
        );

        let state_before_replay = server.server_streams[&requester];
        assert!(matches!(
            server.admit_server_request(&requester, &first, None, &responder, now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        let mut stale_close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: first.service_generation,
            stream_epoch: first.stream_epoch,
            closed_through: first.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        stale_close.bind_canonical_close_id();
        assert!(matches!(
            server.admit_server_close(&requester, &stale_close, None, &responder),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert_eq!(server.server_streams[&requester], state_before_replay);
        assert!(server.server_request_gates.contains_key(&successor_key));
        assert!(server.pending_server_closures.is_empty());
    }

    #[test]
    fn higher_stream_epoch_capacity_rejection_is_fail_atomic() {
        let (_, requester, _, first, now) = start_session(1, 3);
        let responder = first.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &first, None, &responder, now)
                .expect("admit old requester stream"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(first.clone(), None, vec![0xB1], now)
            .expect("retain old requester output");

        let other_requester = peer(b"epoch capacity unrelated requester");
        let other = routed_server_request(
            &first,
            other_requester.clone(),
            b"epoch capacity unrelated",
            1,
        );
        assert!(matches!(
            server
                .admit_server_request(&other_requester, &other, None, &responder, now)
                .expect("admit unrelated capacity owner"),
            ServerRequestAdmission::Materialize
        ));
        server.server_request_gate_capacity = 1;

        let mut successor = first.clone();
        successor.stream_epoch = successor_stream_epoch(first.stream_epoch);
        successor.semantic_sequence = semantic_sequence(1);
        successor.closed_through = 0;
        successor.bind_canonical_request_id();
        let old_key = (requester.clone(), first.request_id);
        let unrelated_key = (other_requester, other.request_id);
        assert!(matches!(
            server.admit_server_request(&requester, &successor, None, &responder, now),
            Err(MergeSidecarError::Capacity("server request gate geometry"))
        ));
        assert_eq!(
            server.server_streams[&requester].stream_epoch,
            first.stream_epoch
        );
        assert!(server.server_request_gates.contains_key(&old_key));
        assert!(server.server_request_gates.contains_key(&unrelated_key));
        assert!(server.outbound.contains_key(&old_key));
        assert!(server.pending_server_closures.is_empty());
    }

    #[test]
    fn stale_close_ack_cannot_terminate_a_reallocated_stream_epoch() {
        let requester = peer(b"epoch ACK requester");
        let responder = peer(b"epoch ACK responder");
        let now = Instant::now();
        let mut transport = MergeSidecarTransport::new();
        let (first_epoch, first_sequence, first_floor) = transport
            .allocate_request_sequence(&responder)
            .expect("allocate first stream incarnation");
        assert_eq!((first_sequence.get(), first_floor), (1, 0));
        transport.close_request_sequence(&responder, first_epoch, first_sequence);
        let first_close = transport
            .begin_close(&requester, &responder, now)
            .and_then(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Close(close) => Some(close),
                _ => None,
            })
            .expect("emit first stream close");
        let first_ack = CertifiedMergeSidecarCloseAckV1 {
            version: first_close.version,
            service_generation: first_close.service_generation,
            stream_epoch: first_close.stream_epoch,
            closed_through: first_close.closed_through,
            close_id: first_close.close_id,
            requester: first_close.requester,
            responder: first_close.responder,
        };
        assert!(
            transport
                .acknowledge_close(&responder, &first_ack, &requester)
                .expect("acknowledge and reclaim first stream")
        );
        assert!(!transport.request_streams.contains_key(&responder));

        let (successor_epoch, successor_sequence, successor_floor) = transport
            .allocate_request_sequence(&responder)
            .expect("allocate successor stream incarnation");
        assert_eq!(successor_epoch, successor_stream_epoch(first_epoch));
        assert_eq!((successor_sequence.get(), successor_floor), (1, 0));
        assert!(matches!(
            transport.acknowledge_close(&responder, &first_ack, &requester),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        let successor = &transport.request_streams[&responder];
        assert_eq!(successor.stream_epoch, successor_epoch);
        assert_eq!(
            successor.open_sequences,
            BTreeSet::from([successor_sequence])
        );
        assert_eq!(successor.acknowledged_through, 0);
    }

    #[test]
    fn authenticated_generation_hint_retires_old_attempt_before_reissue() {
        let (mut client, requester, _, first, now) = start_session(1, 1);
        let responder = first.responder.clone();
        let old_epoch = first.stream_epoch;
        let hint = generation_hint_for_request(&first, service_generation(2));

        assert!(
            client
                .acknowledge_generation_hint(&responder, &hint, &requester)
                .expect("apply an exact newer responder fence")
        );
        let stream = &client.request_streams[&responder];
        assert_eq!(stream.service_generation, hint.current_generation);
        assert_eq!(stream.stream_epoch, successor_stream_epoch(old_epoch));
        assert_eq!(stream.next_sequence, 0);
        assert!(stream.open_sequences.is_empty());
        assert!(
            client
                .inbound
                .values()
                .all(|assembly| assembly.current.is_none()),
            "old-generation assembly ownership is retired before retry"
        );

        let retried = client
            .tick_bounded(&requester, now, 1)
            .expect("schedule retry under the durable generation fence")
            .pop()
            .expect("the sole holder is retried immediately");
        let CertifiedMergeSidecarMessage::Request(retried) = Arc::unwrap_or_clone(retried.message)
        else {
            panic!("generation retry must emit a request")
        };
        assert_eq!(retried.service_generation, hint.current_generation);
        assert_eq!(retried.stream_epoch, successor_stream_epoch(old_epoch));
        assert_eq!(retried.semantic_sequence.get(), 1);
        assert_ne!(retried.request_id, first.request_id);

        let stale_chunk = chunks(&first, &[0x51]).remove(0);
        assert_eq!(
            client.ingest_chunk(&responder, stale_chunk, now),
            Err(MergeSidecarError::RequestIdMismatch),
            "the old-generation occurrence must not alias the reissued request identity"
        );
        assert!(
            !client
                .acknowledge_generation_hint(&responder, &hint, &requester)
                .expect("a duplicate older observation is a no-op")
        );

        let wrong_responder = peer(b"forged generation Hint responder");
        assert_eq!(
            client.acknowledge_generation_hint(&wrong_responder, &hint, &requester),
            Err(MergeSidecarError::PeerIdentityMismatch),
            "only the expected authenticated responder may advance its fence"
        );
        let mut forged_id = generation_hint_for_request(&retried, service_generation(3));
        forged_id.hint_id = Hash::new(b"forged generation Hint identity");
        assert_eq!(
            client.acknowledge_generation_hint(&responder, &forged_id, &requester),
            Err(MergeSidecarError::RequestIdMismatch),
            "a self-described but unauthentic Hint identity fails closed"
        );
        let mut uncorrelated = generation_hint_for_request(&retried, service_generation(3));
        uncorrelated.observed_message_hash = Hash::new(b"uncorrelated outstanding sidecar message");
        uncorrelated.bind_canonical_hint_id();
        assert!(
            !client
                .acknowledge_generation_hint(&responder, &uncorrelated, &requester)
                .expect("a canonical Hint must still name exact outstanding work")
        );

        let mut unaffiliated = generation_hint_for_request(&retried, service_generation(3));
        unaffiliated.observed_generation = service_generation(3);
        unaffiliated.bind_canonical_hint_id();
        assert!(
            !client
                .acknowledge_generation_hint(&responder, &unaffiliated, &requester)
                .expect("a Hint for a generation never issued locally is ignored")
        );
        assert_eq!(
            client.request_streams[&responder].service_generation,
            hint.current_generation
        );
    }

    #[test]
    fn stale_close_yields_an_exact_generation_hint_without_allocating_server_state() {
        let (mut client, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        client
            .release_unsent_request(&request)
            .expect("retire the unsent request occurrence");
        let close_post = client
            .begin_close(&requester, &responder, now)
            .expect("emit the cumulative close");
        let CertifiedMergeSidecarMessage::Close(close) = Arc::unwrap_or_clone(close_post.message)
        else {
            panic!("request retirement must emit a Close")
        };

        let mut server = MergeSidecarTransport::new();
        server.server_service_generation = service_generation(2);
        let hub = peer(b"stale Close relay hub");
        let mut reply_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            hub.clone(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
        );
        let reply_route = reply_routes.mint_via(requester.clone(), hub);
        let first_hint_post = server
            .admit_server_close(&requester, &close, Some(&reply_route), &responder)
            .expect("stale Close receives the current responder fence");
        assert!(
            first_hint_post
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&reply_route)),
            "GenerationHint retains the exact authenticated route of its stale Close"
        );
        let CertifiedMergeSidecarMessage::GenerationHint(hint) =
            Arc::unwrap_or_clone(first_hint_post.message)
        else {
            panic!("a stale Close must not receive a CloseAck")
        };
        assert_eq!(hint.observed_generation, close.service_generation);
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(hint.observed_message_hash, HashOf::new(&close).into());
        assert_eq!(hint.hint_id, hint.canonical_hint_id());
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());

        let repeated = server
            .admit_server_close(&requester, &close, Some(&reply_route), &responder)
            .expect("stale Close replay is answered statelessly");
        assert_eq!(
            repeated,
            MergeSidecarPost {
                peer: requester.clone(),
                reply_route: Some(reply_route),
                message: Arc::new(CertifiedMergeSidecarMessage::GenerationHint(hint.clone())),
            }
        );
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());

        let mut unrelated_hash = hint.clone();
        unrelated_hash.observed_message_hash = Hash::new(b"unrelated stale Close");
        unrelated_hash.bind_canonical_hint_id();
        assert!(
            !client
                .acknowledge_generation_hint(&responder, &unrelated_hash, &requester)
                .expect("a canonical but unaffiliated Hint is a no-op")
        );
        assert!(
            client
                .acknowledge_generation_hint(&responder, &hint, &requester)
                .expect("the exact authenticated Close Hint installs the fence")
        );
        let replacement = &client.request_streams[&responder];
        assert_eq!(replacement.service_generation, hint.current_generation);
        assert_eq!(
            replacement.stream_epoch,
            successor_stream_epoch(close.stream_epoch)
        );
        assert_eq!(replacement.next_sequence, 0);
        assert_eq!(replacement.closed_through, 0);
        assert_eq!(replacement.acknowledged_through, 0);
        assert!(replacement.open_sequences.is_empty());
    }

    #[test]
    fn future_service_generation_is_rejected_without_hint_or_server_state() {
        let (_, requester, _, mut request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        request.service_generation = service_generation(2);
        request.bind_canonical_request_id();
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        close.bind_canonical_close_id();
        let mut server = MergeSidecarTransport::new();

        assert!(
            matches!(
                server.admit_server_request(&requester, &request, None, &responder, now),
                Err(MergeSidecarError::UnsolicitedResponse)
            ),
            "a responder must never advertise a lower generation to future-generation traffic"
        );
        assert_eq!(
            server.admit_server_close(&requester, &close, None, &responder),
            Err(MergeSidecarError::UnsolicitedResponse)
        );
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
        assert!(server.pending_server_closures.is_empty());
    }

    #[test]
    fn stale_request_replay_is_stateless_under_an_obstructed_lifecycle_journal() {
        let temp = tempfile::tempdir().expect("temp dir");
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open the durable responder");
        let successor_roster = vec![peer(b"stale replay successor roster")];
        server
            .transition_server_service_generation_for_test(&successor_roster)
            .expect("persist the successor responder generation");
        let before = server
            .lifecycle_snapshot()
            .expect("snapshot the quiescent successor generation");
        server.obstruct_lifecycle_journal_temp_for_test();

        let (_, _, _, base_request, now) = start_session(1, 1);
        let responder = base_request.responder.clone();
        let hub = peer(b"stale generation replay hub");
        let mut reply_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            hub.clone(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
        );
        for index in 0..64 {
            let requester = peer(format!("stale generation replay requester {index}").as_bytes());
            let request =
                routed_server_request(&base_request, requester.clone(), b"stale replay", 1);
            let reply_route = reply_routes.mint_via(requester.clone(), hub.clone());
            let admission = server
                .admit_server_request(&requester, &request, Some(&reply_route), &responder, now)
                .expect("a canonical stale request is a stateless generation probe");
            let ServerRequestAdmission::GenerationHint(post) = admission else {
                panic!("stale generation replay must receive an exact Hint")
            };
            assert!(
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&reply_route)),
                "each stateless Hint retains the triggering authenticated delivery"
            );
            let CertifiedMergeSidecarMessage::GenerationHint(hint) = post.message.as_ref() else {
                panic!("stale generation replay must not allocate response work")
            };
            assert_eq!(hint.observed_generation, request.service_generation);
            assert_eq!(hint.current_generation, server.server_service_generation);
            assert_eq!(hint.observed_message_hash, HashOf::new(&request).into());
        }
        let synthetic_requester = peer(b"synthetic stale generation replay requester");
        let synthetic_request = routed_server_request(
            &base_request,
            synthetic_requester.clone(),
            b"synthetic stale replay",
            1,
        );
        assert!(matches!(
            server.admit_server_request(
                &synthetic_requester,
                &synthetic_request,
                None,
                &responder,
                now,
            ),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        let synthetic_close = close_for_request(&synthetic_request);
        assert!(matches!(
            server.admit_server_close(&synthetic_requester, &synthetic_close, None, &responder,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after stale replay pressure"),
            before
        );
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
        assert!(server.pending_server_closures.is_empty());
    }

    #[test]
    fn generation_rollover_consumes_a_late_writer_flush_without_mutating_the_successor() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let hub = peer(b"generation rollover delayed writer hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("admit the old-generation response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0xD7], now)
            .expect("materialize the old-generation response");
        let old_post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand the old-generation chunk to its writer");
        let late_flush = reply_chunk_admission(&old_post);
        assert!(routes.retire(&route));
        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: request.service_generation,
            stream_epoch: request.stream_epoch,
            closed_through: request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: requester.clone(),
            responder: responder.clone(),
        };
        close.bind_canonical_close_id();
        server
            .admit_server_close(&requester, &close, None, &responder)
            .expect("terminate the old-generation response before compaction");
        assert_eq!(server.drain_closed_server_prefixes().len(), 1);

        for index in 0..MAX_CERTIFIED_MERGE_SEMANTIC_PEERS - 1 {
            server.server_streams.insert(
                peer(format!("generation rollover terminal requester {index}").as_bytes()),
                ServerStreamState {
                    stream_epoch: stream_epoch(1),
                    closed_through: 1,
                    highest_sequence: 1,
                },
            );
        }
        assert_eq!(
            server.server_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );

        let successor_requester = peer(b"generation rollover successor requester");
        let mut successor =
            routed_server_request(&request, successor_requester.clone(), b"successor", 1);
        assert!(matches!(
            server.admit_server_request(&successor_requester, &successor, None, &responder, now),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            "a drained prefix is not terminal until exact output confirms cancellation"
        );
        assert_eq!(
            server.server_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );
        server.confirm_closed_server_prefix_handoff();
        let terminal_predecessor = server
            .lifecycle_snapshot()
            .expect("snapshot the externally terminal full table");
        assert!(matches!(
            server.admit_server_request(&successor_requester, &successor, None, &responder, now),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after terminal same-roster rejection"),
            terminal_predecessor,
            "external terminality alone cannot advance a responder generation"
        );

        let changed_roster = vec![successor_requester.clone()];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        let mut server = server
            .rehydrate_with_exact_geometry(
                DEFAULT_REPLY_SOURCE_CAPACITY,
                MergeSidecarLimits::defaults(),
                changed_roster.len(),
                changed_digest,
                now,
            )
            .expect("a certified changed roster advances the terminal responder generation");
        let mut successor_routes =
            NetworkReplyRouteTestFixture::new(peer(b"generation rollover successor reply hub"));
        let successor_route = successor_routes.mint(successor_requester.clone());
        let ServerRequestAdmission::GenerationHint(post) = server
            .admit_server_request(
                &successor_requester,
                &successor,
                Some(&successor_route),
                &responder,
                now,
            )
            .expect("the old-generation successor receives the changed-roster fence")
        else {
            panic!("the old-generation successor must receive a Hint")
        };
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = Arc::unwrap_or_clone(post.message)
        else {
            panic!("changed-roster generation rollover must emit a Hint")
        };
        assert_eq!(hint.current_generation, service_generation(2));
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&successor_route))
        );
        assert_eq!(server.server_service_generation, hint.current_generation);
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
        let successor_before_late_flush = server
            .lifecycle_snapshot()
            .expect("snapshot the newly fenced generation");
        let closures_before_late_flush = server.pending_server_closures.clone();

        assert!(
            !server
                .acknowledge_outbound_chunk(&late_flush, now)
                .expect("a late compacted-generation flush is consumed")
        );
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after consuming the late flush"),
            successor_before_late_flush
        );
        assert_eq!(server.pending_server_closures, closures_before_late_flush);

        successor.service_generation = hint.current_generation;
        successor.bind_canonical_request_id();
        assert!(matches!(
            server
                .admit_server_request(&successor_requester, &successor, None, &responder, now,)
                .expect("retry under the successor generation"),
            ServerRequestAdmission::Materialize
        ));
        let successor_gate = server
            .lifecycle_snapshot()
            .expect("snapshot the successor occurrence");
        assert!(
            !server
                .acknowledge_outbound_chunk(&late_flush, now)
                .expect("duplicate late flush remains a consumed no-op")
        );
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after duplicate late flush"),
            successor_gate
        );
    }

    #[test]
    fn generation_hint_fence_survives_lifecycle_restart() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let requester = peer(b"durable generation Hint requester");
        let reference = reference(1, 1);
        let responder = reference.merge_qc.validator_set[0].clone();
        let block_hash = HashOf::from_untyped_unchecked(Hash::new(b"durable generation block"));
        let now = Instant::now();
        let mut client =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("open durable requester");
        let post = client
            .defer_block(block_hash, 2, 0, reference, &requester, 1, now)
            .expect("persist the first request occurrence")
            .expect("emit the first request occurrence");
        let CertifiedMergeSidecarMessage::Request(request) = Arc::unwrap_or_clone(post.message)
        else {
            panic!("durable requester emits a request")
        };
        let hint = generation_hint_for_request(&request, service_generation(2));
        assert!(
            client
                .acknowledge_generation_hint(&responder, &hint, &requester)
                .expect("persist the newer responder fence")
        );
        let replacement_epoch = client.request_streams[&responder].stream_epoch;
        drop(client);

        let restarted =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("restore the durable responder fence");
        let restored = &restarted.request_streams[&responder];
        assert_eq!(restored.service_generation, hint.current_generation);
        assert_eq!(restored.stream_epoch, replacement_epoch);
        assert_eq!(restored.next_sequence, 0);
        assert!(restored.open_sequences.is_empty());
    }

    #[test]
    fn stream_epoch_overflow_rejects_without_allocating_or_reusing_an_epoch() {
        let mut transport = MergeSidecarTransport::new();
        transport.next_stream_epoch = u64::MAX;
        for index in 0..MAX_CERTIFIED_MERGE_SEMANTIC_PEERS {
            transport.request_streams.insert(
                peer(format!("epoch overflow retained responder {index}").as_bytes()),
                RequestStreamState::new(stream_epoch(
                    u64::try_from(index).expect("bounded index fits u64") + 1,
                )),
            );
        }
        let before = transport
            .lifecycle_snapshot()
            .expect("snapshot the full requester table");
        let responder = peer(b"epoch overflow responder");
        assert!(matches!(
            transport.allocate_request_sequence(&responder),
            Err(MergeSidecarError::Capacity(
                "semantic stream epoch exhausted"
            ))
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot after rejected reclamation"),
            before,
            "checked epoch exhaustion must precede every reclamation mutation"
        );
    }

    #[test]
    fn service_generation_overflow_rejects_without_compacting_server_state() {
        let mut server = MergeSidecarTransport::new();
        server.server_service_generation = service_generation(u64::MAX);
        for index in 0..MAX_CERTIFIED_MERGE_SEMANTIC_PEERS {
            server.server_streams.insert(
                peer(format!("generation overflow requester {index}").as_bytes()),
                ServerStreamState {
                    stream_epoch: stream_epoch(1),
                    closed_through: 1,
                    highest_sequence: 1,
                },
            );
        }
        let before = server
            .lifecycle_snapshot()
            .expect("snapshot the full server generation");
        let changed_roster = vec![peer(b"generation overflow changed roster")];
        assert!(matches!(
            server.transition_server_service_generation(
                changed_roster.len(),
                canonical_merge_sidecar_roster_digest(&changed_roster),
            ),
            Err(MergeSidecarError::Capacity(
                "server service generation exhausted"
            ))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after rejected generation rollover"),
            before
        );
        assert!(server.pending_server_closures.is_empty());
    }

    #[test]
    fn full_server_table_never_advances_generation_without_a_changed_roster() {
        let (_, first_requester, _, first, now) = start_session(1, 1);
        let responder = first.responder.clone();
        let limits = MergeSidecarLimits::defaults();
        let roster_digest =
            canonical_merge_sidecar_roster_digest(std::slice::from_ref(&first_requester));
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            DEFAULT_REPLY_SOURCE_CAPACITY,
            limits,
            1,
            roster_digest.clone(),
        )
        .expect("construct a one-requester responder table");
        assert!(matches!(
            server
                .admit_server_request(&first_requester, &first, None, &responder, now)
                .expect("admit the active predecessor occurrence"),
            ServerRequestAdmission::Materialize
        ));

        let extra_requester = peer(b"active full-table extra requester");
        let extra = routed_server_request(
            &first,
            extra_requester.clone(),
            b"active full-table extra request",
            1,
        );
        let reply_hub = peer(b"full-table extra requester reply hub");
        let mut reply_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            reply_hub.clone(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
        );
        let extra_reply_route = reply_routes.mint_via(extra_requester.clone(), reply_hub);
        let active_snapshot = server
            .lifecycle_snapshot()
            .expect("snapshot the active full table");
        assert!(matches!(
            server.admit_server_request(
                &extra_requester,
                &extra,
                Some(&extra_reply_route),
                &responder,
                now,
            ),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after active-state rejection"),
            active_snapshot,
            "active exhaustion must not clear a gate, advance generation, or alter geometry"
        );
        assert!(server.pending_server_closures.is_empty());

        let mut close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: first.service_generation,
            stream_epoch: first.stream_epoch,
            closed_through: first.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: first_requester.clone(),
            responder: responder.clone(),
        };
        close.bind_canonical_close_id();
        server
            .admit_server_close(&first_requester, &close, None, &responder)
            .expect("terminate the only predecessor stream");
        assert_eq!(server.drain_closed_server_prefixes().len(), 1);
        server.confirm_closed_server_prefix_handoff();
        assert!(server.server_generation_is_terminal());

        let terminal_snapshot = server
            .lifecycle_snapshot()
            .expect("snapshot the full terminal table");
        assert!(matches!(
            server.admit_server_request(
                &extra_requester,
                &extra,
                Some(&extra_reply_route),
                &responder,
                now,
            ),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after terminal-state capacity rejection"),
            terminal_snapshot,
            "terminal exhaustion must not clear streams or advance the responder generation"
        );
        assert!(matches!(
            server.transition_server_service_generation(1, roster_digest),
            Err(MergeSidecarError::Capacity(
                "server service generation requires a changed roster identity"
            ))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after same-roster transition rejection"),
            terminal_snapshot,
            "even an explicit same-roster transition is fail-atomic"
        );

        let changed_roster = vec![extra_requester.clone()];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        let mut server = server
            .rehydrate_with_exact_geometry(
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
                changed_roster.len(),
                changed_digest,
                now,
            )
            .expect("a certified changed roster advances the terminal responder generation");
        assert_eq!(server.server_service_generation, service_generation(2));
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());

        let ServerRequestAdmission::GenerationHint(post) = server
            .admit_server_request(
                &extra_requester,
                &extra,
                Some(&extra_reply_route),
                &responder,
                now,
            )
            .expect("old-generation traffic receives the changed-roster fence")
        else {
            panic!("changed-roster stale traffic must receive a GenerationHint")
        };
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&extra_reply_route)),
            "the changed-roster fence retains the triggering authenticated route"
        );
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = Arc::unwrap_or_clone(post.message)
        else {
            panic!("changed-roster stale traffic emits a GenerationHint")
        };
        assert_eq!(
            hint.observed_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(hint.observed_message_hash, HashOf::new(&extra).into());
        assert_eq!(hint.hint_id, hint.canonical_hint_id());
        assert_eq!(server.server_service_generation, service_generation(2));
        assert!(server.server_streams.is_empty());
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
    }

    #[test]
    fn rejected_request_does_not_consume_server_stream_state() {
        let (_, requester, _, mut request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        request.semantic_sequence = semantic_sequence(
            u64::try_from(MAX_INBOUND_SESSIONS_PER_PEER).expect("bounded test geometry") + 1,
        );
        request.bind_canonical_request_id();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server.admit_server_request(&requester, &request, None, &local_peer, now),
            Err(MergeSidecarError::Capacity(
                "semantic request forward window"
            ))
        ));
        assert!(!server.server_streams.contains_key(&requester));
        assert!(server.server_request_gates.is_empty());
    }

    #[test]
    fn transient_materialization_release_keeps_exact_retry() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("reserve the request before transient response pressure"),
            ServerRequestAdmission::Materialize
        ));
        server.cancel_unmaterialized_server_request(&requester, &request);
        let parked = server
            .server_request_gates
            .values()
            .next()
            .and_then(|gate| gate.attempts.values().next())
            .expect("transient pressure retains one bounded retryable attempt");
        assert!(!parked.materialization_authorized);
        assert!(parked.materialization_retryable);
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("the same occurrence remains admissible after transient pressure"),
            ServerRequestAdmission::Materialize
        ));
    }

    #[test]
    fn inactive_outbound_reclamation_releases_bytes_and_preserves_exact_cursor() {
        let response_len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, mut request, now) = start_session(1, 1);
        request.encoded_len = u64::try_from(response_len).expect("bounded response length");
        request.bind_canonical_request_id();
        let responder = request.responder.clone();
        let hub = peer(b"inactive reclamation hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let source = ServerRequestSource::Authenticated(route.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("admit response before route retirement"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route.clone()), vec![0xA7; response_len], now)
            .expect("materialize the shared response");
        let emitted = server.drain_outbound_chunks(1, now);
        assert_eq!(emitted.len(), 1);
        let gate_attempt = &server.server_request_gates[&key].attempts[&source];
        let cursor_before = gate_attempt.cursor;
        let pending_before = gate_attempt
            .pending_flush_chunk
            .clone()
            .expect("drain publishes the exact pending chunk marker");
        assert_eq!(server.retained_outbound_attempt_count_for_test(), 1);
        assert_eq!(
            server.retained_outbound_bytes_for_test(),
            response_len,
            "shared bytes are charged once"
        );

        assert!(routes.retire(&route));
        assert_eq!(
            server
                .reclaim_inactive_outbound_attempts(now)
                .expect("durably reclaim inactive output"),
            1
        );
        assert_eq!(server.retained_outbound_attempt_count_for_test(), 0);
        assert_eq!(server.retained_outbound_bytes_for_test(), 0);
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        let retained = &server.server_request_gates[&key].attempts[&source];
        assert_eq!(retained.cursor, cursor_before);
        assert_eq!(
            retained.pending_flush_chunk.as_ref(),
            Some(&pending_before),
            "reclamation retains the exact late-receipt/rematerialization witness"
        );
        assert!(retained.materialization_retryable);
        assert!(!retained.materialization_authorized);
    }

    #[test]
    fn reply_unwritable_route_parks_inflight_materialization_without_bytes() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"unwritable materialization hub"));
        let route = routes.mint(requester.clone());
        let source = ServerRequestSource::Authenticated(route.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("authorize the exact Kura materialization"),
            ServerRequestAdmission::Materialize
        ));
        assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
        assert!(route.is_active());
        assert!(!route.is_reply_writable());

        assert!(matches!(
            server.enqueue_response(request.clone(), Some(route), vec![0xA1], now),
            Err(MergeSidecarError::Capacity("outbound response budget"))
        ));
        let parked = &server.server_request_gates[&key].attempts[&source];
        assert!(!parked.materialization_authorized);
        assert!(parked.authorized_materialization_route.is_none());
        assert!(parked.materialization_retryable);
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.retained_outbound_attempt_count_for_test(), 0);
        assert_eq!(server.retained_outbound_bytes_for_test(), 0);
        assert!(
            server
                .next_server_request_materialization(now)
                .expect("prune an unwritable materialization route")
                .is_none(),
            "an active inbound capability cannot authorize an obsolete reply writer"
        );

        let reconnected = routes.mint(requester.clone());
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&reconnected), &responder, now,)
                .expect("a writable replacement retries the parked cursor"),
            ServerRequestAdmission::Materialize
        ));
    }

    #[test]
    fn reply_unwritable_reclamation_applies_a_late_exact_flush_once() {
        let response_len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, mut request, now) = start_session(1, 1);
        request.encoded_len = u64::try_from(response_len).expect("bounded response length");
        request.bind_canonical_request_id();
        let responder = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"unwritable late-flush hub"));
        let route = routes.mint(requester.clone());
        let source = ServerRequestSource::Authenticated(route.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("admit the two-chunk response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route.clone()), vec![0xB2; response_len], now)
            .expect("materialize the two-chunk response");
        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("emit chunk zero");
        assert!(acknowledge_reply_chunk(&mut server, &first, now));
        let old_writer_post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand chunk one to the old exact writer");
        let late_exact_flush = reply_chunk_admission(&old_writer_post);
        let pending_before = server.server_request_gates[&key].attempts[&source]
            .pending_flush_chunk
            .clone()
            .expect("retain the chunk-one flush witness");

        assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
        assert!(route.is_active());
        assert!(!route.is_reply_writable());
        assert_eq!(
            server
                .reclaim_inactive_outbound_attempts(now)
                .expect("reclaim the obsolete reply writer"),
            1
        );
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.retained_outbound_bytes_for_test(), 0);
        let retained = &server.server_request_gates[&key].attempts[&source];
        assert_eq!(retained.cursor, ServerResponseCursor::Pending(1));
        assert_eq!(retained.pending_flush_chunk.as_ref(), Some(&pending_before));

        assert!(
            server
                .acknowledge_outbound_chunk(&late_exact_flush, now)
                .expect("a flush published before writer timeout advances once")
        );
        let after_exact_flush = server
            .lifecycle_snapshot()
            .expect("snapshot the exact late-flush advancement");
        assert_eq!(
            server.server_request_gates[&key].attempts[&source].cursor,
            ServerResponseCursor::Complete
        );
        assert!(
            server.server_request_gates[&key].attempts[&source]
                .pending_flush_chunk
                .is_none()
        );
        assert!(
            !server
                .acknowledge_outbound_chunk(&late_exact_flush, now)
                .expect("the same late flush is consumed only once")
        );
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after the duplicate late flush"),
            after_exact_flush,
            "a delayed or duplicated worker callback cannot falsely advance twice"
        );
    }

    #[test]
    fn reply_unwritable_reclamation_persists_pending_cursor_across_restart() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let hub = peer(b"durable unwritable cursor hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
        let route = routes.mint(requester.clone());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open the durable unwritable-cursor fixture");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("admit the durable response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route.clone()), vec![0xC3], now)
            .expect("materialize the durable response");
        let old_writer_post = server
            .drain_outbound_chunks_durable(1, now)
            .expect("persist the pending writer marker")
            .pop()
            .expect("emit the durable response chunk");
        let late_process_local_flush = reply_chunk_admission(&old_writer_post);
        let pending = ServerPendingChunkIdentity::from_message(&old_writer_post.message)
            .expect("response post retains its pending identity");

        assert!(routes.mark_reply_unwritable_while_delivery_active(&route));
        assert_eq!(
            server
                .reclaim_inactive_outbound_attempts(now)
                .expect("persist reclamation of the obsolete writer"),
            1
        );
        assert!(server.outbound.is_empty());
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restore the pending cursor without response bytes");
        let recovered_source = ServerRequestSource::RecoveredAuthenticated(hub);
        let recovered = &restarted.server_request_gates[&key].attempts[&recovered_source];
        assert_eq!(recovered.cursor, ServerResponseCursor::Pending(0));
        assert_eq!(recovered.pending_flush_chunk.as_ref(), Some(&pending));
        assert!(restarted.outbound.is_empty());
        let before_late_process_local_flush = restarted
            .lifecycle_snapshot()
            .expect("snapshot the recovered cursor");
        assert!(
            !restarted
                .acknowledge_outbound_chunk(&late_process_local_flush, now)
                .expect("a pre-restart process-local source cannot claim recovered state")
        );
        assert_eq!(
            restarted
                .lifecycle_snapshot()
                .expect("snapshot after the stale process-local callback"),
            before_late_process_local_flush
        );
    }

    #[test]
    fn reply_unwritable_routes_do_not_block_roster_transition() {
        let (_, _, _, base, now) = start_session(1, 1);
        let responder = base.responder.clone();
        let output_requester = peer(b"unwritable transition output requester");
        let authorized_requester = peer(b"unwritable transition authorized requester");
        let replacement = peer(b"unwritable transition replacement requester");
        let old_roster = vec![output_requester.clone(), authorized_requester.clone()];
        let new_roster = vec![output_requester.clone(), replacement];
        let old_digest = canonical_merge_sidecar_roster_digest(&old_roster);
        let new_digest = canonical_merge_sidecar_roster_digest(&new_roster);
        let source_capacity = 2;
        let limits = MergeSidecarLimits::defaults();
        let output_request = routed_server_request(
            &base,
            output_requester.clone(),
            b"unwritable transition output",
            1,
        );
        let authorized_request = routed_server_request(
            &base,
            authorized_requester.clone(),
            b"unwritable transition authorization",
            1,
        );
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
            peer(b"unwritable transition hub"),
            source_capacity,
        );
        let output_route = routes.mint(output_requester.clone());
        let authorized_route = routes.mint(authorized_requester.clone());
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            old_roster.len(),
            old_digest,
        )
        .expect("construct the old roster");
        assert!(matches!(
            server
                .admit_server_request(
                    &output_requester,
                    &output_request,
                    Some(&output_route),
                    &responder,
                    now,
                )
                .expect("admit the output-owning request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                output_request.clone(),
                Some(output_route.clone()),
                vec![0xD4],
                now,
            )
            .expect("materialize old-roster output");
        let old_writer_post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand old-roster output to its writer");
        let late_old_flush = reply_chunk_admission(&old_writer_post);
        assert!(matches!(
            server
                .admit_server_request(
                    &authorized_requester,
                    &authorized_request,
                    Some(&authorized_route),
                    &responder,
                    now,
                )
                .expect("retain one independently authorized lookup"),
            ServerRequestAdmission::Materialize
        ));
        assert!(
            !server.server_generation_is_terminal(),
            "the old roster still owns both response bytes and lookup authority"
        );
        assert!(routes.mark_reply_unwritable_while_delivery_active(&output_route));
        assert!(routes.mark_reply_unwritable_while_delivery_active(&authorized_route));
        assert!(output_route.is_active() && !output_route.is_reply_writable());
        assert!(authorized_route.is_active() && !authorized_route.is_reply_writable());
        assert!(
            !server.server_generation_is_terminal(),
            "writability pruning, not the timeout callback, releases retained authority"
        );
        assert!(matches!(
            server.transition_server_service_generation(new_roster.len(), new_digest.clone(),),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        for (requester, request) in [
            (&output_requester, &output_request),
            (&authorized_requester, &authorized_request),
        ] {
            let close = close_for_request(request);
            server
                .admit_server_close(requester, &close, None, &responder)
                .expect("the authenticated close terminally releases unwritable ownership");
        }
        assert_eq!(server.drain_closed_server_prefixes().len(), 2);
        server.confirm_closed_server_prefix_handoff();

        let mut transitioned = server
            .rehydrate_with_exact_geometry(
                source_capacity,
                limits,
                new_roster.len(),
                new_digest,
                now,
            )
            .expect("closed unwritable ownership permits the roster transition");
        assert_eq!(
            transitioned.server_service_generation,
            service_generation(2)
        );
        assert!(transitioned.server_streams.is_empty());
        assert!(transitioned.server_request_gates.is_empty());
        assert!(transitioned.outbound.is_empty());
        assert!(transitioned.outbound_order.is_empty());
        assert_eq!(transitioned.retained_outbound_bytes_for_test(), 0);
        assert_eq!(transitioned.drain_closed_server_prefixes().len(), 2);
        let successor = transitioned
            .lifecycle_snapshot()
            .expect("snapshot the transitioned roster");
        assert!(
            !transitioned
                .acknowledge_outbound_chunk(&late_old_flush, now)
                .expect("a compacted old-roster flush is a consumed no-op")
        );
        assert_eq!(
            transitioned
                .lifecycle_snapshot()
                .expect("snapshot after the compacted late flush"),
            successor
        );
    }

    #[test]
    fn materialization_scheduler_round_robins_requesters_without_starvation() {
        let (_, _, _, base, now) = start_session(1, 1);
        let responder = base.responder.clone();
        let first_requester = peer(b"fair materialization requester a");
        let second_requester = peer(b"fair materialization requester b");
        let hub = peer(b"fair materialization hub");
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub, 1);
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            1,
            MergeSidecarLimits::defaults(),
            2,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("bounded fair materialization geometry");
        for requester in [&first_requester, &second_requester] {
            let request =
                routed_server_request(&base, requester.clone(), b"fair materialization request", 1);
            let route = routes.mint(requester.clone());
            server
                .admit_server_request(requester, &request, Some(&route), &responder, now)
                .expect("queue one requester gate");
            let selected = server
                .next_server_request_materialization(now)
                .expect("select the currently authorized requester")
                .expect("one request is selected");
            server.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
        }

        let mut selected_requesters = Vec::new();
        for _ in 0..6 {
            let selected = server
                .next_server_request_materialization(now)
                .expect("advance the durable requester cursor")
                .expect("both requesters remain retryable");
            selected_requesters.push(selected.requester.clone());
            server.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
        }
        for pair in selected_requesters.windows(2) {
            assert_ne!(
                pair[0], pair[1],
                "a requester with a retryable gate cannot take consecutive turns"
            );
        }
        assert_eq!(
            selected_requesters
                .iter()
                .filter(|requester| **requester == first_requester)
                .count(),
            3
        );
        assert_eq!(
            selected_requesters
                .iter()
                .filter(|requester| **requester == second_requester)
                .count(),
            3
        );
    }

    #[test]
    fn materialization_scheduler_chooses_lowest_occurrence_within_requester() {
        let (_, requester, _, base, now) = start_session(1, 1);
        let responder = base.responder.clone();
        let mut server = MergeSidecarTransport::new();
        let mut higher = base.clone();
        higher.semantic_sequence = semantic_sequence(2);
        higher.bind_canonical_request_id();
        assert!(matches!(
            server
                .admit_server_request(&requester, &higher, None, &responder, now)
                .expect("queue the higher occurrence first"),
            ServerRequestAdmission::Materialize
        ));
        server.cancel_unmaterialized_server_request(&requester, &higher);

        let lower = base;
        assert!(matches!(
            server
                .admit_server_request(&requester, &lower, None, &responder, now)
                .expect("queue the lower occurrence second"),
            ServerRequestAdmission::Materialize
        ));
        let selected = server
            .next_server_request_materialization(now)
            .expect("read the authorized lowest occurrence")
            .expect("one occurrence is selected");
        assert_eq!(selected.request.semantic_sequence.get(), 1);
        server
            .retire_unmaterialized_server_request(&requester, &selected.request)
            .expect("retire the terminal lower occurrence");
        let successor = server
            .next_server_request_materialization(now)
            .expect("select the remaining occurrence")
            .expect("higher occurrence remains retryable");
        assert_eq!(successor.request.semantic_sequence.get(), 2);
    }

    #[test]
    fn lifecycle_v3_roundtrip_and_restore_enforce_gate_and_attempt_bounds_separately() {
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let source_capacity = 2;
        let hub_a = peer(b"lifecycle split bound hub a");
        let hub_b = peer(b"lifecycle split bound hub b");
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), source_capacity);
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let limits = MergeSidecarLimits::defaults();
        let mut server = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            1,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("bounded lifecycle split geometry");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &responder, now)
                .expect("admit first durable source"),
            ServerRequestAdmission::Materialize
        ));
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &responder, now)
                .expect("attach second durable source"),
            ServerRequestAdmission::Existing
        ));
        let selected = server
            .next_server_request_materialization(now)
            .expect("read the current selection")
            .expect("first source remains selected");
        server.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
        let selected = server
            .next_server_request_materialization(now)
            .expect("durably select the retryable multi-source gate")
            .expect("multi-source gate remains selectable");
        server.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
        let snapshot = server
            .lifecycle_snapshot()
            .expect("capture V3 split-capacity snapshot");
        assert_eq!(snapshot.payload.server_request_gates.len(), 1);
        assert_eq!(snapshot.payload.server_request_gates[0].attempts.len(), 2);
        assert_eq!(
            snapshot.payload.materialization_requester_cursor,
            Some(requester.clone())
        );

        let mut restored = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            1,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("matching restore geometry");
        restored
            .restore_lifecycle_snapshot(snapshot.clone(), now)
            .expect("roundtrip valid V3 snapshot");
        assert_eq!(
            restored
                .lifecycle_snapshot()
                .expect("capture restored V3 snapshot"),
            snapshot
        );

        let mut gate_limited = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            1,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("gate-bound restore target");
        gate_limited.server_request_gate_capacity = 0;
        let mut over_gate = snapshot.payload.clone();
        over_gate.geometry.server_request_gate_capacity = 0;
        assert!(matches!(
            gate_limited
                .restore_lifecycle_snapshot(MergeSidecarLifecycleSnapshotV3::new(over_gate), now),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unsupported lifecycle journal version or geometry drift")
        ));

        let mut attempt_limited = MergeSidecarTransport::with_limits_and_server_stream_capacity(
            source_capacity,
            limits,
            1,
            unbound_test_merge_sidecar_roster_digest(),
        )
        .expect("attempt-bound restore target");
        attempt_limited.server_request_attempt_capacity = 1;
        let mut over_attempt = snapshot.payload;
        over_attempt.geometry.server_request_attempt_capacity = 1;
        assert!(matches!(
            attempt_limited.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(over_attempt),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unsupported lifecycle journal version or geometry drift")
        ));
    }

    #[test]
    fn legacy_lifecycle_v1_snapshot_is_rejected_without_migration() {
        let temp = tempfile::tempdir().expect("temporary lifecycle root");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let limits = MergeSidecarLimits::defaults();
        let mut server =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("open current lifecycle journal");
        server
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("admit one current request");
        server.cancel_unmaterialized_server_request(&requester, &request);
        let current = server
            .lifecycle_snapshot()
            .expect("capture current lifecycle payload")
            .payload;
        let legacy = UnsupportedMergeSidecarLifecycleSnapshotV1::new(
            UnsupportedMergeSidecarLifecyclePayloadV1 {
                version: 1,
                geometry: current.geometry.runtime,
                next_stream_epoch: current.next_stream_epoch,
                server_service_generation: current.server_service_generation,
                request_streams: current.request_streams,
                server_streams: current.server_streams,
                server_request_gates: current.server_request_gates,
            },
        );
        let legacy_bytes = norito::to_bytes(&legacy).expect("encode legacy V1 fixture");
        let journal = server
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns lifecycle journal");
        let state_path = journal.state_path();
        let journal_directory = journal.directory.clone();
        {
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&state_path)
                .expect("open current lifecycle state for retired-format mutation");
            file.write_all(&legacy_bytes)
                .expect("write legacy lifecycle bytes");
            file.sync_all().expect("sync legacy lifecycle bytes");
        }
        MergeSidecarLifecycleJournal::sync_directory(&journal_directory)
            .expect("sync retired lifecycle mutation");
        drop(server);

        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("migration is not supported")
        ));
    }

    #[test]
    fn legacy_lifecycle_v2_snapshot_is_rejected_without_layout_guessing() {
        let temp = tempfile::tempdir().expect("temporary lifecycle root");
        let limits = MergeSidecarLimits::defaults();
        let server =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("open current lifecycle journal");
        let current = server
            .lifecycle_snapshot()
            .expect("capture current lifecycle payload")
            .payload;
        let legacy = UnsupportedMergeSidecarLifecycleSnapshotV2::new(
            UnsupportedMergeSidecarLifecyclePayloadV2 {
                version: 2,
                geometry: current.geometry,
                next_stream_epoch: current.next_stream_epoch,
                server_service_generation: current.server_service_generation,
                materialization_requester_cursor: current.materialization_requester_cursor,
                request_streams: current.request_streams,
                server_streams: current.server_streams,
                server_request_gates: current.server_request_gates,
            },
        );
        let legacy_bytes = norito::to_bytes(&legacy).expect("encode legacy V2 fixture");
        let journal = server
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns lifecycle journal");
        fs::write(journal.state_path(), legacy_bytes).expect("install legacy V2 lifecycle bytes");
        MergeSidecarLifecycleJournal::sync_directory(&journal.directory)
            .expect("sync retired V2 lifecycle mutation");
        drop(server);

        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("migration is not supported")
        ));
    }

    #[test]
    fn transient_response_capacity_defers_materialization_on_the_same_delivery() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let hub = peer(b"capacity retry hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
        let mut server = MergeSidecarTransport::new();

        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let requester = peer(format!("capacity retry filler {index}").as_bytes());
            let request = routed_server_request(
                &base,
                requester.clone(),
                format!("capacity retry filler request {index}").as_bytes(),
                1,
            );
            let route = routes.mint_via(requester.clone(), hub.clone());
            assert!(matches!(
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("admit one bounded filler response"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(request, Some(route), vec![0x31], now)
                .expect("fill the authenticated source response corridor");
        }

        let requester = peer(b"capacity retry requester");
        let request = routed_server_request(&base, requester.clone(), b"capacity retry request", 1);
        let route = routes.mint_via(requester.clone(), hub);
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                .expect("retain retryable work while the exact source budget is full"),
            ServerRequestAdmission::Existing
        ));
        let source = ServerRequestSource::Authenticated(route.source_key());
        let key = (requester.clone(), request.request_id);
        assert!(
            server.server_request_gates[&key].attempts[&source].materialization_retryable,
            "transient capacity pressure must not require a reconnect"
        );

        let released = server.drain_outbound_chunks(1, now);
        assert_eq!(released.len(), 1);
        assert!(acknowledge_reply_chunk(&mut server, &released[0], now));
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                .expect("the exact delivery retries after an older response releases capacity"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route), vec![0x42], now)
            .expect("the retried response acquires the released source reservation");
    }

    #[test]
    fn terminal_retirement_releases_multi_source_quota_for_honest_admission() {
        let (_, requester, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let hubs = [
            peer(b"terminal retirement attack hub a"),
            peer(b"terminal retirement attack hub b"),
            peer(b"terminal retirement attack hub c"),
        ];
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(
            hubs[0].clone(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
        );
        let mut limits = MergeSidecarLimits::defaults();
        limits.inbound_sessions_per_peer = MAX_SERVER_REQUEST_GATES_PER_SOURCE + 1;
        let mut server = MergeSidecarTransport::with_limits(DEFAULT_REPLY_SOURCE_CAPACITY, limits)
            .expect("gate-quota fixture has a distinct semantic forward window");
        let mut attack_requests = Vec::new();

        for sequence in 1..=MAX_SERVER_REQUEST_GATES_PER_SOURCE {
            let mut request = base.clone();
            request.semantic_sequence =
                semantic_sequence(u64::try_from(sequence).expect("bounded gate count fits u64"));
            request.closed_through = 0;
            request.bind_canonical_request_id();
            for hub in &hubs {
                let route = routes.mint_via(requester.clone(), hub.clone());
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                    .expect("attach every attack gate to every authenticated source");
            }
            attack_requests.push(request);
        }

        for hub in &hubs {
            assert_eq!(
                server.source_gate_count(&ServerRequestSource::RecoveredAuthenticated(hub.clone())),
                MAX_SERVER_REQUEST_GATES_PER_SOURCE,
                "one requester filled this source's entire gate quota"
            );
        }
        let mut honest = base.clone();
        honest.semantic_sequence = semantic_sequence(
            u64::try_from(MAX_SERVER_REQUEST_GATES_PER_SOURCE + 1).expect("bounded sequence"),
        );
        honest.closed_through = 0;
        honest.bind_canonical_request_id();
        let honest_route = routes.mint_via(requester.clone(), hubs[0].clone());
        assert!(matches!(
            server
                .admit_server_request(&requester, &honest, Some(&honest_route), &local_peer, now,),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));

        for remaining in (0..MAX_SERVER_REQUEST_GATES_PER_SOURCE).rev() {
            let request = attack_requests
                .pop()
                .expect("retire the highest retained semantic occurrence");
            server
                .retire_unmaterialized_server_request(&requester, &request)
                .expect("terminal retirement durably releases every attached source");
            assert_eq!(
                server.server_streams[&requester].highest_sequence,
                u64::try_from(remaining).expect("bounded remaining count fits u64")
            );
            for hub in &hubs {
                assert_eq!(
                    server.source_gate_count(&ServerRequestSource::RecoveredAuthenticated(
                        hub.clone()
                    )),
                    remaining
                );
            }
        }
        assert!(server.server_request_gates.is_empty());
        assert!(matches!(
            server
                .admit_server_request(&requester, &honest, Some(&honest_route), &local_peer, now,)
                .expect("honest traffic acquires the released source quota"),
            ServerRequestAdmission::Materialize
        ));
    }

    #[test]
    fn durable_terminal_retirement_is_not_restored_and_exact_replay_is_fresh() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open responder lifecycle journal");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("admit the request before terminal lookup failure"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist the admitted request");
        server
            .retire_unmaterialized_server_request(&requester, &request)
            .expect("persist terminal gate retirement");
        assert!(!server.server_request_gates.contains_key(&key));
        assert_eq!(server.server_streams[&requester].highest_sequence, 0);
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart after terminal gate retirement");
        assert!(!restarted.server_request_gates.contains_key(&key));
        assert_eq!(restarted.server_streams[&requester].highest_sequence, 0);
        assert!(matches!(
            restarted
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("an exact replay acquires a fresh materialization gate"),
            ServerRequestAdmission::Materialize
        ));
        restarted
            .enqueue_response(request, None, vec![0x42], now)
            .expect("the fresh exact replay can materialize its response");
    }

    #[test]
    fn failed_terminal_retirement_persist_leaves_memory_unchanged() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open responder lifecycle journal");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("admit the request before terminal lookup failure"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist the admitted request");
        let before = server
            .lifecycle_snapshot()
            .expect("snapshot memory before failed retirement persistence");
        server.obstruct_lifecycle_journal_temp_for_test();

        assert!(matches!(
            server.retire_unmaterialized_server_request(&requester, &request),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unsafe lifecycle journal temp artifact")
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot memory after failed retirement persistence"),
            before
        );
        assert!(
            server.server_request_gates[&key]
                .attempts
                .values()
                .all(|attempt| attempt.materialization_authorized),
            "failed persistence must leave the exact live admission untouched"
        );
    }

    #[test]
    fn response_materialization_requires_and_consumes_its_exact_admission_gate() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"materialization gate hub"));
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server.enqueue_response(request.clone(), Some(route.clone()), vec![0x11], now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("authorize exact response materialization"),
            ServerRequestAdmission::Materialize
        ));
        let mut changed = request.clone();
        changed.reference_digest = Hash::new(b"changed after admission");
        assert!(matches!(
            server.enqueue_response(changed, Some(route.clone()), vec![0x11], now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));

        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0x11], now)
            .expect("consume exact response authorization");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("hand exact response to worker output");
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
        assert!(server.outbound.is_empty());
        assert!(matches!(
            server.enqueue_response(request.clone(), Some(route), vec![0x11], now),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        server.cancel_unmaterialized_server_request(&requester, &request);
        assert_eq!(
            server.server_request_gates.len(),
            1,
            "a consumed gate remains bounded semantic-delivery history"
        );
    }

    #[test]
    fn inactive_reply_route_is_rejected_before_server_gate_admission() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let route = routes.mint(requester.clone());
        assert!(routes.retire(&route));
        let mut server = MergeSidecarTransport::new();

        assert!(matches!(
            server.admit_server_request(&requester, &request, Some(&route), &local_peer, now,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert!(server.server_request_gates.is_empty());
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn route_retirement_between_admission_and_enqueue_releases_all_response_reservations() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"retired materialization hub a");
        let hub_b = peer(b"retired materialization hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        let mut server = MergeSidecarTransport::new();

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit source A before materialization"),
            ServerRequestAdmission::Materialize
        ));
        assert!(routes.retire(&route_a));
        assert!(matches!(
            server.enqueue_response(request.clone(), Some(route_a), vec![0x91], now,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        let parked_attempt = server
            .server_request_gates
            .values()
            .next()
            .and_then(|gate| gate.attempts.get(&source_a))
            .expect("retired source keeps bounded route and cursor history");
        assert!(!parked_attempt.materialization_authorized);
        assert!(parked_attempt.authorized_materialization_route.is_none());
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.outbound_attempt_count(), 0);
        assert_eq!(server.global_outbound_bytes(), 0);
        assert_eq!(server.source_outbound_count(&source_a), 0);
        assert_eq!(server.source_outbound_bytes(&source_a), 0);

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("independent authenticated source remains admissible"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route_b.clone()), vec![0x92], now)
            .expect("independent source materializes exact response bytes");
        assert_eq!(server.source_outbound_count(&source_b), 1);
        assert_eq!(server.source_outbound_bytes(&source_b), 1);
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("independent source progresses after source A teardown");
        assert!(matches!(
            &post,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&route_b)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.bytes.as_slice() == [0x92])
        ));
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.outbound_attempt_count(), 0);
        assert_eq!(server.global_outbound_bytes(), 0);
        assert_eq!(server.source_outbound_count(&source_b), 0);
        assert_eq!(server.source_outbound_bytes(&source_b), 0);
    }

    #[test]
    fn completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        let hub_a = peer(b"completed terminal source hub a");
        let hub_b = peer(b"completed terminal source hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let prior_route = routes.mint_via(requester.clone(), hub_a.clone());
        let sibling_route = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(prior_route.source_key());
        let source_b = ServerRequestSource::Authenticated(sibling_route.source_key());
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&prior_route), &local_peer, now,)
                .expect("admit first exact request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(prior_route.clone()), vec![0x11], now)
            .expect("queue singleton response");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&sibling_route), &local_peer, now,)
                .expect("attach an independent sibling to the materialized response"),
            ServerRequestAdmission::Existing
        ));
        let first = server.drain_outbound_chunks(1, now);
        assert!(matches!(
            first.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(route),
                ..
            }] if route.same_delivery(&prior_route)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &first[0], now));
        let key = (requester.clone(), request.request_id);
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));
        assert!(server.outbound[&key].attempts.contains_key(&source_b));
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&prior_route), &local_peer, now,)
                .expect("an exact completed-source duplicate remains terminal"),
            ServerRequestAdmission::Existing
        ));
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));

        let later_route = routes
            .redeliver(&prior_route)
            .expect("mint later delivery for the same source");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now,)
                .expect("later delivery preserves the terminal cursor"),
            ServerRequestAdmission::Existing
        ));
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
        assert!(routes.retire(&later_route));
        let reconnected_route = routes.mint_via(requester.clone(), hub_a.clone());
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&reconnected_route),
                    &local_peer,
                    now,
                )
                .expect("reconnect preserves the completed source cursor"),
            ServerRequestAdmission::Existing
        ));
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );

        let sibling_only = server.drain_outbound_chunks(usize::MAX, now);
        assert!(matches!(
            sibling_only.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(sibling),
                message,
                ..
            }] if sibling.same_delivery(&sibling_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &sibling_only[0], now));
        assert!(server.outbound.is_empty());

        assert!(routes.retire(&reconnected_route));
        let rematerialized_route = routes.mint_via(requester.clone(), hub_a);
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&rematerialized_route),
                    &local_peer,
                    now,
                )
                .expect("completed reconnect without shared bytes remains terminal"),
            ServerRequestAdmission::Existing
        ));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
        assert!(server.drain_outbound_chunks(usize::MAX, now).is_empty());
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn exact_delivery_retry_stays_terminal_beyond_retired_ttl_horizon() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit first exact request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route.clone()), vec![0x11], now)
            .expect("queue first response");
        let first = server.drain_outbound_chunks(usize::MAX, now);
        assert_eq!(first.len(), 1);
        let first_admission = reply_chunk_admission(&first[0]);
        let stale_first_admission = first_admission.clone();
        assert!(
            server
                .acknowledge_outbound_chunk(&first_admission, now)
                .expect("the first exact writer receipt advances")
        );

        let retry_at = now + Duration::from_secs(301);
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, retry_at,)
                .expect("elapsed time cannot reopen a completed semantic request"),
            ServerRequestAdmission::Existing
        ));
        assert!(
            server
                .drain_outbound_chunks(usize::MAX, retry_at)
                .is_empty()
        );
        assert!(
            !server
                .acknowledge_outbound_chunk(&stale_first_admission, retry_at)
                .expect("a consumed old receipt is a harmless no-op"),
            "a cloned receipt cannot reopen or advance the terminal source"
        );
    }

    #[test]
    fn completed_source_does_not_block_a_new_alternate_source() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"completed source hub a");
        let hub_b = peer(b"completed source hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit request through source A"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a), vec![0x11], now)
            .expect("queue first response");
        let first = server.drain_outbound_chunks(usize::MAX, now);
        assert_eq!(first.len(), 1);
        assert!(acknowledge_reply_chunk(&mut server, &first[0], now));

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("new alternate source authorizes rematerialization"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request, Some(route_b.clone()), vec![0x11], now)
            .expect("materialize response for the alternate source");
        let alternate = server.drain_outbound_chunks(usize::MAX, now);
        assert!(matches!(
            alternate.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(route),
                ..
            }] if route.same_delivery(&route_b)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &alternate[0], now));
    }

    #[test]
    fn configured_route_source_capacity_bounds_semantic_attempts() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"configured source capacity hub a");
        let hub_b = peer(b"configured source capacity hub b");
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 1);
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let mut server = MergeSidecarTransport::with_reply_source_capacity(1)
            .expect("one-source response geometry");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("reserve the configured single source"),
            ServerRequestAdmission::Materialize
        ));
        assert!(matches!(
            server.admit_server_request(&requester, &request, Some(&route_b), &local_peer, now),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));
    }

    #[test]
    fn authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes() {
        assert_eq!(MAX_SERVER_REQUEST_GATES_PER_SOURCE, 4);
        assert_eq!(MAX_OUTBOUND_SESSIONS_PER_SOURCE, 2);
        assert_eq!(MAX_OUTBOUND_BYTES_PER_SOURCE, 16 * 1024 * 1024);
    }

    #[test]
    fn configured_source_geometry_reserves_more_than_eight_independent_attempts() {
        assert!(matches!(
            MergeSidecarTransport::with_reply_source_capacity(usize::MAX),
            Err(MergeSidecarError::Capacity(
                "outbound response session geometry"
            ))
        ));

        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let source_capacity = 9;
        let first_hub = peer(b"configured geometry hub 0");
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(first_hub.clone(), source_capacity);
        let first_route = routes.mint_via(requester.clone(), first_hub);
        let mut server = MergeSidecarTransport::with_reply_source_capacity(source_capacity)
            .expect("nine-source response geometry");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now,)
                .expect("admit first configured source"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(first_route), vec![0xA9], now)
            .expect("materialize shared response bytes");
        for index in 1..source_capacity {
            let hub = peer(format!("configured geometry hub {index}").as_bytes());
            let route = routes.mint_via(requester.clone(), hub);
            assert!(matches!(
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("attach configured alternate source"),
                ServerRequestAdmission::Existing
            ));
        }
        assert_eq!(server.outbound_attempt_count(), source_capacity);
        let posts = server.drain_outbound_chunks(source_capacity, now);
        assert_eq!(posts.len(), source_capacity);
        for post in &posts {
            assert!(acknowledge_reply_chunk(&mut server, post, now));
        }
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
    }

    #[test]
    fn fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let hub_a = peer(b"gate cap hub a");
        let hub_b = peer(b"gate cap hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let mut server = MergeSidecarTransport::new();

        for index in 0..MAX_SERVER_REQUEST_GATES_PER_SOURCE {
            let requester = peer(format!("gate cap origin {index}").as_bytes());
            let request = routed_server_request(
                &base,
                requester.clone(),
                format!("gate cap request {index}").as_bytes(),
                1,
            );
            let route = routes.mint_via(requester.clone(), hub_a.clone());
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("reserve one bounded authenticated-hub gate");
        }

        let additional_requester = peer(b"gate cap additional origin");
        let additional = routed_server_request(
            &base,
            additional_requester.clone(),
            b"gate cap additional request",
            1,
        );
        let additional_route = routes.mint_via(additional_requester.clone(), hub_a);
        assert!(matches!(
            server.admit_server_request(
                &additional_requester,
                &additional,
                Some(&additional_route),
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            "network admission never rolls the responder generation"
        );

        let independent_requester = peer(b"gate cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"gate cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        server
            .admit_server_request(
                &independent_requester,
                &independent,
                Some(&independent_route),
                &local_peer,
                now,
            )
            .expect("independent authenticated hub retains its own reservation");
        assert_eq!(
            server.source_gate_count(&ServerRequestSource::Authenticated(
                additional_route.source_key()
            )),
            MAX_SERVER_REQUEST_GATES_PER_SOURCE
        );
        assert_eq!(
            server.source_gate_count(&ServerRequestSource::Authenticated(
                independent_route.source_key()
            )),
            1
        );
        assert_eq!(server.server_request_gates.len(), 5);
        assert_eq!(server.server_gate_attempt_count(), 5);
    }

    #[test]
    fn quiescent_multi_source_pressure_never_rolls_or_bypasses_source_caps() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (_, attacker, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let mut limits = MergeSidecarLimits::defaults();
        limits.server_request_gates_per_source = 2;
        let source_capacity = 2;
        let hubs = [
            peer(b"quiescent global roll hub a"),
            peer(b"quiescent global roll hub b"),
        ];
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(hubs[0].clone(), source_capacity);
        let mut server = MergeSidecarTransport::open_durable(temp.path(), source_capacity, limits)
            .expect("open the small durable responder geometry");

        for sequence in 1..=limits.server_request_gates_per_source {
            let mut request = base.clone();
            request.semantic_sequence =
                semantic_sequence(u64::try_from(sequence).expect("small gate sequence fits u64"));
            request.closed_through = 0;
            request.bind_canonical_request_id();
            for hub in &hubs {
                let route = routes.mint_via(attacker.clone(), hub.clone());
                server
                    .admit_server_request(&attacker, &request, Some(&route), &local_peer, now)
                    .expect("fill every source with the same attacker's gate");
            }
            let selected = server
                .next_server_request_materialization(now)
                .expect("read the bounded scheduler selection")
                .expect("one attack gate is selected");
            server.cancel_unmaterialized_server_request(&selected.requester, &selected.request);
            server
                .persist_lifecycle_state()
                .expect("persist the quiescent retryable attack gate");
        }
        assert_eq!(
            server.server_gate_attempt_count(),
            limits.server_request_gates_per_source * source_capacity
        );
        assert!(
            !server.server_generation_is_terminal(),
            "retryable request gates keep the generation non-terminal"
        );
        for hub in &hubs {
            assert_eq!(
                server.source_gate_count(&ServerRequestSource::RecoveredAuthenticated(hub.clone())),
                limits.server_request_gates_per_source
            );
        }

        let honest = peer(b"quiescent global roll honest requester");
        let honest_request =
            routed_server_request(&base, honest.clone(), b"honest generation retry", 1);
        let honest_route = routes.mint_via(honest.clone(), hubs[0].clone());
        assert!(matches!(
            server.admit_server_request(
                &honest,
                &honest_request,
                Some(&honest_route),
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));
        assert_eq!(
            server.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(
            server.server_request_gates.len(),
            limits.server_request_gates_per_source
        );
        assert_eq!(
            server.server_gate_attempt_count(),
            limits.server_request_gates_per_source * source_capacity
        );
        drop(server);

        let restarted = MergeSidecarTransport::open_durable(temp.path(), source_capacity, limits)
            .expect("restart without changing the responder generation");
        assert_eq!(
            restarted.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(
            restarted.server_request_gates.len(),
            limits.server_request_gates_per_source
        );
    }

    #[test]
    fn saturated_materializer_does_not_erase_same_request_alternate_session() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let requester = peer(b"shared session materialization origin");
        let request = routed_server_request(
            &base,
            requester.clone(),
            b"shared session materialization request",
            1,
        );
        let hub_a = peer(b"shared session materialization hub a");
        let hub_b = peer(b"shared session materialization hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a.clone());
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        let mut server = MergeSidecarTransport::new();

        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let filler_requester = peer(format!("session saturation origin {index}").as_bytes());
            let filler = routed_server_request(
                &base,
                filler_requester.clone(),
                format!("session saturation request {index}").as_bytes(),
                1,
            );
            let filler_route = routes.mint_via(filler_requester.clone(), hub_a.clone());
            assert!(matches!(
                server
                    .admit_server_request(
                        &filler_requester,
                        &filler,
                        Some(&filler_route),
                        &local_peer,
                        now,
                    )
                    .expect("reserve source A's bounded response session"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(filler, Some(filler_route), vec![0x81], now)
                .expect("fill source A's bounded response session");
        }

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A retains retryable work behind its full session budget"),
            ServerRequestAdmission::Existing
        ));
        server
            .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
            .expect("source B makes the shared semantic request schedulable");
        let materialization = server
            .next_server_request_materialization(now)
            .expect("read the fair shared-response selection")
            .expect("source B supplies response capacity");
        assert_eq!(materialization.request, request);
        server
            .enqueue_response(
                materialization.request,
                materialization.reply_route,
                vec![0x82],
                now,
            )
            .expect("source B remains eligible for the shared materialized bytes");

        let key = (requester.clone(), request.request_id);
        let transfer = &server.outbound[&key];
        assert!(matches!(
            transfer.chunks.as_slice(),
            [message]
                if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.bytes.as_slice() == [0x82])
        ));
        assert_eq!(transfer.attempts.len(), 1);
        assert!(!transfer.attempts.contains_key(&source_a));
        assert!(transfer.attempts.contains_key(&source_b));
        let gate = &server.server_request_gates[&key];
        assert!(gate.attempts.contains_key(&source_a));
        assert!(gate.attempts.contains_key(&source_b));
        assert!(!gate.attempts[&source_a].materialization_authorized);
        assert_eq!(
            server.source_outbound_count(&source_a),
            MAX_OUTBOUND_SESSIONS_PER_SOURCE
        );
        assert_eq!(server.source_outbound_count(&source_b), 1);
        let posts = server.drain_outbound_chunks(3, now);
        let shared_post = posts
            .iter()
            .find(|post| {
                matches!(
                    post,
                    MergeSidecarPost {
                        reply_route: Some(route),
                        message,
                        ..
                    } if route.same_delivery(&route_b)
                        && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                            if chunk.request_id == request.request_id)
                )
            })
            .expect("source B receives the shared response");
        let filler_post = posts
            .iter()
            .find(|post| {
                matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.request_id != request.request_id
                )
            })
            .expect("one source A filler is available to release");
        assert!(acknowledge_reply_chunk(&mut server, shared_post, now));
        assert!(
            !server.outbound.contains_key(&key),
            "the shared transfer retires after its only admitted source completes"
        );
        assert!(acknowledge_reply_chunk(&mut server, filler_post, now));
        assert_eq!(
            server.source_outbound_count(&source_a),
            MAX_OUTBOUND_SESSIONS_PER_SOURCE - 1
        );

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect(
                    "the unchanged capacity-partitioned delivery rematerializes after shared bytes retire"
                ),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0x82], now)
            .expect("the original source acquires the released reservation");
        assert!(server.drain_outbound_chunks(1, now).iter().any(|post| {
            matches!(
                post,
                MergeSidecarPost {
                    reply_route: Some(route),
                    message,
                    ..
                } if route.same_delivery(&route_a)
                    && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.request_id == request.request_id)
            )
        }));
    }

    #[test]
    fn saturated_materializer_does_not_erase_same_request_alternate_bytes() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let requester = peer(b"shared byte materialization origin");
        let request = routed_server_request(
            &base,
            requester.clone(),
            b"shared byte materialization request",
            1,
        );
        let hub_a = peer(b"shared byte materialization hub a");
        let hub_b = peer(b"shared byte materialization hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a.clone());
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        let mut server = MergeSidecarTransport::new();

        let filler_requester = peer(b"byte saturation origin");
        let filler = routed_server_request(
            &base,
            filler_requester.clone(),
            b"byte saturation request",
            MAX_OUTBOUND_BYTES_PER_SOURCE,
        );
        let filler_request_id = filler.request_id;
        let filler_key = (filler_requester.clone(), filler_request_id);
        let filler_route = routes.mint_via(filler_requester.clone(), hub_a);
        assert!(matches!(
            server
                .admit_server_request(
                    &filler_requester,
                    &filler,
                    Some(&filler_route),
                    &local_peer,
                    now,
                )
                .expect("reserve source A's exact byte corridor"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                filler,
                Some(filler_route),
                vec![0x91; MAX_OUTBOUND_BYTES_PER_SOURCE],
                now,
            )
            .expect("fill source A's exact byte corridor");

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A retains retryable work behind its full byte budget"),
            ServerRequestAdmission::Existing
        ));
        server
            .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
            .expect("source B makes the shared semantic request schedulable");
        let materialization = server
            .next_server_request_materialization(now)
            .expect("read the fair shared-response selection")
            .expect("source B supplies response byte capacity");
        assert_eq!(materialization.request, request);
        server
            .enqueue_response(
                materialization.request,
                materialization.reply_route,
                vec![0x92],
                now,
            )
            .expect("source B retains the shared materialized bytes");

        let key = (requester.clone(), request.request_id);
        let transfer = &server.outbound[&key];
        assert!(matches!(
            transfer.chunks.as_slice(),
            [message]
                if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.bytes.as_slice() == [0x92])
        ));
        assert_eq!(transfer.attempts.len(), 1);
        assert!(!transfer.attempts.contains_key(&source_a));
        assert!(transfer.attempts.contains_key(&source_b));
        let gate = &server.server_request_gates[&key];
        assert!(gate.attempts.contains_key(&source_a));
        assert!(gate.attempts.contains_key(&source_b));
        assert!(!gate.attempts[&source_a].materialization_authorized);
        assert_eq!(
            server.source_outbound_bytes(&source_a),
            MAX_OUTBOUND_BYTES_PER_SOURCE
        );
        assert_eq!(server.source_outbound_bytes(&source_b), 1);
        assert_eq!(
            server.global_outbound_bytes(),
            MAX_OUTBOUND_BYTES_PER_SOURCE + 1
        );
        let posts = server.drain_outbound_chunks(2, now);
        let shared_post = posts
            .iter()
            .find(|post| {
                matches!(
                    post,
                    MergeSidecarPost {
                        reply_route: Some(route),
                        message,
                        ..
                    } if route.same_delivery(&route_b)
                        && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                            if chunk.request_id == request.request_id)
                )
            })
            .expect("source B receives the shared response");
        let filler_post = posts
            .iter()
            .find(|post| {
                matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.request_id == filler_request_id && chunk.chunk_index == 0
                )
            })
            .expect("source A's byte-filling response starts at chunk zero");
        assert!(acknowledge_reply_chunk(&mut server, shared_post, now));
        assert!(
            !server.outbound.contains_key(&key),
            "the shared transfer retires after its only admitted source completes"
        );
        assert!(acknowledge_reply_chunk(&mut server, filler_post, now));
        let filler_chunk_count =
            MAX_OUTBOUND_BYTES_PER_SOURCE.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        for expected_index in 1..filler_chunk_count {
            let continued = server.drain_outbound_chunks(1, now);
            assert!(matches!(
                continued.as_slice(),
                [MergeSidecarPost { message, .. }]
                    if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.request_id == filler_request_id
                            && usize::try_from(chunk.chunk_index).ok() == Some(expected_index))
            ));
            assert!(acknowledge_reply_chunk(&mut server, &continued[0], now));
        }
        assert!(!server.outbound.contains_key(&filler_key));
        assert_eq!(server.source_outbound_bytes(&source_a), 0);

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect(
                    "the unchanged byte-partitioned delivery rematerializes after shared bytes retire"
                ),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0x92], now)
            .expect("the original source acquires the released byte reservation");
        assert!(server.drain_outbound_chunks(1, now).iter().any(|post| {
            matches!(
                post,
                MergeSidecarPost {
                    reply_route: Some(route),
                    message,
                    ..
                } if route.same_delivery(&route_a)
                    && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                        if chunk.request_id == request.request_id)
            )
        }));
    }

    #[test]
    fn reclaimed_source_releases_capacity_and_resumes_at_durable_cursor() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let requester = peer(b"reclaimed capacity resume origin");
        let response_len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let request = routed_server_request(
            &base,
            requester.clone(),
            b"reclaimed capacity resume request",
            response_len,
        );
        let response_bytes = vec![0xA5; response_len];
        let hub = peer(b"reclaimed capacity resume hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub.clone());
        let route_a = routes.mint_via(requester.clone(), hub.clone());
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let key = (requester.clone(), request.request_id);
        let mut server = MergeSidecarTransport::new();

        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts the original response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(route_a.clone()),
                response_bytes.clone(),
                now,
            )
            .expect("queue the original two-chunk response");
        let first = server.drain_outbound_chunks(1, now);
        assert!(matches!(
            first.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            }] if route.same_delivery(&route_a)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &first[0], now));
        assert_eq!(server.outbound[&key].attempts[&source_a].next_chunk, 1);
        let in_flight = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("publish the exact chunk-one writer marker before retirement");
        assert!(matches!(
            in_flight.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1
        ));
        assert!(routes.retire(&route_a));
        assert_eq!(
            server
                .reclaim_inactive_outbound_attempts(now)
                .expect("reclaim the inactive source"),
            1
        );
        assert_eq!(server.source_outbound_count(&source_a), 0);
        assert!(!server.outbound.contains_key(&key));
        assert!(server.outbound_order.is_empty());
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert!(
            server.server_request_gates[&key].attempts[&source_a]
                .pending_flush_chunk
                .is_some()
        );

        let mut first_filler = None;
        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let filler_requester =
                peer(format!("reclaimed capacity resume filler {index}").as_bytes());
            let filler = routed_server_request(
                &base,
                filler_requester.clone(),
                format!("reclaimed capacity resume filler request {index}").as_bytes(),
                1,
            );
            let filler_route = routes.mint_via(filler_requester.clone(), hub.clone());
            if first_filler.is_none() {
                first_filler = Some(filler.request_id);
            }
            assert!(matches!(
                server
                    .admit_server_request(
                        &filler_requester,
                        &filler,
                        Some(&filler_route),
                        &local_peer,
                        now,
                    )
                    .expect("fill source A's independent response sessions"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(filler, Some(filler_route), vec![0xB6], now)
                .expect("queue one source A filler session");
        }
        assert_eq!(
            server.source_outbound_count(&source_a),
            MAX_OUTBOUND_SESSIONS_PER_SOURCE
        );
        let route_a_reconnected = routes.mint_via(requester.clone(), hub);
        assert!(matches!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&route_a_reconnected),
                    &local_peer,
                    now,
                )
                .expect("source A remains queued while its hub budget is full"),
            ServerRequestAdmission::Existing
        ));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert!(!server.server_request_gates[&key].attempts[&source_a].materialization_authorized);

        let released = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("one filler releases the shared hub budget");
        assert!(matches!(
            released.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk)
                if Some(chunk.request_id) == first_filler
        ));
        assert!(acknowledge_reply_chunk(&mut server, &released, now));
        assert_eq!(
            server.source_outbound_count(&source_a),
            MAX_OUTBOUND_SESSIONS_PER_SOURCE - 1
        );

        let materialization = server
            .next_server_request_materialization(now)
            .expect("released capacity makes the original source schedulable")
            .expect("the original source receives terminating lookup authority");
        assert_eq!(materialization.request, request);
        assert!(
            materialization
                .reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&route_a_reconnected))
        );
        server
            .enqueue_response(
                materialization.request,
                materialization.reply_route,
                response_bytes,
                now,
            )
            .expect("rematerialization preserves the pending chunk identity");
        let resumed = server
            .drain_outbound_chunks(usize::MAX, now)
            .into_iter()
            .find(|post| {
                post.reply_route
                    .as_ref()
                    .is_some_and(|route| route.same_delivery(&route_a_reconnected))
            })
            .expect("the reclaimed source resumes through its new tenure");
        assert!(matches!(
            resumed.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 1
        ));
        assert!(acknowledge_reply_chunk(&mut server, &resumed, now));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
    }

    #[test]
    fn third_session_from_one_hub_is_rejected_while_another_hub_progresses() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let hub_a = peer(b"session cap hub a");
        let hub_b = peer(b"session cap hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let mut server = MergeSidecarTransport::new();

        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let requester = peer(format!("session cap origin {index}").as_bytes());
            let request = routed_server_request(
                &base,
                requester.clone(),
                format!("session cap request {index}").as_bytes(),
                1,
            );
            let route = routes.mint_via(requester.clone(), hub_a.clone());
            assert!(matches!(
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("admit bounded hub A session"),
                ServerRequestAdmission::Materialize
            ));
            server
                .enqueue_response(request, Some(route), vec![0x22], now)
                .expect("queue bounded hub A session");
        }

        let rejected_requester = peer(b"session cap rejected origin");
        let rejected = routed_server_request(
            &base,
            rejected_requester.clone(),
            b"session cap rejected request",
            1,
        );
        let rejected_route = routes.mint_via(rejected_requester.clone(), hub_a);
        assert!(matches!(
            server
                .admit_server_request(
                    &rejected_requester,
                    &rejected,
                    Some(&rejected_route),
                    &local_peer,
                    now,
                )
                .expect("the cheap gate remains retryable without a terminating lookup"),
            ServerRequestAdmission::Existing
        ));

        let independent_requester = peer(b"session cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"session cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        assert!(matches!(
            server
                .admit_server_request(
                    &independent_requester,
                    &independent,
                    Some(&independent_route),
                    &local_peer,
                    now,
                )
                .expect("independent hub retains its own session reservation"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                independent,
                Some(independent_route.clone()),
                vec![0x44],
                now,
            )
            .expect("independent hub queues a response");
        assert!(server.drain_outbound_chunks(3, now).iter().any(|post| {
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&independent_route))
        }));
    }

    #[test]
    fn source_byte_overflow_is_rejected_while_another_hub_progresses() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let hub_a = peer(b"byte cap hub a");
        let hub_b = peer(b"byte cap hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let mut server = MergeSidecarTransport::new();

        let full_requester = peer(b"byte cap full origin");
        let full = routed_server_request(
            &base,
            full_requester.clone(),
            b"byte cap full request",
            MAX_OUTBOUND_BYTES_PER_SOURCE,
        );
        let full_route = routes.mint_via(full_requester.clone(), hub_a.clone());
        assert!(matches!(
            server
                .admit_server_request(&full_requester, &full, Some(&full_route), &local_peer, now,)
                .expect("admit the exact per-source byte bound"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                full,
                Some(full_route),
                vec![0x55; MAX_OUTBOUND_BYTES_PER_SOURCE],
                now,
            )
            .expect("reserve the exact per-source byte bound");

        let overflow_requester = peer(b"byte cap overflow origin");
        let overflow = routed_server_request(
            &base,
            overflow_requester.clone(),
            b"byte cap overflow request",
            1,
        );
        let overflow_route = routes.mint_via(overflow_requester.clone(), hub_a);
        assert!(matches!(
            server
                .admit_server_request(
                    &overflow_requester,
                    &overflow,
                    Some(&overflow_route),
                    &local_peer,
                    now,
                )
                .expect("retain bounded work without looking up bytes the source cannot own"),
            ServerRequestAdmission::Existing
        ));

        let independent_requester = peer(b"byte cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"byte cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        assert!(matches!(
            server
                .admit_server_request(
                    &independent_requester,
                    &independent,
                    Some(&independent_route),
                    &local_peer,
                    now,
                )
                .expect("independent hub retains its own byte reservation"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                independent,
                Some(independent_route.clone()),
                vec![0x77],
                now,
            )
            .expect("independent hub queues a response");
        assert!(server.drain_outbound_chunks(2, now).iter().any(|post| {
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&independent_route))
        }));
    }

    #[test]
    fn outbound_chunk_drain_is_fair_across_bounded_sessions() {
        let (_, _, _, first, now) = start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3, 3);
        let (_, _, _, mut second, _) = start_session(1, 3);
        second.semantic_sequence = semantic_sequence(2);
        second.bind_canonical_request_id();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&first.requester, &first, None, &first.responder, now,)
                .expect("admit first fair outbound response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                first.clone(),
                None,
                vec![0x11; MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3],
                now,
            )
            .expect("queue first response");
        assert!(matches!(
            server
                .admit_server_request(&second.requester, &second, None, &second.responder, now,)
                .expect("admit second fair outbound response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(second.clone(), None, vec![0x22], now)
            .expect("queue second response");

        let posts = server.drain_outbound_chunks(2, now);
        assert_eq!(posts.len(), 2);
        let request_ids = posts
            .into_iter()
            .map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Chunk(chunk) => chunk.request_id,
                CertifiedMergeSidecarMessage::Request(_)
                | CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_) => {
                    panic!("response emitted a control message")
                }
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            request_ids,
            BTreeSet::from([first.request_id, second.request_id])
        );
    }

    #[test]
    fn completed_short_session_replacement_cannot_starve_an_older_long_session() {
        let (_, _, _, mut short, now) = start_session(1, 3);
        let (_, _, _, mut long, _) = start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3, 3);
        short.semantic_sequence = semantic_sequence(1);
        long.semantic_sequence = semantic_sequence(2);
        short.bind_canonical_request_id();
        long.bind_canonical_request_id();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"replacement fairness hub"));
        let short_route = routes.mint(short.requester.clone());
        let long_route = routes.mint(long.requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(
                    &short.requester,
                    &short,
                    Some(&short_route),
                    &short.responder,
                    now,
                )
                .expect("admit initial short response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(short.clone(), Some(short_route.clone()), vec![0x11], now)
            .expect("queue first short response");
        assert!(matches!(
            server
                .admit_server_request(
                    &long.requester,
                    &long,
                    Some(&long_route),
                    &long.responder,
                    now,
                )
                .expect("admit initial long response"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                long.clone(),
                Some(long_route),
                vec![0x22; MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3],
                now,
            )
            .expect("queue long response");

        let first = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("short response is the first FIFO owner");
        assert!(matches!(
            first.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.request_id == short.request_id
        ));
        assert!(acknowledge_reply_chunk(&mut server, &first, now));

        let mut replacement = short;
        replacement.semantic_sequence = semantic_sequence(3);
        replacement.closed_through = 1;
        replacement.bind_canonical_request_id();
        let replacement_route = routes.mint(replacement.requester.clone());
        assert!(matches!(
            server
                .admit_server_request(
                    &replacement.requester,
                    &replacement,
                    Some(&replacement_route),
                    &replacement.responder,
                    now,
                )
                .expect("admit adversarial short replacement"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(replacement, Some(replacement_route), vec![0x33], now)
            .expect("queue adversarial short replacement");

        let next = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("older long session must receive the next rank");
        assert!(matches!(
            next.message.as_ref(),
            CertifiedMergeSidecarMessage::Chunk(chunk)
                if chunk.request_id == long.request_id && chunk.chunk_index == 0
        ));
        assert!(acknowledge_reply_chunk(&mut server, &next, now));
    }

    #[test]
    fn tick_bounded_limit_one_alternates_saturated_requests_and_responses() {
        let now = Instant::now();
        let requester = peer(b"fair tick requester");
        let mut transport = MergeSidecarTransport::new();

        // Fill the per-peer inbound reservation so that, after one timeout,
        // every bounded tick has another fetch request ready to emit.
        for index in 0..MAX_INBOUND_SESSIONS_PER_PEER {
            let mut pending = reference(1, 3);
            pending.entry_hash = HashOf::from_untyped_unchecked(Hash::new([0xA5, index as u8]));
            let block_hash = HashOf::from_untyped_unchecked(Hash::new([0x5A, index as u8]));
            assert!(
                transport
                    .defer_block(block_hash, 2, 0, pending, &requester, 1, now)
                    .expect("register bounded inbound fetch")
                    .is_some()
            );
        }

        let (_, _, _, response_request, _) = start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3, 3);
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"fair tick response hub"));
        let response_route = routes.mint(response_request.requester.clone());
        assert!(matches!(
            transport
                .admit_server_request(
                    &response_request.requester,
                    &response_request,
                    Some(&response_route),
                    &response_request.responder,
                    now,
                )
                .expect("admit bounded response"),
            ServerRequestAdmission::Materialize
        ));
        transport
            .enqueue_response(
                response_request,
                Some(response_route),
                vec![0xC3; MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3],
                now,
            )
            .expect("queue a multi-chunk response behind saturated fetches");

        let timed_out_at = now + REQUEST_TIMEOUT;
        let mut kinds = Vec::new();
        for _ in 0..6 {
            let posts = transport
                .tick_bounded(&requester, timed_out_at, 1)
                .expect("service one fair transport item");
            assert_eq!(posts.len(), 1, "bounded tick must use its one slot");
            let is_chunk = matches!(
                posts[0].message.as_ref(),
                CertifiedMergeSidecarMessage::Chunk(_)
            );
            if is_chunk {
                assert!(acknowledge_reply_chunk(
                    &mut transport,
                    &posts[0],
                    timed_out_at
                ));
            }
            kinds.push(is_chunk);
        }
        assert_eq!(
            kinds,
            vec![true, false, true, false, true, false],
            "continuous inbound fetch pressure must neither starve response chunks nor be starved by them"
        );
    }

    #[test]
    fn session_and_deferred_caps_fail_closed_without_unbounded_growth() {
        let now = Instant::now();
        let requester = peer(b"requester");
        let mut transport = MergeSidecarTransport::new();
        for index in 0..MAX_INBOUND_SESSIONS - RESERVED_DECIDED_INBOUND_SESSIONS {
            let mut reference = reference(1, 2);
            reference.entry_hash = HashOf::from_untyped_unchecked(Hash::new(index.to_le_bytes()));
            let block_hash = HashOf::from_untyped_unchecked(Hash::new([index as u8, 1]));
            transport
                .defer_block(block_hash, 2, 0, reference, &requester, 1, now)
                .expect("within session cap");
        }
        let mut overflow = reference(1, 2);
        overflow.entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"overflow"));
        assert_eq!(
            transport.defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"overflow-block")),
                2,
                0,
                overflow,
                &requester,
                1,
                now,
            ),
            Err(MergeSidecarError::Capacity("inbound session count"))
        );
        let mut decided = reference(1, 2);
        decided.entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"decided entry"));
        assert!(
            transport
                .defer_decided_block(
                    HashOf::from_untyped_unchecked(Hash::new(b"decided block")),
                    2,
                    0,
                    decided,
                    &requester,
                    1,
                    now,
                )
                .expect("decided fetch owns reserved capacity")
                .is_some(),
            "ordinary requests must leave one global and per-holder slot for finality"
        );
        assert_eq!(transport.inbound_len(), MAX_INBOUND_SESSIONS);
    }

    #[test]
    fn attacker_first_conflicting_reference_isolated_from_honest_session() {
        let (_, requester, honest, _, now) = start_session(64, 3);
        let attacker_block = HashOf::from_untyped_unchecked(Hash::new(b"attacker block"));
        let honest_block = HashOf::from_untyped_unchecked(Hash::new(b"honest decided block"));
        let mut attacker = honest.clone();
        attacker.encoded_len += 1;
        let mut transport = MergeSidecarTransport::new();
        transport
            .defer_block(attacker_block, 2, 0, attacker, &requester, 1, now)
            .expect("retain attacker-first exact reference independently");
        let honest_post = transport
            .defer_block(honest_block, 2, 0, honest.clone(), &requester, 1, now)
            .expect("conflicting metadata cannot poison honest registration")
            .expect("honest registration emits a request");
        let CertifiedMergeSidecarMessage::Request(honest_request) =
            Arc::unwrap_or_clone(honest_post.message)
        else {
            panic!("honest registration emitted a response chunk")
        };
        assert_eq!(transport.inbound_len(), 2);

        let responder = honest_request.responder.clone();
        assert!(matches!(
            transport
                .ingest_chunk(
                    &responder,
                    chunks(&honest_request, &[0_u8; 64]).remove(0),
                    now,
                )
                .expect("complete honest exact-reference response"),
            ChunkIngestOutcome::Complete(_)
        ));
        let (deferred, _) = transport
            .finish_completed(
                honest.entry_hash,
                certified_merge_reference_digest(&honest),
                true,
                &requester,
                now,
            )
            .expect("persist honest request lifecycle");
        assert_eq!(deferred, vec![(honest_block, 2, 0)]);
        assert_eq!(
            transport.inbound_len(),
            1,
            "honest completion must not mutate the isolated attacker session"
        );
    }

    #[test]
    fn same_hash_variant_saturation_cannot_crowd_out_decided_fetch() {
        let now = Instant::now();
        let requester = peer(b"requester");
        let mut transport = MergeSidecarTransport::new();
        let honest = reference(64, 3);
        for index in 0..MAX_INBOUND_SESSIONS - RESERVED_DECIDED_INBOUND_SESSIONS {
            let mut attacker = honest.clone();
            attacker.encoded_len = u64::try_from(index + 1).expect("fixture length fits u64");
            transport
                .defer_block(
                    HashOf::from_untyped_unchecked(Hash::new([index as u8, 0xA1])),
                    2,
                    0,
                    attacker,
                    &requester,
                    1,
                    now,
                )
                .expect("ordinary attacker variant remains within its bounded partition");
        }
        assert_eq!(
            transport.defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"ordinary honest block")),
                2,
                0,
                honest.clone(),
                &requester,
                1,
                now,
            ),
            Err(MergeSidecarError::Capacity("inbound session count"))
        );
        assert!(
            transport
                .defer_decided_block(
                    HashOf::from_untyped_unchecked(Hash::new(b"decided honest block")),
                    2,
                    0,
                    honest,
                    &requester,
                    1,
                    now,
                )
                .expect("decided same-hash reference bypasses ordinary variant saturation")
                .is_some()
        );
        assert_eq!(transport.inbound_len(), MAX_INBOUND_SESSIONS);
    }

    #[test]
    fn unsent_request_restores_holder_and_backoff_state() {
        let (mut transport, requester, reference, first, now) = start_session(1, 3);
        transport
            .release_unsent_request(&first)
            .expect("persist unsent request lifecycle");
        let key = (
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
        );
        let assembly = transport.inbound.get(&key).expect("retained exact session");
        assert_eq!(assembly.attempts, 0);
        assert_eq!(assembly.holder_cursor, 0);

        let reissued = transport
            .tick_bounded(&requester, now, 1)
            .expect("reissue released request")
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("unsent request is immediately reissued");
        assert_eq!(reissued.responder, first.responder);
        assert_ne!(
            reissued.request_id, first.request_id,
            "a reissued semantic occurrence must have a fresh canonical identity"
        );
        assert!(reissued.semantic_sequence > first.semantic_sequence);
        assert_eq!(reissued.closed_through, first.semantic_sequence.get());

        let rotated = transport
            .tick_bounded(&requester, now + REQUEST_TIMEOUT, 1)
            .expect("rotate timed-out request")
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("first real attempt expires at the base timeout");
        assert_ne!(rotated.responder, reissued.responder);

        let second_timeout = now + REQUEST_TIMEOUT + retry_timeout(REQUEST_TIMEOUT, 2);
        let forced_close = transport
            .tick_bounded(&requester, second_timeout, 1)
            .expect("service retained close debt before another late rotation")
            .pop()
            .expect("one bounded item remains ready");
        assert!(
            matches!(
                forced_close.message.as_ref(),
                CertifiedMergeSidecarMessage::Close(_)
            ),
            "consecutive late timeout ticks may defer a due Close at most once"
        );
        assert!(
            transport
                .tick_bounded(&requester, second_timeout, 1)
                .expect("resume the timed-out fetch after servicing one Close")
                .into_iter()
                .any(|post| matches!(
                    post.message.as_ref(),
                    CertifiedMergeSidecarMessage::Request(_)
                )),
            "Close debt service must retain the timed-out fetch for its next fair turn"
        );
    }

    #[test]
    fn idle_request_retry_starts_strictly_after_the_fairness_cursor() {
        let now = Instant::now();
        let requester = peer(b"requester");
        let mut transport = MergeSidecarTransport::new();
        for index in 0..4_u8 {
            let mut candidate = reference(1, 1);
            candidate.entry_hash = HashOf::from_untyped_unchecked(Hash::new([0xF2, index]));
            transport
                .defer_block(
                    HashOf::from_untyped_unchecked(Hash::new([0xB2, index])),
                    2,
                    0,
                    candidate,
                    &requester,
                    1,
                    now,
                )
                .expect("retain bounded session");
        }
        let cursor = transport
            .inbound_cursor
            .expect("at least one request was activated");
        let keys = transport.inbound.keys().copied().collect::<Vec<_>>();
        let start = keys.partition_point(|candidate| *candidate <= cursor);
        let expected = keys.get(start).copied().unwrap_or(keys[0]);

        let request = transport
            .tick_bounded(&requester, now + REQUEST_TIMEOUT, 1)
            .expect("service fairness successor")
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Close(_)
                | CertifiedMergeSidecarMessage::CloseAck(_)
                | CertifiedMergeSidecarMessage::GenerationHint(_)
                | CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("one timed-out or idle request is scheduled");
        assert_eq!((request.entry_hash, request.reference_digest), expected);
    }

    #[test]
    fn restart_drops_partial_assemblies() {
        let (mut transport, _, _, request, now) =
            start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1, 2);
        let responder = request.responder.clone();
        let bytes = vec![3_u8; MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1];
        assert!(matches!(
            transport.ingest_chunk(&responder, chunks(&request, &bytes).remove(0), now),
            Ok(ChunkIngestOutcome::Accepted)
        ));
        assert_eq!(transport.inbound_len(), 1);
        let restarted = MergeSidecarTransport::new();
        assert_eq!(restarted.inbound_len(), 0);
    }

    #[test]
    fn durable_requester_restart_advances_sequence_and_carries_close_floor() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let requester = peer(b"durable requester");
        let reference = reference(64, 1);
        let mut durable_requester = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open requester lifecycle journal");
        let first = durable_requester
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"durable requester block one")),
                2,
                0,
                reference.clone(),
                &requester,
                1,
                now,
            )
            .expect("defer first block")
            .expect("emit first request");
        let CertifiedMergeSidecarMessage::Request(first_request) =
            Arc::unwrap_or_clone(first.message)
        else {
            panic!("expected first request")
        };
        assert_eq!(first_request.semantic_sequence.get(), 1);
        assert_eq!(first_request.closed_through, 0);

        let local_peer = first_request.responder.clone();
        let hub = peer(b"durable requester live responder hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let first_route = routes.mint(requester.clone());
        let mut live_responder = MergeSidecarTransport::new();
        assert!(matches!(
            live_responder
                .admit_server_request(
                    &requester,
                    &first_request,
                    Some(&first_route),
                    &local_peer,
                    now,
                )
                .expect("live responder admits sequence one"),
            ServerRequestAdmission::Materialize
        ));

        drop(durable_requester);
        let mut restarted_requester = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart requester lifecycle journal");
        let recovered = &restarted_requester.request_streams[&local_peer];
        assert_eq!(recovered.next_sequence, 1);
        assert_eq!(recovered.closed_through, 1);
        assert_eq!(recovered.acknowledged_through, 0);
        assert!(recovered.open_sequences.is_empty());

        let second = restarted_requester
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"durable requester block two")),
                2,
                0,
                reference,
                &requester,
                1,
                now,
            )
            .expect("defer block after requester restart")
            .expect("emit request after requester restart");
        let CertifiedMergeSidecarMessage::Request(second_request) =
            Arc::unwrap_or_clone(second.message)
        else {
            panic!("expected second request")
        };
        assert_ne!(
            second_request.request_id, first_request.request_id,
            "restart recovery advances the exact semantic occurrence identity"
        );
        assert_eq!(second_request.semantic_sequence.get(), 2);
        assert_eq!(second_request.closed_through, 1);

        let second_route = routes
            .redeliver(&first_route)
            .expect("same live responder connection delivers sequence two");
        assert!(matches!(
            live_responder
                .admit_server_request(
                    &requester,
                    &second_request,
                    Some(&second_route),
                    &local_peer,
                    now,
                )
                .expect("live responder accepts the recovered close floor"),
            ServerRequestAdmission::Materialize
        ));
        assert_eq!(
            live_responder
                .server_streams
                .get(&requester)
                .map(|stream| stream.closed_through),
            Some(1)
        );
        assert_eq!(
            live_responder
                .server_streams
                .get(&requester)
                .map(|stream| stream.highest_sequence),
            Some(2)
        );
        assert_eq!(
            live_responder.server_request_gates[&(requester.clone(), second_request.request_id)]
                .semantic_sequence
                .get(),
            2
        );
    }

    #[test]
    fn durable_requester_crash_before_send_closes_unobserved_sequence() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let requester = peer(b"durable pre-send requester");
        let reference = reference(64, 1);
        let mut client = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open requester lifecycle journal");
        let unsent = client
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"durable unsent block")),
                2,
                0,
                reference,
                &requester,
                1,
                now,
            )
            .expect("persist request before send")
            .expect("allocate request before crash");
        let CertifiedMergeSidecarMessage::Request(unsent_request) =
            Arc::unwrap_or_clone(unsent.message)
        else {
            panic!("expected an unsent request")
        };
        let responder = unsent_request.responder.clone();
        drop(client);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart requester after pre-send crash");
        let close_post = restarted
            .tick_bounded(&requester, now, 1)
            .expect("schedule recovered close")
            .pop()
            .expect("unobserved durable sequence still requires a close");
        let CertifiedMergeSidecarMessage::Close(close) = Arc::unwrap_or_clone(close_post.message)
        else {
            panic!("recovered work must emit Close")
        };
        assert_eq!(close.closed_through, unsent_request.semantic_sequence.get());

        let mut fresh_responder = MergeSidecarTransport::new();
        let ack_post = fresh_responder
            .admit_server_close(&requester, &close, None, &responder)
            .expect("first-observation close is acknowledged statelessly");
        let CertifiedMergeSidecarMessage::CloseAck(ack) = Arc::unwrap_or_clone(ack_post.message)
        else {
            panic!("first-observation close must be acknowledged")
        };
        assert_eq!(fresh_responder.server_stream_count_for_test(), 0);
        assert_eq!(fresh_responder.server_request_gate_count_for_test(), 0);
        assert_eq!(fresh_responder.server_request_attempt_count_for_test(), 0);
        assert!(
            restarted
                .acknowledge_close(&responder, &ack, &requester)
                .expect("acknowledge recovered close")
        );
        assert!(
            restarted
                .tick_bounded(&requester, now + REQUEST_TIMEOUT, 1)
                .expect("service terminated recovered stream")
                .is_empty(),
            "the exact close ACK terminates local recovery work"
        );
    }

    #[test]
    fn durable_stream_epochs_and_service_generations_bound_peer_churn() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let reply_source_capacity = DEFAULT_REPLY_SOURCE_CAPACITY;
        let limits = MergeSidecarLimits::defaults();
        let churn = MAX_CERTIFIED_MERGE_SEMANTIC_PEERS;
        let initial_server_roster = (0..churn)
            .map(|index| peer(format!("semantic requester {index}").as_bytes()))
            .collect::<Vec<_>>();
        let initial_server_roster_digest =
            canonical_merge_sidecar_roster_digest(&initial_server_roster);
        let mut transport = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            reply_source_capacity,
            limits,
            initial_server_roster.len(),
            initial_server_roster_digest,
        )
        .expect("open durable churn fixture");

        let mut allocated_epochs = BTreeSet::new();
        for index in 0..churn {
            let responder = peer(format!("semantic responder {index}").as_bytes());
            let (epoch, sequence, closed_through) = transport
                .allocate_request_sequence(&responder)
                .expect("one requester stream fits each bounded semantic responder");
            assert!(
                allocated_epochs.insert(epoch),
                "requester-issued stream epochs are globally unique"
            );
            assert_eq!(sequence.get(), 1);
            assert_eq!(closed_through, 0);
            transport.close_request_sequence(&responder, epoch, sequence);
        }
        assert_eq!(
            transport.request_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );
        assert_eq!(
            transport.next_stream_epoch,
            u64::try_from(churn).expect("bounded churn count fits u64")
        );
        let blocked_responder = peer(b"semantic responder blocked by close debt");
        let full_requester_snapshot = transport
            .lifecycle_snapshot()
            .expect("snapshot the full requester table with unacknowledged close debt");
        assert!(matches!(
            transport.allocate_request_sequence(&blocked_responder),
            Err(MergeSidecarError::Capacity(
                "requester semantic responder geometry"
            ))
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot after requester-capacity rejection"),
            full_requester_snapshot,
            "capacity must not discard a durable Close which still needs an exact ACK"
        );

        let (_, _, _, base_request, _) = start_session(1, 1);
        let local_peer = base_request.responder.clone();
        let mut delayed_request = None;
        for requester in &initial_server_roster {
            let requester = requester.clone();
            let request = routed_server_request(&base_request, requester.clone(), b"semantic", 1);
            assert!(matches!(
                transport
                    .admit_server_request(&requester, &request, None, &local_peer, now)
                    .expect("admit one bounded server requester"),
                ServerRequestAdmission::Materialize
            ));
            delayed_request.get_or_insert((requester.clone(), request.clone()));
            let mut close = CertifiedMergeSidecarCloseV1 {
                version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
                service_generation: request.service_generation,
                stream_epoch: request.stream_epoch,
                closed_through: request.semantic_sequence.get(),
                close_id: Hash::prehashed([0; Hash::LENGTH]),
                requester: requester.clone(),
                responder: local_peer.clone(),
            };
            close.bind_canonical_close_id();
            transport
                .admit_server_close(&requester, &close, None, &local_peer)
                .expect("retire the requester's only active semantic gate");
            assert_eq!(transport.drain_closed_server_prefixes().len(), 1);
            transport.confirm_closed_server_prefix_handoff();
            assert!(transport.server_request_gates.is_empty());
        }
        assert_eq!(
            transport.server_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );
        let extra_requester = peer(b"semantic requester beyond roster");
        let mut extra_request =
            routed_server_request(&base_request, extra_requester.clone(), b"extra", 1);
        let full_terminal_snapshot = transport
            .lifecycle_snapshot()
            .expect("snapshot the full terminal server table");
        assert!(matches!(
            transport.admit_server_request(
                &extra_requester,
                &extra_request,
                None,
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity(
                "server semantic requester geometry"
            ))
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot after terminal server-capacity rejection"),
            full_terminal_snapshot,
            "same-roster peer churn cannot advance a responder generation"
        );

        let mut changed_server_roster = initial_server_roster.clone();
        changed_server_roster[0] = extra_requester.clone();
        let changed_server_roster_digest =
            canonical_merge_sidecar_roster_digest(&changed_server_roster);
        transport = transport
            .rehydrate_with_exact_geometry(
                reply_source_capacity,
                limits,
                changed_server_roster.len(),
                changed_server_roster_digest.clone(),
                now,
            )
            .expect("a certified changed roster advances the terminal responder generation");
        let mut stale_routes = NetworkReplyRouteTestFixture::with_source_capacity(
            peer(b"durable churn stale reply hub"),
            reply_source_capacity,
        );
        let extra_route = stale_routes.mint(extra_requester.clone());
        let ServerRequestAdmission::GenerationHint(post) = transport
            .admit_server_request(
                &extra_requester,
                &extra_request,
                Some(&extra_route),
                &local_peer,
                now,
            )
            .expect("old-generation traffic receives the changed-roster fence")
        else {
            panic!("changed-roster stale traffic must receive a GenerationHint")
        };
        assert!(
            post.reply_route
                .as_ref()
                .is_some_and(|route| route.same_delivery(&extra_route)),
            "the stale fence retains the exact authenticated delivery"
        );
        let CertifiedMergeSidecarMessage::GenerationHint(hint) = Arc::unwrap_or_clone(post.message)
        else {
            unreachable!("changed-roster stale traffic emits an exact Hint")
        };
        assert_eq!(
            hint.observed_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(hint.current_generation, service_generation(2));
        assert_eq!(
            hint.observed_message_hash,
            HashOf::new(&extra_request).into()
        );
        assert_eq!(hint.hint_id, hint.canonical_hint_id());
        assert_eq!(transport.server_service_generation, service_generation(2));
        assert!(transport.server_streams.is_empty());
        assert!(transport.server_request_gates.is_empty());
        assert_eq!(
            transport.drain_closed_server_prefixes().len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );

        let (delayed_requester, delayed_request) =
            delayed_request.expect("retain one compacted request");
        let delayed_route = stale_routes.mint(delayed_requester.clone());
        assert!(matches!(
            transport
                .admit_server_request(
                    &delayed_requester,
                    &delayed_request,
                    Some(&delayed_route),
                    &local_peer,
                    now,
                )
                .expect("a delayed compacted request is answered statelessly"),
            ServerRequestAdmission::GenerationHint(_)
        ));
        assert!(
            !transport.server_streams.contains_key(&delayed_requester),
            "old-generation replay must not recreate a per-peer tombstone"
        );

        extra_request.service_generation = hint.current_generation;
        extra_request.bind_canonical_request_id();
        assert!(matches!(
            transport
                .admit_server_request(&extra_requester, &extra_request, None, &local_peer, now,)
                .expect("retry under the advertised generation"),
            ServerRequestAdmission::Materialize
        ));
        let mut extra_close = CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: extra_request.service_generation,
            stream_epoch: extra_request.stream_epoch,
            closed_through: extra_request.semantic_sequence.get(),
            close_id: Hash::prehashed([0; Hash::LENGTH]),
            requester: extra_requester.clone(),
            responder: local_peer.clone(),
        };
        extra_close.bind_canonical_close_id();
        transport
            .admit_server_close(&extra_requester, &extra_close, None, &local_peer)
            .expect("terminate the new-generation server stream");
        assert_eq!(transport.drain_closed_server_prefixes().len(), 1);
        assert!(transport.server_request_gates.is_empty());
        transport
            .persist_lifecycle_state()
            .expect("persist compacted semantic streams atomically");
        let durable_epoch_high_water = transport.next_stream_epoch;
        drop(transport);

        let mut restarted = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            reply_source_capacity,
            limits,
            changed_server_roster.len(),
            changed_server_roster_digest,
        )
        .expect("restore compacted semantic streams under the changed roster");
        assert_eq!(
            restarted.request_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );
        assert_eq!(restarted.server_streams.len(), 1);
        assert!(restarted.server_request_gates.is_empty());
        assert_eq!(restarted.next_stream_epoch, durable_epoch_high_water);
        assert_eq!(restarted.server_service_generation, hint.current_generation);

        let post_restart_responder = peer(b"post-restart semantic responder");
        let restart_snapshot = restarted
            .lifecycle_snapshot()
            .expect("snapshot retained post-restart Close debt");
        assert!(matches!(
            restarted.allocate_request_sequence(&post_restart_responder),
            Err(MergeSidecarError::Capacity(
                "requester semantic responder geometry"
            ))
        ));
        assert_eq!(
            restarted
                .lifecycle_snapshot()
                .expect("snapshot rejected post-restart allocation"),
            restart_snapshot,
            "restart must retain every unacknowledged Close before admitting peer churn"
        );

        let close_requester = peer(b"post-restart Close requester");
        let releasable_responder = peer(b"semantic responder 0");
        let close = restarted
            .begin_close(&close_requester, &releasable_responder, now)
            .and_then(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Close(close) => Some(close),
                _ => None,
            })
            .expect("the retained requester stream retries its exact Close");
        let ack = CertifiedMergeSidecarCloseAckV1 {
            version: close.version,
            service_generation: close.service_generation,
            stream_epoch: close.stream_epoch,
            closed_through: close.closed_through,
            close_id: close.close_id,
            requester: close.requester,
            responder: close.responder,
        };
        assert!(
            restarted
                .acknowledge_close(&releasable_responder, &ack, &close_requester)
                .expect("an exact CloseAck releases one requester-stream slot")
        );
        let (post_restart_epoch, sequence, closed_through) = restarted
            .allocate_request_sequence(&post_restart_responder)
            .expect("an exact CloseAck permits a fresh globally unique epoch");
        assert_eq!(post_restart_epoch.get(), durable_epoch_high_water + 1);
        assert_eq!((sequence.get(), closed_through), (1, 0));
        assert!(!allocated_epochs.contains(&post_restart_epoch));
        restarted.close_request_sequence(&post_restart_responder, post_restart_epoch, sequence);

        let snapshot = restarted
            .lifecycle_snapshot()
            .expect("snapshot compacted semantic streams");
        assert_eq!(
            snapshot.payload.geometry.runtime.semantic_peer_capacity,
            u64::try_from(MAX_CERTIFIED_MERGE_SEMANTIC_PEERS)
                .expect("protocol roster bound fits u64")
        );

        let mut oversized_requesters = snapshot.payload.clone();
        oversized_requesters
            .request_streams
            .push(RequestStreamLifecycleV3 {
                responder: peer(b"oversized durable responder"),
                service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
                stream_epoch: stream_epoch(snapshot.payload.next_stream_epoch + 1),
                next_sequence: 1,
                closed_through: 1,
                acknowledged_through: 0,
            });
        let mut fresh = MergeSidecarTransport::with_limits(reply_source_capacity, limits)
            .expect("construct fresh restore target");
        assert!(matches!(
            fresh.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(oversized_requesters),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("exceeds configured source geometry")
        ));

        let server_service_generation = snapshot.payload.server_service_generation;
        let mut oversized_responders = snapshot.payload;
        for index in 0..MAX_CERTIFIED_MERGE_SEMANTIC_PEERS {
            oversized_responders
                .server_streams
                .push(ServerStreamLifecycleV3 {
                    requester: peer(format!("oversized durable requester {index}").as_bytes()),
                    service_generation: server_service_generation,
                    stream_epoch: stream_epoch(1),
                    closed_through: 1,
                    highest_sequence: 1,
                });
        }
        assert!(matches!(
            fresh.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(oversized_responders),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("exceeds configured source geometry")
        ));
    }

    #[test]
    fn service_generation_rollover_journal_failure_is_fail_atomic() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut server =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("open durable generation rollover fixture");
        for index in 0..MAX_CERTIFIED_MERGE_SEMANTIC_PEERS {
            server.server_streams.insert(
                peer(format!("durable generation requester {index}").as_bytes()),
                ServerStreamState {
                    stream_epoch: stream_epoch(1),
                    closed_through: 1,
                    highest_sequence: 1,
                },
            );
        }
        server
            .persist_lifecycle_state()
            .expect("persist the full terminal generation");
        let before = server
            .lifecycle_snapshot()
            .expect("snapshot before obstructed rollover");
        let changed_roster = vec![peer(b"durable rollover changed roster")];
        server.obstruct_lifecycle_journal_temp_for_test();

        assert!(matches!(
            server.transition_server_service_generation_for_test(&changed_roster),
            Err(MergeSidecarError::LifecycleJournal(_))
        ));
        assert_eq!(
            server
                .lifecycle_snapshot()
                .expect("snapshot after obstructed rollover"),
            before,
            "failed durable replacement must not install a generation or clear state"
        );
        assert!(server.pending_server_closures.is_empty());

        let journal = server
            .lifecycle_journal
            .as_ref()
            .expect("durable server retains its lifecycle journal");
        assert!(matches!(
            journal.load(),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unsafe lifecycle journal temp artifact")
        ));
        fs::remove_dir(journal.temp_path()).expect("remove the injected state obstruction");
        assert_eq!(
            journal
                .load()
                .expect("load the durable predecessor")
                .expect("the predecessor snapshot remains durable"),
            before
        );
        drop(server);
        let mut restarted =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("failed replacement leaves the complete predecessor snapshot");
        assert_eq!(
            restarted.server_service_generation,
            CertifiedMergeSidecarServiceGenerationV1::INITIAL
        );
        assert_eq!(
            restarted.server_streams.len(),
            MAX_CERTIFIED_MERGE_SEMANTIC_PEERS
        );
        assert_eq!(
            restarted
                .lifecycle_snapshot()
                .expect("snapshot the restarted predecessor"),
            before
        );

        restarted
            .transition_server_service_generation_for_test(&changed_roster)
            .expect("commit the complete successor after recovery");
        let successor = restarted
            .lifecycle_snapshot()
            .expect("snapshot the committed successor");
        assert_eq!(
            successor.payload.server_service_generation,
            service_generation(2)
        );
        assert!(successor.payload.server_streams.is_empty());
        assert!(successor.payload.server_request_gates.is_empty());
        assert_eq!(
            restarted
                .lifecycle_journal
                .as_ref()
                .expect("restarted server retains its lifecycle journal")
                .load()
                .expect("load the durable successor")
                .expect("the successor snapshot is durable"),
            successor
        );
    }

    #[test]
    fn durable_lifecycle_v3_root_high_water_is_exact_monotonic_and_noop_stable() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize the V3 lifecycle pair");
        let (initial, marker) = read_lifecycle_pair(&transport);
        assert_eq!(initial.payload.root_generation, 1);
        assert_eq!(marker.version, LIFECYCLE_JOURNAL_VERSION_V3);
        assert!(marker.matches(&initial));
        let state_path = transport.lifecycle_journal_state_path_for_test();
        let root_path = transport.lifecycle_root_high_water_path_for_test();
        let initial_state_bytes = fs::read(&state_path).expect("read initial lifecycle state");
        let initial_root_bytes = fs::read(&root_path).expect("read initial lifecycle root");

        transport
            .persist_lifecycle_state()
            .expect("an unchanged snapshot is a durable no-op");
        assert_eq!(
            fs::read(&state_path).expect("reread no-op lifecycle state"),
            initial_state_bytes
        );
        assert_eq!(
            fs::read(&root_path).expect("reread no-op lifecycle root"),
            initial_root_bytes
        );
        drop(transport);

        let mut restarted =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("an equal-geometry restart is also a durable no-op");
        assert_eq!(
            fs::read(&state_path).expect("read restarted lifecycle state"),
            initial_state_bytes
        );
        assert_eq!(
            fs::read(&root_path).expect("read restarted lifecycle root"),
            initial_root_bytes
        );

        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        assert!(matches!(
            restarted
                .admit_server_request(&requester, &request, None, &responder, now)
                .expect("commit one semantic responder occurrence"),
            ServerRequestAdmission::Materialize
        ));
        let (advanced, advanced_marker) = read_lifecycle_pair(&restarted);
        assert_eq!(advanced.payload.root_generation, 2);
        assert!(advanced_marker.matches(&advanced));
        let advanced_state_path = restarted.lifecycle_journal_state_path_for_test();
        assert_ne!(
            advanced_state_path, state_path,
            "successive generations alternate immutable state slots"
        );
        let advanced_state_bytes =
            fs::read(&advanced_state_path).expect("read advanced lifecycle state");
        let advanced_root_bytes = fs::read(&root_path).expect("read advanced lifecycle root");
        restarted
            .persist_lifecycle_state()
            .expect("repeating the advanced snapshot is a no-op");
        assert_eq!(
            fs::read(&advanced_state_path).expect("reread advanced lifecycle state"),
            advanced_state_bytes
        );
        assert_eq!(
            fs::read(&root_path).expect("reread advanced lifecycle root"),
            advanced_root_bytes
        );
    }

    #[test]
    fn durable_lifecycle_v3_bootstrap_recovers_first_commit_and_rejects_missing_roots() {
        let limits = MergeSidecarLimits::defaults();

        {
            let temp = tempfile::tempdir().expect("temp dir");
            let fixture = MergeSidecarTransport::with_limits(DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("construct an empty first-commit projection");
            let (mut journal, restored) = MergeSidecarLifecycleJournal::open(
                temp.path(),
                fixture
                    .lifecycle_protocol_max_snapshot_bytes()
                    .expect("derive lifecycle snapshot bound"),
            )
            .expect("publish the durable bootstrap sentinel");
            assert!(restored.is_none());
            let bootstrap = journal
                .decode_root_high_water(&journal.root_high_water_path())
                .expect("decode the bootstrap sentinel");
            assert!(bootstrap.is_bootstrap());
            let candidate = fixture
                .lifecycle_snapshot()
                .expect("snapshot the empty first generation");
            journal.fail_after_state_replace_before_directory_sync = true;
            assert!(matches!(
                journal.persist_next(candidate),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("state replacement but before directory synchronization")
            ));
            let state_path = journal.state_path_for_generation(1);
            let candidate = journal
                .decode_snapshot(&state_path)
                .expect("state replacement leaves the complete first-generation candidate");
            let root_path = journal.root_high_water_path();
            let bootstrap_bytes = fs::read(&root_path).expect("read bootstrap root");
            drop(journal);

            let recovered = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("validate and select the complete first-generation candidate");
            let (snapshot, marker) = read_lifecycle_pair(&recovered);
            assert_eq!(snapshot, candidate);
            assert!(marker.matches(&snapshot));
            assert_ne!(
                fs::read(root_path).expect("read committed first-generation root"),
                bootstrap_bytes,
                "first-generation validation replaces the bootstrap commit point"
            );
            assert!(state_path.is_file());
        }

        {
            let temp = tempfile::tempdir().expect("temp dir");
            let transport = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("initialize missing-root fixture");
            let state_path = transport.lifecycle_journal_state_path_for_test();
            let root_path = transport.lifecycle_root_high_water_path_for_test();
            let state_bytes = fs::read(&state_path).expect("read committed generation-one state");
            drop(transport);
            fs::remove_file(&root_path).expect("delete the committed root");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("directory survived without its root high-water")
            ));
            assert_eq!(
                fs::read(state_path).expect("missing-root rejection preserves committed state"),
                state_bytes
            );
            assert!(!root_path.exists());
        }

        {
            let temp = tempfile::tempdir().expect("temp dir");
            fs::create_dir(temp.path().join(LIFECYCLE_JOURNAL_DIR))
                .expect("install an ambiguous rootless lifecycle directory");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("directory survived without its root high-water")
            ));
            assert!(
                temp.path().join(LIFECYCLE_JOURNAL_DIR).is_dir(),
                "rootless directory remains for operator inspection"
            );
        }

        {
            let temp = tempfile::tempdir().expect("temp dir");
            let fixture = MergeSidecarTransport::with_limits(DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("construct bootstrap-only fixture");
            let (journal, restored) = MergeSidecarLifecycleJournal::open(
                temp.path(),
                fixture
                    .lifecycle_protocol_max_snapshot_bytes()
                    .expect("derive lifecycle snapshot bound"),
            )
            .expect("publish bootstrap sentinel and directory");
            assert!(restored.is_none());
            let root_path = journal.root_high_water_path();
            let bootstrap_bytes = fs::read(&root_path).expect("read bootstrap sentinel");
            let directory = journal.directory.clone();
            drop(journal);
            fs::remove_dir(&directory).expect("simulate crash before directory creation");
            let recovered = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("bootstrap sentinel uniquely authorizes directory recreation");
            assert_eq!(read_lifecycle_pair(&recovered).0.payload.root_generation, 1);
            assert_ne!(
                fs::read(root_path).expect("read initialized root"),
                bootstrap_bytes
            );
        }

        {
            let temp = tempfile::tempdir().expect("temp dir");
            let mut transport = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("initialize non-initial state-without-root fixture");
            let (_, requester, _, request, now) = start_session(1, 1);
            let responder = request.responder.clone();
            transport
                .admit_server_request(&requester, &request, None, &responder, now)
                .expect("publish generation two");
            let root_path = transport.lifecycle_root_high_water_path_for_test();
            let directory = transport
                .lifecycle_journal_state_path_for_test()
                .parent()
                .expect("lifecycle state has a directory")
                .to_path_buf();
            drop(transport);
            fs::remove_file(root_path).expect("remove the non-initial root high-water");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("directory survived without its root high-water")
            ));
            assert_eq!(
                fs::read_dir(directory)
                    .expect("inspect retained alternating slots")
                    .filter_map(Result::ok)
                    .count(),
                1,
                "a completed commit retires its predecessor slot"
            );
        }

        {
            let temp = tempfile::tempdir().expect("temp dir");
            let transport = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("initialize root-without-directory fixture");
            let state_path = transport.lifecycle_journal_state_path_for_test();
            let directory = state_path
                .parent()
                .expect("lifecycle state has a directory")
                .to_path_buf();
            let root_path = transport.lifecycle_root_high_water_path_for_test();
            let root_bytes = fs::read(&root_path).expect("read retained root high-water");
            drop(transport);
            fs::remove_file(state_path).expect("remove lifecycle state");
            fs::remove_dir(directory).expect("remove empty lifecycle directory");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("committed lifecycle root high-water survived without")
            ));
            assert_eq!(
                fs::read(root_path).expect("root survives rejected open"),
                root_bytes
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn durable_lifecycle_v3_rejects_windows_reparse_directory_before_noop_open() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store_root = temp.path().join("store");
        let junction_target = temp.path().join("junction-target");
        fs::create_dir(&store_root).expect("create lifecycle store root");
        fs::create_dir(&junction_target).expect("create junction target");
        let lifecycle_junction = store_root.join(LIFECYCLE_JOURNAL_DIR);
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&lifecycle_junction)
            .arg(&junction_target)
            .output()
            .expect("invoke the Windows junction creator");
        assert!(
            output.status.success(),
            "create lifecycle junction: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let fixture = MergeSidecarTransport::with_limits(
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("construct lifecycle geometry");
        let error = MergeSidecarLifecycleJournal::open(
            &store_root,
            fixture
                .lifecycle_protocol_max_snapshot_bytes()
                .expect("derive lifecycle snapshot bound"),
        )
        .expect_err("a Windows reparse directory cannot own lifecycle state");
        assert!(matches!(
            error,
            MergeSidecarError::LifecycleJournal(ref message)
                if message.contains("unsafe lifecycle journal directory")
        ));
        fs::remove_dir(&lifecycle_junction).expect("remove lifecycle junction");
    }

    #[test]
    fn durable_lifecycle_v3_rejects_crossed_bootstrap_and_committed_root_shapes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize malformed-root fixture");
        let (snapshot, marker) = read_lifecycle_pair(&transport);
        let state_path = transport.lifecycle_journal_state_path_for_test();
        let state_bytes = fs::read(&state_path).expect("read committed state");
        let root_path = transport.lifecycle_root_high_water_path_for_test();
        let committed_hash = marker
            .snapshot_hash
            .expect("committed marker carries its snapshot hash");
        drop(transport);

        for malformed in [
            MergeSidecarLifecycleRootHighWaterV3 {
                version: LIFECYCLE_JOURNAL_VERSION_V3,
                root_generation: snapshot.payload.root_generation,
                snapshot_hash: None,
            },
            MergeSidecarLifecycleRootHighWaterV3 {
                version: LIFECYCLE_JOURNAL_VERSION_V3,
                root_generation: 0,
                snapshot_hash: Some(committed_hash),
            },
        ] {
            fs::write(
                &root_path,
                norito::to_bytes(&malformed).expect("encode malformed root shape"),
            )
            .expect("install malformed root shape");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("malformed lifecycle root high-water")
            ));
            assert_eq!(
                fs::read(&state_path).expect("malformed root rejection preserves state"),
                state_bytes
            );
        }
    }

    #[test]
    fn durable_lifecycle_v3_recovers_regular_temps_and_rejects_unsafe_artifacts() {
        let limits = MergeSidecarLimits::defaults();
        for root_temp in [false, true] {
            let temp = tempfile::tempdir().expect("temp dir");
            let transport = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("initialize temp-artifact fixture");
            let journal = transport
                .lifecycle_journal
                .as_ref()
                .expect("durable fixture owns its journal");
            let artifact = if root_temp {
                journal.root_high_water_temp_path()
            } else {
                journal.temp_path()
            };
            let state_path = journal.state_path();
            let root_path = journal.root_high_water_path();
            let state_bytes = fs::read(&state_path).expect("read committed state");
            let root_bytes = fs::read(&root_path).expect("read committed root");
            let bytes = b"incomplete lifecycle commit";
            drop(transport);
            fs::write(&artifact, bytes).expect("install an incomplete regular temp file");
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("the exact committed pair discards its uncommitted regular temp");
            assert!(
                !artifact.exists(),
                "recovery removes only the known regular temp artifact"
            );
            assert_eq!(
                fs::read(state_path).expect("reread committed state"),
                state_bytes
            );
            assert_eq!(
                fs::read(root_path).expect("reread committed root"),
                root_bytes
            );
        }

        for root_temp in [false, true] {
            let temp = tempfile::tempdir().expect("temp dir");
            let transport = MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            )
            .expect("initialize unsafe-temp fixture");
            let journal = transport
                .lifecycle_journal
                .as_ref()
                .expect("durable fixture owns its journal");
            let artifact = if root_temp {
                journal.root_high_water_temp_path()
            } else {
                journal.temp_path()
            };
            let expected = if root_temp {
                "root high-water temp"
            } else {
                "state temp"
            };
            drop(transport);
            fs::create_dir(&artifact).expect("install an unsafe temp directory");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    limits,
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains(expected)
            ));
            assert!(artifact.is_dir(), "unsafe temp is never removed implicitly");
        }

        let temp = tempfile::tempdir().expect("temp dir");
        let transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize unknown-artifact fixture");
        let unknown = transport
            .lifecycle_journal_state_path_for_test()
            .with_file_name("untrusted.norito");
        drop(transport);
        fs::write(&unknown, b"unknown").expect("install unknown lifecycle artifact");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("unknown artifact in V3 lifecycle directory")
        ));
        assert_eq!(
            fs::read(&unknown).expect("unknown artifact remains for operator inspection"),
            b"unknown"
        );

        let temp = tempfile::tempdir().expect("temp dir");
        let transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize cleanup prevalidation fixture");
        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal");
        let committed_generation = journal
            .committed
            .as_ref()
            .expect("initialized fixture has a committed root")
            .root_generation;
        let inactive = journal.state_path_for_generation(committed_generation ^ 1);
        let state_temp = journal.temp_path();
        let root_temp = journal.root_high_water_temp_path();
        let state_temp_bytes = b"regular state temp retained for inspection";
        let root_temp_bytes = b"regular root temp retained for inspection";
        drop(transport);
        fs::write(&state_temp, state_temp_bytes).expect("install regular state temp");
        fs::write(&root_temp, root_temp_bytes).expect("install regular root temp");
        fs::create_dir(&inactive).expect("install unsafe inactive-slot directory");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("uncommitted state slot")
        ));
        assert_eq!(
            fs::read(&state_temp).expect("inactive-slot rejection preserves state temp"),
            state_temp_bytes
        );
        assert_eq!(
            fs::read(&root_temp).expect("inactive-slot rejection preserves root temp"),
            root_temp_bytes
        );
        assert!(
            inactive.is_dir(),
            "unsafe inactive slot remains for operator inspection"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let temp = tempfile::tempdir().expect("temp dir");
            let direct = temp.path().join("direct.norito");
            let alias = temp.path().join("alias.norito");
            fs::write(&direct, b"direct lifecycle bytes").expect("write direct artifact");
            symlink(&direct, &alias).expect("create lifecycle symlink");
            assert!(
                open_lifecycle_regular(&alias, "test alias").is_err(),
                "the no-follow open cannot acquire a symlink target"
            );
            assert!(matches!(
                MergeSidecarLifecycleJournal::read_bounded_regular(
                    &alias,
                    1024,
                    "test alias",
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("unsafe lifecycle test alias artifact")
            ));

            fs::remove_file(&alias).expect("remove lifecycle symlink");
            fs::hard_link(&direct, &alias).expect("create lifecycle hard link");
            assert!(matches!(
                MergeSidecarLifecycleJournal::read_bounded_regular(
                    &direct,
                    1024,
                    "test hard link",
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("unsafe lifecycle test hard link artifact")
            ));
            fs::remove_file(&alias).expect("restore single-link lifecycle artifact");

            let opened =
                open_lifecycle_regular(&direct, "test replacement").expect("open direct artifact");
            let replacement = temp.path().join("replacement.norito");
            fs::write(&replacement, b"replacement lifecycle bytes")
                .expect("write replacement artifact");
            fs::rename(&replacement, &direct)
                .expect("atomically replace the lifecycle path behind its open handle");
            assert!(matches!(
                verify_open_lifecycle_regular(&direct, &opened, "test replacement"),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("changed identity")
            ));
        }
    }

    #[test]
    fn durable_lifecycle_v3_validates_semantics_before_retiring_crash_artifacts() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize validation-order fixture");
        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal");
        let active_path = journal.state_path();
        let committed_generation = journal
            .committed
            .as_ref()
            .expect("initialized fixture has a committed root")
            .root_generation;
        let inactive_path = journal.state_path_for_generation(committed_generation ^ 1);
        let state_temp = journal.temp_path();
        let root_path = journal.root_high_water_path();
        let root_temp = journal.root_high_water_temp_path();
        let valid_state_bytes = fs::read(&active_path).expect("read valid lifecycle state");
        let valid_root_bytes = fs::read(&root_path).expect("read valid lifecycle root");
        let mut invalid_payload = read_lifecycle_pair(&transport).0.payload;
        invalid_payload
            .request_streams
            .push(RequestStreamLifecycleV3 {
                responder: peer(b"semantically invalid durable responder"),
                service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
                stream_epoch: stream_epoch(1),
                next_sequence: 0,
                closed_through: 0,
                acknowledged_through: 0,
            });
        let invalid = MergeSidecarLifecycleSnapshotV3::new(invalid_payload);
        let invalid_state_bytes =
            norito::to_bytes(&invalid).expect("encode self-consistent invalid state");
        let invalid_root_bytes =
            norito::to_bytes(&MergeSidecarLifecycleRootHighWaterV3::new(&invalid))
                .expect("encode root for self-consistent invalid state");
        let inactive_bytes = b"retained predecessor for operator recovery";
        let state_temp_bytes = b"retained incomplete state temp";
        let root_temp_bytes = b"retained incomplete root temp";
        drop(transport);

        fs::write(&active_path, &invalid_state_bytes).expect("install invalid selected state");
        fs::write(&root_path, &invalid_root_bytes).expect("install its matching root");
        fs::write(&inactive_path, inactive_bytes).expect("install inactive predecessor");
        fs::write(&state_temp, state_temp_bytes).expect("install regular state temp");
        fs::write(&root_temp, root_temp_bytes).expect("install regular root temp");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("request stream lifecycle regressed")
        ));
        for (path, expected) in [
            (&active_path, invalid_state_bytes.as_slice()),
            (&root_path, invalid_root_bytes.as_slice()),
            (&inactive_path, inactive_bytes.as_slice()),
            (&state_temp, state_temp_bytes.as_slice()),
            (&root_temp, root_temp_bytes.as_slice()),
        ] {
            assert_eq!(
                fs::read(path).expect("failed validation preserves every artifact"),
                expected
            );
        }

        fs::write(&active_path, &valid_state_bytes).expect("restore valid selected state");
        fs::write(&root_path, &valid_root_bytes).expect("restore its matching root");
        let recovered =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("valid selected pair permits bounded crash cleanup");
        assert_eq!(
            fs::read(recovered.lifecycle_journal_state_path_for_test())
                .expect("reread selected valid state"),
            valid_state_bytes
        );
        assert_eq!(
            fs::read(recovered.lifecycle_root_high_water_path_for_test())
                .expect("reread selected valid root"),
            valid_root_bytes
        );
        assert!(!inactive_path.exists());
        assert!(!state_temp.exists());
        assert!(!root_temp.exists());
    }

    #[test]
    fn durable_lifecycle_v3_rejects_split_generations_and_rehashed_state() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize split-pair fixture");
        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal");
        let state_path_generation_one = journal.state_path();
        let root_path = journal.root_high_water_path();
        let state_generation_one =
            fs::read(&state_path_generation_one).expect("read generation-one state");
        let root_generation_one = fs::read(&root_path).expect("read generation-one root");
        let snapshot_generation_one = journal
            .decode_snapshot(&state_path_generation_one)
            .expect("decode generation-one state");

        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("commit generation-two state");
        let state_path_generation_two = transport.lifecycle_journal_state_path_for_test();
        assert_ne!(state_path_generation_one, state_path_generation_two);
        let state_generation_two =
            fs::read(&state_path_generation_two).expect("read generation-two state");
        let root_generation_two = fs::read(&root_path).expect("read generation-two root");
        drop(transport);

        fs::write(&state_path_generation_two, &state_generation_one).expect("install old state");
        fs::write(&root_path, &root_generation_two).expect("retain new root");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("generation/hash mismatch")
        ));

        fs::write(&state_path_generation_one, &state_generation_two).expect("install new state");
        fs::write(&root_path, &root_generation_one).expect("install old root");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("generation/hash mismatch")
        ));

        let mut rehashed = snapshot_generation_one;
        rehashed.payload.next_stream_epoch = rehashed
            .payload
            .next_stream_epoch
            .checked_add(1)
            .expect("test stream epoch remains representable");
        rehashed.payload_hash = HashOf::new(&rehashed.payload);
        fs::write(
            &state_path_generation_one,
            norito::to_bytes(&rehashed).expect("encode rehashed state"),
        )
        .expect("install independently rehashed state");
        fs::write(&root_path, root_generation_one).expect("retain original root commitment");
        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("generation/hash mismatch")
        ));
    }

    #[test]
    fn durable_lifecycle_v3_generation_exhaustion_precedes_close_mutation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let close = close_for_request(&request);
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize maximum-generation fixture");
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("persist one responder occurrence");
        let mut maximum = read_lifecycle_pair(&transport).0;
        maximum.payload.root_generation = u64::MAX;
        install_lifecycle_pair(
            transport
                .lifecycle_journal
                .as_ref()
                .expect("durable fixture owns its journal"),
            maximum,
        );
        let state_path = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable fixture owns its journal")
            .state_path_for_generation(u64::MAX);
        let root_path = transport.lifecycle_root_high_water_path_for_test();
        let maximum_state_bytes = fs::read(&state_path).expect("read maximum-generation state");
        let maximum_root_bytes = fs::read(&root_path).expect("read maximum-generation root");
        drop(transport);

        let mut restarted =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("an unchanged maximum-generation pair can reopen");
        restarted
            .persist_lifecycle_state()
            .expect("an unchanged maximum-generation snapshot is a no-op");
        let before = restarted
            .lifecycle_snapshot()
            .expect("snapshot memory before exhausted close");
        assert!(matches!(
            restarted.admit_server_close(&requester, &close, None, &responder),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("root generation exhausted")
        ));
        assert_eq!(
            restarted
                .lifecycle_snapshot()
                .expect("snapshot memory after exhausted close"),
            before
        );
        assert_eq!(
            fs::read(&state_path).expect("reread maximum-generation state"),
            maximum_state_bytes
        );
        assert_eq!(
            fs::read(&root_path).expect("reread maximum-generation root"),
            maximum_root_bytes
        );
        assert!(matches!(
            restarted.admit_server_close(&requester, &close, None, &responder),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("requires process restart")
        ));
    }

    #[test]
    fn durable_lifecycle_v3_generation_exhaustion_precedes_writer_flush_cas() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        let hub = peer(b"maximum-generation flush hub");
        let mut routes =
            NetworkReplyRouteTestFixture::with_source_capacity(hub, DEFAULT_REPLY_SOURCE_CAPACITY);
        let route = routes.mint(requester.clone());
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize maximum-generation flush fixture");
        assert!(matches!(
            transport
                .admit_server_request(&requester, &request, Some(&route), &responder, now)
                .expect("admit one exact responder occurrence"),
            ServerRequestAdmission::Materialize
        ));
        transport
            .enqueue_response(request.clone(), Some(route), vec![0xA5], now)
            .expect("materialize one response chunk");
        let post = transport
            .drain_outbound_chunks_durable(1, now)
            .expect("publish the pending chunk identity")
            .pop()
            .expect("emit one response chunk");
        let admission = reply_chunk_admission(&post);

        let mut maximum = read_lifecycle_pair(&transport).0;
        maximum.payload.root_generation = u64::MAX;
        let marker = install_lifecycle_pair(
            transport
                .lifecycle_journal
                .as_ref()
                .expect("durable fixture owns its journal"),
            maximum,
        );
        transport
            .lifecycle_journal
            .as_mut()
            .expect("durable fixture owns its journal")
            .committed = Some(marker);
        let before = transport
            .lifecycle_snapshot()
            .expect("snapshot the maximum-generation pending flush");
        let key = (requester.clone(), request.request_id);
        let source = ServerRequestSource::Authenticated(
            post.reply_route
                .as_ref()
                .expect("response retains its exact route")
                .source_key(),
        );
        let pending_before = transport.server_request_gates[&key].attempts[&source].clone();

        assert!(matches!(
            transport.acknowledge_outbound_chunk(&admission, now),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("root generation exhausted")
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot after rejected writer flush"),
            before
        );
        let pending_after = &transport.server_request_gates[&key].attempts[&source];
        assert_eq!(pending_after.cursor, pending_before.cursor);
        assert_eq!(
            pending_after.pending_flush_chunk,
            pending_before.pending_flush_chunk
        );
        assert!(
            admission.flush_identity.claim_writer_flush_once(),
            "generation preflight must fail before claiming the writer-flush CAS"
        );
    }

    #[test]
    fn durable_lifecycle_v3_recovers_predecessor_before_state_directory_sync() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize pre-state-sync fixture");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("persist active predecessor ownership");
        let predecessor = transport
            .lifecycle_snapshot()
            .expect("snapshot the predecessor in memory");
        let root_path = transport.lifecycle_root_high_water_path_for_test();
        let predecessor_root = fs::read(&root_path).expect("read predecessor root");
        let successor_generation = predecessor
            .payload
            .root_generation
            .checked_add(1)
            .expect("test generation remains representable");
        let abandoned_successor_path = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns its journal")
            .state_path_for_generation(successor_generation);
        transport.fail_after_lifecycle_state_replace_before_sync_for_test();
        let changed_roster = vec![peer(b"pre-state-sync successor roster")];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        assert!(matches!(
            transport.transition_server_service_generation_after_exact_output_fence(
                changed_roster.len(),
                changed_digest,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("state replacement but before directory synchronization")
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot memory after pre-sync failure"),
            predecessor
        );
        assert_eq!(
            fs::read(&root_path).expect("reread predecessor root"),
            predecessor_root
        );
        assert!(abandoned_successor_path.is_file());
        drop(transport);

        let recovered =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("the predecessor root remains authoritative");
        assert_eq!(
            recovered
                .lifecycle_snapshot()
                .expect("snapshot the recovered predecessor"),
            predecessor
        );
        assert!(
            !abandoned_successor_path.exists(),
            "startup cleans the unselected slot only after cementing the selected pair"
        );
    }

    #[test]
    fn durable_lifecycle_v3_recovers_predecessor_between_state_and_root_publication() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize split-publication fixture");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("persist active predecessor ownership");
        let before = transport
            .lifecycle_snapshot()
            .expect("snapshot the predecessor in memory");
        let root_path = transport.lifecycle_root_high_water_path_for_test();
        let predecessor_root = fs::read(&root_path).expect("read predecessor root high-water");
        transport.fail_after_lifecycle_state_publish_for_test();
        let changed_roster = vec![peer(b"split publication successor roster")];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        assert!(matches!(
            transport.transition_server_service_generation_after_exact_output_fence(
                changed_roster.len(),
                changed_digest.clone(),
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("after lifecycle state publication")
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot memory after split publication"),
            before,
            "projection-first rollover cannot publish successor memory on failure"
        );
        assert_eq!(
            fs::read(&root_path).expect("reread predecessor root high-water"),
            predecessor_root,
            "the independent root still commits the predecessor"
        );
        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("failed transport retains its journal");
        let successor_generation = before
            .payload
            .root_generation
            .checked_add(1)
            .expect("test generation remains representable");
        let successor_state = journal
            .decode_snapshot(&journal.state_path_for_generation(successor_generation))
            .expect("the successor state was atomically published");
        let predecessor_marker = journal
            .decode_root_high_water(&root_path)
            .expect("the predecessor marker remains canonical");
        assert!(!predecessor_marker.matches(&successor_state));
        assert_eq!(
            journal
                .load()
                .expect("the old root selects the predecessor slot")
                .expect("the predecessor remains committed"),
            before
        );
        assert!(matches!(
            transport.persist_lifecycle_state(),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("requires process restart")
        ));
        drop(transport);

        let mut recovered =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("restart follows the predecessor root and ignores the uncommitted slot");
        assert_eq!(
            recovered
                .lifecycle_snapshot()
                .expect("snapshot recovered predecessor"),
            before
        );
        assert_eq!(recovered.server_request_gates.len(), 1);
        recovered
            .transition_server_service_generation_after_exact_output_fence(
                changed_roster.len(),
                changed_digest,
            )
            .expect("a later complete commit overwrites the abandoned successor slot");
        let committed = recovered
            .lifecycle_snapshot()
            .expect("snapshot the later complete successor");
        assert_eq!(committed.payload.root_generation, successor_generation);
        assert_eq!(
            committed.payload.server_service_generation,
            service_generation(2)
        );
        assert!(read_lifecycle_pair(&recovered).1.matches(&committed));
    }

    #[test]
    fn durable_lifecycle_v3_resyncs_replaced_root_before_predecessor_cleanup() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize unsynchronized-root fixture");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("persist active predecessor ownership");
        let predecessor = transport
            .lifecycle_snapshot()
            .expect("snapshot the predecessor in memory");
        let predecessor_path = transport.lifecycle_journal_state_path_for_test();
        let successor_generation = predecessor
            .payload
            .root_generation
            .checked_add(1)
            .expect("test generation remains representable");
        let successor_path = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns its journal")
            .state_path_for_generation(successor_generation);
        transport.fail_after_lifecycle_root_replace_before_sync_for_test();
        let changed_roster = vec![peer(b"unsynchronized root successor roster")];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        assert!(matches!(
            transport.transition_server_service_generation_after_exact_output_fence(
                changed_roster.len(),
                changed_digest.clone(),
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("root replacement but before store synchronization")
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot memory after unsynchronized root replacement"),
            predecessor,
            "a failed durable call cannot publish successor memory"
        );
        assert!(predecessor_path.is_file());
        assert!(successor_path.is_file());
        drop(transport);

        let recovered = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            limits,
            changed_roster.len(),
            changed_digest.clone(),
        )
        .expect("startup cements the selected root before cleaning its predecessor");
        let successor = recovered
            .lifecycle_snapshot()
            .expect("snapshot the recovered successor");
        assert_eq!(successor.payload.root_generation, successor_generation);
        assert_eq!(
            successor.payload.server_service_generation,
            service_generation(2)
        );
        assert!(successor.payload.server_streams.is_empty());
        assert!(successor.payload.server_request_gates.is_empty());
        assert!(
            !predecessor_path.exists(),
            "predecessor cleanup occurs only after startup resynchronizes the selected pair"
        );
        assert!(successor_path.is_file());
        drop(recovered);

        let reopened = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            limits,
            changed_roster.len(),
            changed_digest,
        )
        .expect("the cleaned successor pair survives a second restart");
        assert_eq!(
            reopened
                .lifecycle_snapshot()
                .expect("snapshot the twice-reopened successor"),
            successor
        );
    }

    #[test]
    fn durable_lifecycle_v3_recovers_successor_after_root_publication() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let mut transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("initialize post-root-publication fixture");
        let (_, requester, _, request, now) = start_session(1, 1);
        let responder = request.responder.clone();
        transport
            .admit_server_request(&requester, &request, None, &responder, now)
            .expect("persist active predecessor ownership");
        let predecessor = transport
            .lifecycle_snapshot()
            .expect("snapshot the predecessor in memory");
        transport.fail_after_lifecycle_root_publish_for_test();
        let changed_roster = vec![peer(b"post-root-publication successor roster")];
        let changed_digest = canonical_merge_sidecar_roster_digest(&changed_roster);
        assert!(matches!(
            transport.transition_server_service_generation_after_exact_output_fence(
                changed_roster.len(),
                changed_digest.clone(),
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("after lifecycle root publication")
        ));
        assert_eq!(
            transport
                .lifecycle_snapshot()
                .expect("snapshot memory after injected post-commit failure"),
            predecessor,
            "memory cannot publish a successor after the durable call reports failure"
        );
        assert!(matches!(
            transport
                .lifecycle_journal
                .as_ref()
                .expect("failed transport retains its journal")
                .load(),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("changed outside the live journal")
        ));
        drop(transport);

        let recovered = MergeSidecarTransport::open_durable_with_server_stream_capacity(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            limits,
            changed_roster.len(),
            changed_digest,
        )
        .expect("restart follows the durable successor root");
        let successor = recovered
            .lifecycle_snapshot()
            .expect("snapshot recovered successor");
        assert_eq!(
            successor.payload.root_generation,
            predecessor
                .payload
                .root_generation
                .checked_add(1)
                .expect("test generation remains representable")
        );
        assert_eq!(
            successor.payload.server_service_generation,
            service_generation(2)
        );
        assert!(successor.payload.server_streams.is_empty());
        assert!(successor.payload.server_request_gates.is_empty());
        assert!(recovered.pending_server_closures.is_empty());
        assert!(read_lifecycle_pair(&recovered).1.matches(&successor));
    }

    #[test]
    fn durable_lifecycle_v3_rejects_missing_state_with_surviving_root_high_water() {
        let temp = tempfile::tempdir().expect("temp dir");
        let limits = MergeSidecarLimits::defaults();
        let transport =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("perform the first lifecycle initialization");
        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns its lifecycle journal");
        fs::remove_file(journal.state_path()).expect("remove only the lifecycle state");
        drop(transport);

        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                limits,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("not both present")
        ));
    }

    #[test]
    fn durable_lifecycle_rejects_regressed_duplicate_and_cross_epoch_state() {
        let now = Instant::now();
        let responder_a = peer(b"durable epoch responder A");
        let responder_b = peer(b"durable epoch responder B");
        let mut requester = MergeSidecarTransport::new();
        requester
            .allocate_request_sequence(&responder_a)
            .expect("allocate first durable epoch");
        requester
            .allocate_request_sequence(&responder_b)
            .expect("allocate second durable epoch");
        let snapshot = requester
            .lifecycle_snapshot()
            .expect("snapshot requester epochs");

        let mut regressed_counter = snapshot.payload.clone();
        regressed_counter.next_stream_epoch = 0;
        let mut restore_target = MergeSidecarTransport::new();
        assert!(matches!(
            restore_target.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(regressed_counter),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("request stream lifecycle regressed")
        ));

        let mut duplicate_epoch = snapshot.payload;
        let first_epoch = duplicate_epoch.request_streams[0].stream_epoch;
        duplicate_epoch.request_streams[1].stream_epoch = first_epoch;
        assert!(matches!(
            restore_target.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(duplicate_epoch),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("request stream lifecycle regressed")
        ));

        let (_, server_requester, _, request, _) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&server_requester, &request, None, &local_peer, now,)
                .expect("admit durable server request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(request.clone(), None, vec![0xC1], now)
            .expect("materialize durable response");
        assert_eq!(server.drain_outbound_chunks(1, now).len(), 1);
        let server_snapshot = server
            .lifecycle_snapshot()
            .expect("snapshot pending server response");

        let mut gate_epoch_mismatch = server_snapshot.payload.clone();
        gate_epoch_mismatch.server_request_gates[0].stream_epoch =
            successor_stream_epoch(request.stream_epoch);
        assert!(matches!(
            restore_target.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(gate_epoch_mismatch),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("invalid durable server request gate")
        ));

        let mut marker_epoch_mismatch = server_snapshot.payload;
        marker_epoch_mismatch.server_request_gates[0].attempts[0]
            .pending_flush_chunk
            .as_mut()
            .expect("drained response retains a durable pending marker")
            .stream_epoch = successor_stream_epoch(request.stream_epoch);
        assert!(matches!(
            restore_target.restore_lifecycle_snapshot(
                MergeSidecarLifecycleSnapshotV3::new(marker_epoch_mismatch),
                now,
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("durable pending chunk differs")
        ));
    }

    #[test]
    fn durable_lifecycle_rejects_source_geometry_and_pending_marker_corruption() {
        let (_, requester, _, request, now) = start_session(MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1, 1);
        let local_peer = request.responder.clone();
        let hub = peer(b"durable corruption authenticated hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit the durable corruption fixture"),
            ServerRequestAdmission::Materialize
        ));
        server
            .enqueue_response(
                request.clone(),
                Some(route),
                vec![0xC2; MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1],
                now,
            )
            .expect("materialize the durable corruption fixture");
        assert_eq!(server.drain_outbound_chunks(1, now).len(), 1);
        let baseline = server
            .lifecycle_snapshot()
            .expect("snapshot one authenticated pending response")
            .payload;

        let restore_error = |payload: MergeSidecarLifecyclePayloadV3| {
            let mut target = MergeSidecarTransport::new();
            target
                .restore_lifecycle_snapshot(MergeSidecarLifecycleSnapshotV3::new(payload), now)
                .expect_err("corrupt lifecycle state must fail closed")
                .to_string()
        };

        let mut excess_attempts = baseline.clone();
        let gate = &mut excess_attempts.server_request_gates[0];
        let first_attempt = gate.attempts[0].clone();
        for index in 0..DEFAULT_REPLY_SOURCE_CAPACITY {
            let mut alternate = first_attempt.clone();
            alternate.source = DurableServerRequestSourceV3::Authenticated(peer(
                format!("excess durable source {index}").as_bytes(),
            ));
            gate.attempts.push(alternate);
        }
        assert!(
            restore_error(excess_attempts)
                .contains("durable server attempts exceed their source capacity")
        );

        let mut authenticated_without_capacity = baseline.clone();
        authenticated_without_capacity.server_request_gates[0].source_capacity = None;
        assert!(
            restore_error(authenticated_without_capacity)
                .contains("durable server source kind differs from its route geometry")
        );

        let mut synthetic_with_capacity = baseline.clone();
        synthetic_with_capacity.server_request_gates[0].attempts[0].source =
            DurableServerRequestSourceV3::Synthetic(requester.clone());
        assert!(
            restore_error(synthetic_with_capacity)
                .contains("durable server source kind differs from its route geometry")
        );

        let mut unaffiliated_synthetic = baseline.clone();
        unaffiliated_synthetic.server_request_gates[0].source_capacity = None;
        unaffiliated_synthetic.server_request_gates[0].attempts[0].source =
            DurableServerRequestSourceV3::Synthetic(peer(b"wrong durable synthetic requester"));
        assert!(
            restore_error(unaffiliated_synthetic)
                .contains("durable server source kind differs from its route geometry")
        );

        let mut terminal_with_pending = baseline.clone();
        terminal_with_pending.server_request_gates[0].attempts[0].cursor =
            DurableServerResponseCursorV3::Complete;
        assert!(
            restore_error(terminal_with_pending)
                .contains("terminal durable cursor retained an in-flight chunk")
        );

        let mut duplicate_occurrence = baseline.clone();
        let mut conflicting_gate = duplicate_occurrence.server_request_gates[0].clone();
        conflicting_gate.request.reference_digest =
            Hash::new(b"conflicting durable semantic occurrence");
        conflicting_gate.request.bind_canonical_request_id();
        conflicting_gate.request_id = conflicting_gate.request.request_id;
        conflicting_gate.request_hash = HashOf::new(&conflicting_gate.request);
        duplicate_occurrence
            .server_request_gates
            .push(conflicting_gate);
        assert!(
            restore_error(duplicate_occurrence)
                .contains("duplicate durable server semantic occurrence")
        );

        let mut unapplied_request_floor = baseline.clone();
        let gate = &mut unapplied_request_floor.server_request_gates[0];
        gate.request.semantic_sequence = semantic_sequence(2);
        gate.request.closed_through = 1;
        gate.request.bind_canonical_request_id();
        gate.request_id = gate.request.request_id;
        gate.request_hash = HashOf::new(&gate.request);
        gate.semantic_sequence = gate.request.semantic_sequence;
        unapplied_request_floor.server_streams[0].highest_sequence = 2;
        assert!(
            restore_error(unapplied_request_floor).contains("invalid durable server request gate")
        );

        let mut unsupported_high_water = baseline.clone();
        unsupported_high_water.server_streams[0].highest_sequence = 2;
        assert!(
            restore_error(unsupported_high_water)
                .contains("server stream high-water differs from durable request gates")
        );

        let mut wrong_chunk_count = baseline;
        wrong_chunk_count.server_request_gates[0].attempts[0]
            .pending_flush_chunk
            .as_mut()
            .expect("baseline retains one pending marker")
            .chunk_count += 1;
        assert!(
            restore_error(wrong_chunk_count)
                .contains("durable pending chunk differs from its request gate")
        );
    }

    #[test]
    fn durable_lifecycle_rejects_legacy_stream_state_without_guessing_a_layout() {
        for legacy in LEGACY_LIFECYCLE_JOURNAL_DIRS {
            let temp = tempfile::tempdir().expect("temp dir");
            fs::create_dir(temp.path().join(legacy))
                .expect("create legacy lifecycle journal directory");
            assert!(matches!(
                MergeSidecarTransport::open_durable(
                    temp.path(),
                    DEFAULT_REPLY_SOURCE_CAPACITY,
                    MergeSidecarLimits::defaults(),
                ),
                Err(MergeSidecarError::LifecycleJournal(ref error))
                    if error.contains("unsupported legacy lifecycle journal")
            ));
            assert!(
                !temp.path().join(LIFECYCLE_JOURNAL_DIR).exists(),
                "V1/V2 state must be rejected before a V3 journal is created"
            );
        }
    }

    #[test]
    fn durable_lifecycle_rejects_canonical_payload_with_stale_digest() {
        let temp = tempfile::tempdir().expect("temp dir");
        let now = Instant::now();
        let requester = peer(b"durable corrupt lifecycle requester");
        let mut transport = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open lifecycle journal");
        transport
            .defer_block(
                HashOf::from_untyped_unchecked(Hash::new(b"durable corrupt lifecycle block")),
                2,
                0,
                reference(64, 1),
                &requester,
                1,
                now,
            )
            .expect("persist one semantic request")
            .expect("request has a holder");

        let journal = transport
            .lifecycle_journal
            .as_ref()
            .expect("durable transport owns its journal");
        let snapshot = journal
            .load()
            .expect("load valid lifecycle snapshot")
            .expect("snapshot exists");
        let mut snapshot = snapshot;
        snapshot.payload.request_streams[0].next_sequence = snapshot.payload.request_streams[0]
            .next_sequence
            .checked_add(1)
            .expect("test sequence remains representable");
        assert!(
            !snapshot.integrity_is_valid(),
            "the semantic mutation must leave the prior payload digest stale"
        );
        let canonical =
            norito::to_bytes(&snapshot).expect("mutated snapshot still has canonical Norito bytes");
        assert_eq!(
            norito::decode_from_bytes::<MergeSidecarLifecycleSnapshotV3>(&canonical)
                .expect("mutated canonical bytes remain structurally decodable"),
            snapshot
        );
        fs::write(journal.state_path(), canonical)
            .expect("replace state with canonical corruption");
        drop(transport);

        assert!(matches!(
            MergeSidecarTransport::open_durable(
                temp.path(),
                DEFAULT_REPLY_SOURCE_CAPACITY,
                MergeSidecarLimits::defaults(),
            ),
            Err(MergeSidecarError::LifecycleJournal(ref error))
                if error.contains("payload digest mismatch")
        ));
    }

    #[test]
    fn durable_responder_restart_preserves_same_hub_gate_budget() {
        let temp = tempfile::tempdir().expect("temp dir");
        let (_, _, _, base_request, now) = start_session(1, 1);
        let local_peer = base_request.responder.clone();
        let hub_a = peer(b"durable gate budget hub a");
        let hub_b = peer(b"durable gate budget hub b");
        let origin = peer(b"durable gate budget origin");
        let mut first_actor = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let mut limits = MergeSidecarLimits::defaults();
        limits.inbound_sessions_per_peer = MAX_SERVER_REQUEST_GATES_PER_SOURCE + 1;
        let mut server =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("open responder lifecycle journal");

        for index in 0..MAX_SERVER_REQUEST_GATES_PER_SOURCE {
            let mut request = routed_server_request(
                &base_request,
                origin.clone(),
                format!("durable gate request {index}").as_bytes(),
                1,
            );
            request.semantic_sequence = semantic_sequence(
                u64::try_from(index + 1).expect("bounded gate sequence fits u64"),
            );
            request.bind_canonical_request_id();
            let route = first_actor.mint_via(origin.clone(), hub_a.clone());
            server
                .admit_server_request(&origin, &request, Some(&route), &local_peer, now)
                .expect("admit one same-requester/same-hub gate before restart");
            server
                .persist_lifecycle_state()
                .expect("persist same-hub gate");
        }
        drop(server);

        let mut restarted =
            MergeSidecarTransport::open_durable(temp.path(), DEFAULT_REPLY_SOURCE_CAPACITY, limits)
                .expect("restart responder with full same-hub gate budget");
        let mut restarted_actor = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let mut fifth_request = routed_server_request(&base_request, origin.clone(), b"fifth", 1);
        fifth_request.semantic_sequence = semantic_sequence(
            u64::try_from(MAX_SERVER_REQUEST_GATES_PER_SOURCE + 1).expect("bounded fifth sequence"),
        );
        fifth_request.bind_canonical_request_id();
        let fifth_route = restarted_actor.mint_via(origin.clone(), hub_a.clone());
        assert!(matches!(
            restarted.admit_server_request(
                &origin,
                &fifth_request,
                Some(&fifth_route),
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));

        let independent_origin = peer(b"durable independent gate origin");
        let independent_request =
            routed_server_request(&base_request, independent_origin.clone(), b"independent", 1);
        let same_hub_route = restarted_actor.mint_via(independent_origin.clone(), hub_a.clone());
        assert!(matches!(
            restarted.admit_server_request(
                &independent_origin,
                &independent_request,
                Some(&same_hub_route),
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));
        let independent_route = restarted_actor.mint_via(independent_origin.clone(), hub_b.clone());
        restarted
            .admit_server_request(
                &independent_origin,
                &independent_request,
                Some(&independent_route),
                &local_peer,
                now,
            )
            .expect("an independent authenticated hub retains its gate corridor");
        let saturated_source = ServerRequestSource::Authenticated(same_hub_route.source_key());
        let independent_source = ServerRequestSource::Authenticated(independent_route.source_key());
        assert_eq!(
            restarted.source_gate_count(&saturated_source),
            MAX_SERVER_REQUEST_GATES_PER_SOURCE
        );
        assert_eq!(restarted.source_gate_count(&independent_source), 1);
        assert_eq!(
            independent_route.authenticated_source_peer(),
            &hub_b,
            "the admitted gate is charged to the shared authenticated hub"
        );
    }

    #[test]
    fn durable_responder_restart_allows_new_source_while_recovered_source_is_offline() {
        let temp = tempfile::tempdir().expect("temp dir");
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 1);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"durable offline source a");
        let hub_b = peer(b"durable responsive source b");
        let mut first_actor = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = first_actor.mint(requester.clone());
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open responder lifecycle journal");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit source A"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist source A gate");
        server
            .enqueue_response(request.clone(), Some(route_a), vec![0xE5; len], now)
            .expect("materialize source A response");
        let first_a = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("source A receives chunk zero");
        assert!(acknowledge_reply_chunk(&mut server, &first_a, now));
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart responder with source A offline");
        let key = (requester.clone(), request.request_id);
        let recovered_a = ServerRequestSource::RecoveredAuthenticated(hub_a);
        assert_eq!(
            restarted.server_request_gates[&key].attempts[&recovered_a].cursor,
            ServerResponseCursor::Pending(1)
        );

        let mut restarted_actor = NetworkReplyRouteTestFixture::new(hub_b.clone());
        let route_b = restarted_actor.mint(requester.clone());
        assert!(matches!(
            restarted
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("responsive source B attaches while recovered A stays offline"),
            ServerRequestAdmission::Materialize
        ));
        let source_b = ServerRequestSource::Authenticated(route_b.source_key());
        assert_eq!(
            restarted.server_request_gates[&key].attempts[&recovered_a].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert_eq!(
            restarted.server_request_gates[&key].attempts[&source_b].cursor,
            ServerResponseCursor::Pending(0)
        );
        restarted
            .enqueue_response(request, Some(route_b.clone()), vec![0xE5; len], now)
            .expect("materialize shared bytes through source B");
        let first_b = restarted
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("source B starts at chunk zero");
        assert!(matches!(
            &first_b,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&route_b)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk) if chunk.chunk_index == 0)
        ));
        assert_eq!(
            restarted.server_request_gates[&key].attempts[&recovered_a].cursor,
            ServerResponseCursor::Pending(1),
            "source B materialization cannot reset offline source A"
        );
    }

    #[test]
    fn durable_response_drain_persists_pending_identity_before_handoff() {
        let temp = tempfile::tempdir().expect("temp dir");
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 1);
        let local_peer = request.responder.clone();
        let hub = peer(b"durable pending response hub");
        let mut first_actor = NetworkReplyRouteTestFixture::new(hub.clone());
        let route = first_actor.mint(requester.clone());
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open durable responder");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                .expect("admit durable request"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist request admission before materialization");
        server
            .enqueue_response(request.clone(), Some(route), vec![0xA7; len], now)
            .expect("materialize durable response");
        assert!(
            server.outbound_drain_requires_lifecycle_commit(1),
            "the first handoff must publish its exact pending identity"
        );
        let first = server
            .drain_outbound_chunks_durable(1, now)
            .expect("persist pending chunk before handoff")
            .pop()
            .expect("emit first response chunk");
        let pending = ServerPendingChunkIdentity::from_message(&first.message)
            .expect("response post has a pending chunk identity");
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart durable responder");
        let key = (requester.clone(), request.request_id);
        let recovered_source = ServerRequestSource::RecoveredAuthenticated(hub.clone());
        let recovered = &restarted.server_request_gates[&key].attempts[&recovered_source];
        assert_eq!(recovered.cursor, ServerResponseCursor::Pending(0));
        assert_eq!(recovered.pending_flush_chunk.as_ref(), Some(&pending));

        let mut restarted_actor = NetworkReplyRouteTestFixture::new(hub);
        let rebound = restarted_actor.mint(requester.clone());
        assert!(matches!(
            restarted
                .admit_server_request(&requester, &request, Some(&rebound), &local_peer, now,)
                .expect("rebind recovered pending source"),
            ServerRequestAdmission::Materialize
        ));
        restarted
            .persist_lifecycle_state()
            .expect("persist rebound source before rematerialization");
        restarted
            .enqueue_response(request, Some(rebound), vec![0xA7; len], now)
            .expect("rematerialize identical response");
        assert!(
            !restarted.outbound_drain_requires_lifecycle_commit(1),
            "an exact retry of the already-published pending identity is a journal no-op"
        );
        let retried = restarted
            .drain_outbound_chunks_durable(1, now)
            .expect("persist retried pending chunk")
            .pop()
            .expect("retry the retained current chunk");
        assert_eq!(
            ServerPendingChunkIdentity::from_message(&retried.message),
            Some(pending),
            "restart may retry only the exact durably retained chunk identity"
        );
    }

    #[test]
    fn durable_responder_restart_preserves_terminal_source_cursor_and_rebinds_capability() {
        let temp = tempfile::tempdir().expect("temp dir");
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 1);
        let local_peer = request.responder.clone();
        let hub = peer(b"durable responder hub");
        let mut first_actor = NetworkReplyRouteTestFixture::new(hub.clone());
        let route = first_actor.mint(requester.clone());
        let mut server = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("open responder lifecycle journal");
        assert!(matches!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit request before responder restart"),
            ServerRequestAdmission::Materialize
        ));
        server
            .persist_lifecycle_state()
            .expect("persist admitted responder gate");
        server
            .enqueue_response(request.clone(), Some(route), vec![0xD3; len], now)
            .expect("materialize response before responder restart");
        loop {
            let Some(post) = server.drain_outbound_chunks(1, now).pop() else {
                break;
            };
            assert!(acknowledge_reply_chunk(&mut server, &post, now));
        }
        assert!(server.outbound.is_empty());
        let key = (requester.clone(), request.request_id);
        assert!(
            server.server_request_gates[&key]
                .attempts
                .values()
                .all(|attempt| attempt.cursor == ServerResponseCursor::Complete
                    && attempt.pending_flush_chunk.is_none())
        );
        drop(server);

        let mut restarted = MergeSidecarTransport::open_durable(
            temp.path(),
            DEFAULT_REPLY_SOURCE_CAPACITY,
            MergeSidecarLimits::defaults(),
        )
        .expect("restart responder lifecycle journal");
        assert!(matches!(
            restarted.server_request_gates[&key]
                .attempts
                .keys()
                .next(),
            Some(ServerRequestSource::RecoveredAuthenticated(peer)) if peer == &hub
        ));
        let mut restarted_actor = NetworkReplyRouteTestFixture::new(hub);
        let rebound = restarted_actor.mint(requester.clone());
        assert!(matches!(
            restarted
                .admit_server_request(&requester, &request, Some(&rebound), &local_peer, now,)
                .expect("rebind terminal source to the new process-local capability"),
            ServerRequestAdmission::Existing
        ));
        let rebound_source = ServerRequestSource::Authenticated(rebound.source_key());
        let rebound_attempt = &restarted.server_request_gates[&key].attempts[&rebound_source];
        assert_eq!(rebound_attempt.cursor, ServerResponseCursor::Complete);
        assert!(rebound_attempt.pending_flush_chunk.is_none());
        assert!(restarted.outbound.is_empty());
        assert!(
            restarted.drain_outbound_chunks(1, now).is_empty(),
            "terminal durable progress must never rematerialize or replay response bytes"
        );
    }

    #[test]
    fn corrupt_canonical_bytes_are_rejected() {
        let reference = reference(4, 1);
        assert!(matches!(
            decode_certified_merge_sidecar(&reference, &[1, 2, 3, 4]),
            Err(MergeSidecarError::Decode(_))
        ));
    }

    #[test]
    fn unsupported_merge_entry_version_is_rejected_before_reference_matching() {
        let seed_reference = reference(1, 1);
        let merge_qc = seed_reference.merge_qc.clone();
        let context = MergeSigningContextV1 {
            epoch_id: seed_reference.epoch_id,
            view: merge_qc.view,
            carrier_height: merge_qc.carrier_height,
            parent_hash: merge_qc.carrier_parent_hash,
            validator_set_hash: merge_qc.validator_set_hash,
        };
        let mut entry = signing_candidate(&context, b"unsupported-version").into_entry(merge_qc);
        entry.version = MergeLedgerEntry::VERSION + 1;
        let bytes = entry.canonical_bytes();
        let reference = CertifiedMergeLedgerReference::new(&entry);

        assert!(matches!(
            decode_certified_merge_sidecar(&reference, &bytes),
            Err(MergeSidecarError::Decode(ref message))
                if message.contains("unsupported merge ledger entry version")
        ));
    }

    #[test]
    fn signing_guard_is_restart_safe_and_rejects_equivocation() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 4,
            view: 2,
            carrier_height: 9,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-9")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let first = Hash::new(b"first");
        let second = Hash::new(b"second");
        let candidate = signing_candidate(&context, b"first");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), first, &candidate)
            .expect("first authorization");
        guard
            .authorize(context.clone(), first, &candidate)
            .expect("idempotent authorization");
        let mut unsupported_candidate = candidate.clone();
        unsupported_candidate.version = MergeLedgerCandidate::VERSION + 1;
        assert_eq!(
            guard.authorize(context.clone(), first, &unsupported_candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
        assert_eq!(
            guard.authorize(context.clone(), second, &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
        drop(guard);
        let restarted = MergeSigningGuard::open(temp.path()).expect("restart guard");
        let (recovered_digest, recovered_candidate, recovered_bytes) = restarted
            .authorized_candidate(&context)
            .expect("read durable candidate")
            .expect("candidate survives restart");
        assert_eq!(recovered_digest, first);
        assert_eq!(recovered_candidate, candidate);
        assert_eq!(recovered_bytes, candidate.canonical_bytes());
        assert_eq!(
            restarted.authorize(context.clone(), second, &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );

        let mut substituted = candidate.clone();
        substituted.global_state_root = Hash::new(b"substituted candidate body");
        assert_eq!(
            restarted.authorize(context, first, &substituted),
            Err(MergeSidecarError::LocalSigningEquivocation),
            "equal digest cannot substitute different canonical candidate bytes"
        );
    }

    #[test]
    fn signing_guard_rejects_legacy_journal_without_implicit_recovery() {
        let temp = tempfile::tempdir().expect("temp dir");
        fs::create_dir(temp.path().join(LEGACY_SIGNING_GUARD_DIRS[0]))
            .expect("create legacy journal");
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(message))
                if message.contains("authenticated candidate-body recovery")
        ));
        assert!(!temp.path().join(SIGNING_GUARD_DIR).exists());
    }

    #[test]
    fn signing_guard_rejects_aggregate_oversize_before_recovery_scan() {
        let temp = tempfile::tempdir().expect("temp dir");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let path = guard.directory.join(format!(
            "{}.{}",
            Hash::new(b"oversized signing guard"),
            SIGNING_GUARD_RECORD_EXT
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .expect("create sparse oversized artifact");
        file.set_len(
            u64::try_from(MAX_SIGNING_GUARD_TOTAL_BYTES)
                .expect("aggregate bound fits u64")
                .saturating_add(1),
        )
        .expect("size sparse oversized artifact");
        drop(file);
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(message))
                if message.contains("aggregate bytes")
        ));
    }

    #[test]
    fn signing_guard_rejects_oversized_candidate_temp() {
        let temp = tempfile::tempdir().expect("temp dir");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let path = guard.directory.join(format!(
            "{}.{}",
            Hash::new(b"oversized signing guard temp"),
            SIGNING_GUARD_TEMP_EXT
        ));
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .expect("create sparse oversized temp");
        file.set_len(
            u64::try_from(MAX_SIGNING_GUARD_RECORD_BYTES)
                .expect("record bound fits u64")
                .saturating_add(1),
        )
        .expect("size sparse oversized temp");
        drop(file);
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(message))
                if message.contains("unsafe signing-guard record temp")
        ));
    }

    #[test]
    fn signing_guard_rejects_truncated_final_record() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 5,
            view: 3,
            carrier_height: 11,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-10")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"truncated final record");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"truncated"), &candidate)
            .expect("authorize candidate before truncation");
        let path = guard.record_path(&context);
        let mut bytes = fs::read(&path).expect("read exact final record");
        bytes.truncate(bytes.len() / 2);
        fs::write(path, bytes).expect("install truncated final record");
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
    }

    #[test]
    fn signing_guard_high_water_allows_more_than_record_cap_committed_epochs() {
        let temp = tempfile::tempdir().expect("temp dir");
        let roster_hash = HashOf::new(&vec![peer(b"validator")]);
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        for epoch_id in 1..=(MAX_SIGNING_GUARD_RECORDS as u64 + 64) {
            let context = MergeSigningContextV1 {
                epoch_id,
                view: 0,
                carrier_height: epoch_id + 1,
                parent_hash: HashOf::from_untyped_unchecked(Hash::new(epoch_id.to_le_bytes())),
                validator_set_hash: roster_hash,
            };
            let candidate = signing_candidate(&context, &epoch_id.to_le_bytes());
            guard
                .authorize(context, Hash::new(epoch_id.to_le_bytes()), &candidate)
                .expect("authorize next epoch");
            guard
                .advance_committed_epoch(epoch_id)
                .expect("advance committed high-water");
        }
        let restarted = MergeSigningGuard::open_with_committed_epoch(
            temp.path(),
            MAX_SIGNING_GUARD_RECORDS as u64 + 64,
        )
        .expect("restart beyond record cap");
        assert_eq!(
            restarted.committed_epoch,
            MAX_SIGNING_GUARD_RECORDS as u64 + 64
        );
    }
    #[test]
    fn signing_guard_height_high_water_handles_many_ordinary_carrier_misses() {
        let temp = tempfile::tempdir().expect("temp dir");
        let roster_hash = HashOf::new(&vec![peer(b"validator")]);
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let rounds = MAX_SIGNING_GUARD_RECORDS as u64 + 64;
        for carrier_height in 1..=rounds {
            let context = MergeSigningContextV1 {
                epoch_id: 1,
                view: 0,
                carrier_height,
                parent_hash: HashOf::from_untyped_unchecked(Hash::new(
                    carrier_height.saturating_sub(1).to_le_bytes(),
                )),
                validator_set_hash: roster_hash,
            };
            let candidate = signing_candidate(&context, &carrier_height.to_le_bytes());
            guard
                .authorize(context, Hash::new(carrier_height.to_le_bytes()), &candidate)
                .expect("authorize exact uncommitted carrier round");
            guard
                .advance_committed_frontier(0, carrier_height)
                .expect("ordinary global block finalizes carrier height");
        }
        drop(guard);
        let restarted = MergeSigningGuard::open_with_committed_frontier(
            temp.path(),
            0,
            rounds,
            MergeSigningGuardLimits::defaults(),
        )
        .expect("restart after many ordinary blocks");
        assert_eq!(restarted.committed_carrier_height, rounds);

        let later = MergeSigningContextV1 {
            epoch_id: 1,
            view: 0,
            carrier_height: rounds + 1,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(rounds.to_le_bytes())),
            validator_set_hash: roster_hash,
        };
        let later_candidate = signing_candidate(&later, b"later candidate");
        restarted
            .authorize(later, Hash::new(b"later candidate"), &later_candidate)
            .expect("same epoch/view remains signable at a new exact carrier");
    }

    #[test]
    fn signing_guard_reconciles_partial_temps_without_weakening_final_records() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 2,
            view: 1,
            carrier_height: 8,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-8")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let first = Hash::new(b"first");
        let second = Hash::new(b"second");
        let candidate = signing_candidate(&context, b"partial-temp");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), first, &candidate)
            .expect("authorize final record");
        let record_temp = guard.record_path(&context).with_extension("norito.tmp");
        fs::write(&record_temp, [0xA5, 0x5A]).expect("write partial record temp");
        let high_water_temp = MergeSigningGuard::high_water_temp_path(&guard.directory);
        fs::write(&high_water_temp, [0x01]).expect("write partial high-water temp");
        drop(guard);

        let restarted = MergeSigningGuard::open(temp.path()).expect("reconcile partial temps");
        assert!(!record_temp.exists());
        assert!(!high_water_temp.exists());
        assert_eq!(
            restarted.authorize(context, second, &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }

    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_symlink_and_unknown_temps() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        let target = temp.path().join("target");
        fs::write(&target, b"target").expect("write target");
        let malicious =
            guard
                .directory
                .join(format!("{}.{}", Hash::new(b"temp"), SIGNING_GUARD_TEMP_EXT));
        symlink(&target, &malicious).expect("create symlink temp");
        drop(guard);
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
        fs::remove_file(&malicious).expect("remove malicious symlink");
        fs::write(
            temp.path().join(SIGNING_GUARD_DIR).join("unknown.tmp"),
            b"unknown",
        )
        .expect("write unknown temp");
        assert!(matches!(
            MergeSigningGuard::open(temp.path()),
            Err(MergeSidecarError::SigningGuard(_))
        ));
    }

    #[test]
    fn signing_guard_prune_boundary_never_reopens_committed_context() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 1,
            view: 0,
            carrier_height: 2,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-1")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"prune-boundary");
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"), &candidate)
            .expect("authorize epoch");
        guard
            .advance_committed_epoch(1)
            .expect("commit and prune epoch");
        drop(guard);
        let restarted =
            MergeSigningGuard::open_with_committed_epoch(temp.path(), 1).expect("restart guard");
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict"), &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }

    #[test]
    fn signing_guard_restart_completes_gc_after_high_water_crash_boundary() {
        let temp = tempfile::tempdir().expect("temp dir");
        let context = MergeSigningContextV1 {
            epoch_id: 1,
            view: 3,
            carrier_height: 2,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent-1")),
            validator_set_hash: HashOf::new(&vec![peer(b"validator")]),
        };
        let candidate = signing_candidate(&context, b"high-water-recovery");
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"), &candidate)
            .expect("authorize decision");
        let record_path = guard.record_path(&context);
        let record_bytes = fs::read(&record_path).expect("capture durable decision");
        guard
            .advance_committed_frontier(1, 2)
            .expect("persist high-water and collect decision");
        assert!(!record_path.exists());

        // Recreate the exact on-disk state of a crash after the high-water was
        // fsynced but immediately before the now-idempotent record GC.
        fs::write(&record_path, record_bytes).expect("restore stale durable decision");
        drop(guard);
        let restarted = MergeSigningGuard::open_with_committed_frontier(
            temp.path(),
            1,
            2,
            MergeSigningGuardLimits::defaults(),
        )
        .expect("restart completes stale-record GC");
        assert!(!record_path.exists());
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict"), &candidate),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }
}
