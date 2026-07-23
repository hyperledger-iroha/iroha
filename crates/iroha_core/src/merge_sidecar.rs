//! Bounded, authenticated transfer of certified merge-ledger sidecars.
//!
//! Global blocks carry only a compact [`CertifiedMergeLedgerReference`].  A
//! validator that does not yet have the referenced full entry asks one of the
//! exact merge-QC signers for it.  Transfer sessions are deliberately
//! in-memory: only a completely reassembled, canonical, reference-matching
//! entry may be handed to Kura's atomic pending-sidecar store.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::{BlockHeader, CertifiedMergeLedgerReference},
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

use crate::sumeragi::v2_core::{
    CanonicalIdentityProjection, IDENTITY_DOMAIN_PAYLOAD, IDENTITY_DOMAIN_PEER,
    IDENTITY_DOMAIN_PROCESS_LOCAL, IDENTITY_KIND_MERGE_ENTRY, IDENTITY_KIND_NETWORK_RESPONSE,
    IDENTITY_KIND_PEER, IDENTITY_KIND_REFERENCE_DIGEST, IDENTITY_KIND_REPLY_DELIVERY_ROUTE,
    IDENTITY_KIND_REPLY_PAYLOAD, IDENTITY_KIND_REPLY_SOURCE_KEY,
    IDENTITY_KIND_REPLY_WRITER_OCCURRENCE, IDENTITY_KIND_SIDECAR_CHUNK,
    IDENTITY_KIND_SIDECAR_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST, IDENTITY_KIND_SIDECAR_RESPONSE,
    IDENTITY_KIND_SIDECAR_SHARED_TRANSFER_STATE, IDENTITY_KIND_SIDECAR_SIBLING_STATE,
    IDENTITY_KIND_SIDECAR_TARGET_GATE_STATE, IDENTITY_KIND_SIDECAR_TARGET_OUTBOUND_STATE,
    ProductionReliableFlushApplicationProjection, ProductionReliableFlushTraceProjection,
    production_reliable_flush_application_refines_source_lane_kernel,
    production_reliable_flush_trace_refines_outbound_ownership_kernel,
    production_reliable_flush_two_phase_link_kernel,
};

/// Current certified merge-sidecar transfer protocol version.
pub const CERTIFIED_MERGE_SIDECAR_VERSION_V1: u8 = 1;
/// Maximum payload carried by one sidecar chunk.
pub const MAX_CERTIFIED_MERGE_CHUNK_BYTES: usize = 64 * 1024;
/// Maximum chunks required by a protocol-sized full entry.
pub const MAX_CERTIFIED_MERGE_CHUNKS: usize =
    MAX_MERGE_LEDGER_ENTRY_BYTES.div_ceil(MAX_CERTIFIED_MERGE_CHUNK_BYTES);

const REFERENCE_DIGEST_DOMAIN: &[u8] = b"iroha:merge:sidecar-reference:v1\0";
const REQUEST_ID_DOMAIN: &[u8] = b"iroha:merge:sidecar-request:v1\0";
const SIGNING_CONTEXT_DOMAIN: &[u8] = b"iroha:merge:signing-context:v1\0";

const MAX_INBOUND_SESSIONS: usize = 32;
const MAX_INBOUND_SESSIONS_PER_PEER: usize = 4;
const MAX_INBOUND_ASSEMBLY_BYTES: usize = 64 * 1024 * 1024;
const MAX_INBOUND_ASSEMBLY_BYTES_PER_PEER: usize = 32 * 1024 * 1024;
const RESERVED_DECIDED_INBOUND_SESSIONS: usize = 1;
const RESERVED_DECIDED_INBOUND_BYTES: usize = MAX_MERGE_LEDGER_ENTRY_BYTES;
const MAX_DEFERRED_BLOCKS: usize = 128;
const RESERVED_DECIDED_DEFERRED_BLOCKS: usize = 1;
const MAX_FUTURE_BLOCK_DISTANCE: u64 = 64;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(test)]
const DEFAULT_REPLY_SOURCE_CAPACITY: usize = 8;
const MAX_OUTBOUND_SESSIONS_PER_SOURCE: usize = 2;
const MAX_OUTBOUND_BYTES_PER_SOURCE: usize = 16 * 1024 * 1024;
const MAX_SERVER_REQUEST_GATES_PER_SOURCE: usize = 4;
const SERVER_REQUEST_GATE_TTL: Duration = Duration::from_secs(10);
const CHUNK_PAYLOAD_DIGEST_DOMAIN: &[u8] = b"iroha:merge-sidecar:chunk-payload:v1";
const RELIABLE_FLUSH_SIBLING_STATE_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-sibling-state:v1\0";
const RELIABLE_FLUSH_SHARED_TRANSFER_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-shared-transfer:v1\0";
const RELIABLE_FLUSH_TARGET_GATE_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-target-gate:v1\0";
const RELIABLE_FLUSH_TARGET_OUTBOUND_DIGEST_DOMAIN: &[u8] =
    b"iroha:merge-sidecar:reliable-flush-target-outbound:v1\0";

const SIGNING_GUARD_VERSION: u8 = 1;
const SIGNING_GUARD_DIR: &str = "merge-signing-guard-v1";
const SIGNING_GUARD_RECORD_EXT: &str = "norito";
const SIGNING_GUARD_TEMP_EXT: &str = "norito.tmp";
const SIGNING_GUARD_HIGH_WATER_FILE: &str = "committed-high-water.norito";
const SIGNING_GUARD_HIGH_WATER_TEMP: &str = "committed-high-water.norito.tmp";
const MAX_SIGNING_GUARD_RECORDS: usize = 4_096;
const MAX_SIGNING_GUARD_RECORD_BYTES: usize = 4 * 1024;

fn retry_timeout(base: Duration, attempts: u32) -> Duration {
    let backoff_shift = attempts.saturating_sub(1).min(4);
    base.saturating_mul(1_u32 << backoff_shift)
}

/// Point-to-point request for one exact certified merge sidecar.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarRequestV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Request nonce generated by the requester.
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

/// One fixed-boundary chunk of a certified merge sidecar response.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CertifiedMergeSidecarChunkV1 {
    /// Protocol version; must equal [`CERTIFIED_MERGE_SIDECAR_VERSION_V1`].
    pub version: u8,
    /// Request nonce copied verbatim from the request.
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
    /// Digest of the canonical priority, semantic target, and encoded response.
    pub(crate) canonical_request_digest: Hash,
    /// Exact encrypted-stream queue charge assigned by the network actor.
    pub(crate) stream_wire_bytes: usize,
    /// Semantic sidecar request nonce.
    pub(crate) request_id: Hash,
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
            canonical_request_digest: flush_identity.canonical_request_digest(),
            stream_wire_bytes: flush_identity.ticket_stream_wire_bytes(),
            request_id: chunk.request_id,
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
        if trace.status != 2
            || !production_reliable_flush_trace_refines_outbound_ownership_kernel(trace)
            || !production_reliable_flush_two_phase_link_kernel(trace, occurrence)
        {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "worker flush trace is not the accepted transition for this occurrence",
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
        self.projection.semantic_target == *route.semantic_target()
            && self.source_key == route.source_key()
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

type InboundSidecarKey = (HashOf<MergeLedgerEntry>, Hash);
type ServerRequestKey = (PeerId, Hash);
type OutboundAttemptKey = (ServerRequestKey, ServerRequestSource);

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
}

#[derive(Clone, Debug)]
struct ServerRequestGate {
    request_hash: HashOf<CertifiedMergeSidecarRequestV1>,
    source_capacity: Option<usize>,
    attempts: BTreeMap<ServerRequestSource, ServerRequestGateAttempt>,
}

#[derive(Clone, Debug)]
struct ServerRequestGateAttempt {
    reply_route: Option<NetworkReplyRoute>,
    materialization_authorized: bool,
    authorized_materialization_route: Option<NetworkReplyRoute>,
    /// A local lookup failed before immutable response bytes were installed.
    ///
    /// This keeps the bounded route/cursor history while allowing the exact
    /// authenticated delivery to retry the same terminating local work.
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
    /// Exact authenticated return route for response chunks.
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
    reply_source_capacity: usize,
    outbound_session_capacity: usize,
    outbound_byte_capacity: usize,
    server_request_gate_capacity: usize,
    inbound: BTreeMap<InboundSidecarKey, InboundAssembly>,
    inbound_cursor: Option<InboundSidecarKey>,
    outbound: BTreeMap<ServerRequestKey, OutboundTransfer>,
    /// Exact source-attempt ownership order. Every serviced incomplete attempt
    /// moves to the tail and every new source starts behind all current owners.
    outbound_order: VecDeque<OutboundAttemptKey>,
    tick_response_next: bool,
    server_request_gates: BTreeMap<ServerRequestKey, ServerRequestGate>,
    next_request_nonce: u64,
    boot_nonce: Hash,
}

impl MergeSidecarTransport {
    /// Construct an empty transport with the dependent-test source geometry.
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_reply_source_capacity(DEFAULT_REPLY_SOURCE_CAPACITY)
            .expect("default sidecar reply-source geometry is representable")
    }

    /// Construct an empty transport whose global corridors reserve every
    /// configured authenticated source's independent per-source limits.
    pub(crate) fn with_reply_source_capacity(
        reply_source_capacity: usize,
    ) -> Result<Self, MergeSidecarError> {
        if reply_source_capacity == 0 {
            return Err(MergeSidecarError::Capacity(
                "reply-source geometry must be non-zero",
            ));
        }
        let outbound_session_capacity = reply_source_capacity
            .checked_mul(MAX_OUTBOUND_SESSIONS_PER_SOURCE)
            .ok_or(MergeSidecarError::Capacity(
                "outbound response session geometry",
            ))?;
        let outbound_byte_capacity = reply_source_capacity
            .checked_mul(MAX_OUTBOUND_BYTES_PER_SOURCE)
            .ok_or(MergeSidecarError::Capacity(
                "outbound response byte geometry",
            ))?;
        let server_request_gate_capacity = reply_source_capacity
            .checked_mul(MAX_SERVER_REQUEST_GATES_PER_SOURCE)
            .ok_or(MergeSidecarError::Capacity("server request gate geometry"))?;
        let unix_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes();
        let process_id = std::process::id().to_le_bytes();
        Ok(Self {
            reply_source_capacity,
            outbound_session_capacity,
            outbound_byte_capacity,
            server_request_gate_capacity,
            inbound: BTreeMap::new(),
            inbound_cursor: None,
            outbound: BTreeMap::new(),
            outbound_order: VecDeque::new(),
            tick_response_next: true,
            server_request_gates: BTreeMap::new(),
            next_request_nonce: 0,
            boot_nonce: Hash::new_from_chunks(&[
                REQUEST_ID_DOMAIN,
                unix_nanos.as_slice(),
                process_id.as_slice(),
            ]),
        })
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

    fn request_id(&self, requester: &PeerId, key: InboundSidecarKey, nonce: u64) -> Hash {
        let (entry_hash, reference_digest) = key;
        Hash::new_from_chunks(&[
            REQUEST_ID_DOMAIN,
            self.boot_nonce.as_ref(),
            requester.to_string().as_bytes(),
            entry_hash.as_ref().as_ref(),
            reference_digest.as_ref(),
            &nonce.to_le_bytes(),
        ])
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
                < MAX_INBOUND_SESSIONS_PER_PEER
                && self
                    .inbound_peer_reserved_bytes(holder)
                    .saturating_add(requested_len)
                    <= MAX_INBOUND_ASSEMBLY_BYTES_PER_PEER;
            let priority_capacity = priority == InboundPriority::Decided
                || (self.ordinary_inbound_peer_session_count(holder)
                    < MAX_INBOUND_SESSIONS_PER_PEER - RESERVED_DECIDED_INBOUND_SESSIONS
                    && self
                        .ordinary_inbound_peer_reserved_bytes(holder)
                        .saturating_add(requested_len)
                        <= MAX_INBOUND_ASSEMBLY_BYTES_PER_PEER - RESERVED_DECIDED_INBOUND_BYTES);
            if holder == requester || !full_peer_capacity || !priority_capacity {
                None
            } else {
                Some((index, holder.clone()))
            }
        });
        let Some((holder_index, holder)) = selected else {
            return Ok(None);
        };
        self.next_request_nonce = self.next_request_nonce.wrapping_add(1);
        let request_id = self.request_id(requester, key, self.next_request_nonce);
        let assembly = self
            .inbound
            .get_mut(&key)
            .expect("assembly exists while beginning request");
        let previous_attempts = assembly.attempts;
        assembly.attempts = assembly.attempts.saturating_add(1);
        assembly.holder_cursor = (holder_index + 1) % holders.len();
        assembly.current = Some(RequestAttempt {
            id: request_id,
            holder: holder.clone(),
            last_progress_at: now,
            previous_holder_cursor: start_cursor,
            previous_attempts,
        });
        assembly.chunks.clear();
        assembly.received_bytes = 0;
        let reference = &assembly.reference;
        let request = CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id,
            entry_hash: key.0,
            encoded_len: reference.encoded_len,
            epoch_id: reference.epoch_id,
            reference_digest: key.1,
            requester: requester.clone(),
            responder: holder.clone(),
        };
        self.inbound_cursor = Some(key);
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
    pub(crate) fn release_unsent_request(&mut self, request: &CertifiedMergeSidecarRequestV1) {
        let key = (request.entry_hash, request.reference_digest);
        let Some(assembly) = self.inbound.get_mut(&key) else {
            return;
        };
        if !assembly.current.as_ref().is_some_and(|attempt| {
            attempt.id == request.request_id && attempt.holder == request.responder
        }) {
            return;
        }
        let attempt = assembly
            .current
            .take()
            .expect("matching request attempt was checked above");
        assembly.holder_cursor = attempt.previous_holder_cursor;
        assembly.attempts = attempt.previous_attempts;
        assembly.chunks.clear();
        assembly.received_bytes = 0;
        assembly.complete_pending_validation = false;
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
            || height > committed_height.saturating_add(MAX_FUTURE_BLOCK_DISTANCE)
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
            && (self.deferred_count() >= MAX_DEFERRED_BLOCKS
                || (priority == InboundPriority::Ordinary
                    && self.ordinary_deferred_count()
                        >= MAX_DEFERRED_BLOCKS - RESERVED_DECIDED_DEFERRED_BLOCKS))
        {
            return Err(MergeSidecarError::Capacity("deferred block count"));
        }
        if !self.inbound.contains_key(&key) {
            if self.inbound.len() >= MAX_INBOUND_SESSIONS
                || (priority == InboundPriority::Ordinary
                    && self.ordinary_inbound_session_count()
                        >= MAX_INBOUND_SESSIONS - RESERVED_DECIDED_INBOUND_SESSIONS)
            {
                return Err(MergeSidecarError::Capacity("inbound session count"));
            }
            let requested_len = usize::try_from(reference.encoded_len).unwrap_or(usize::MAX);
            if self.inbound_reserved_bytes().saturating_add(requested_len)
                > MAX_INBOUND_ASSEMBLY_BYTES
                || (priority == InboundPriority::Ordinary
                    && self
                        .ordinary_inbound_reserved_bytes()
                        .saturating_add(requested_len)
                        > MAX_INBOUND_ASSEMBLY_BYTES - RESERVED_DECIDED_INBOUND_BYTES)
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
        if new_global_bytes > MAX_INBOUND_ASSEMBLY_BYTES {
            return Err(MergeSidecarError::Capacity("global inbound bytes"));
        }
        let new_peer_bytes = self
            .inbound_peer_received_bytes(sender)
            .checked_add(chunk.bytes.len())
            .ok_or(MergeSidecarError::Capacity(
                "per-peer byte counter overflow",
            ))?;
        if new_peer_bytes > MAX_INBOUND_ASSEMBLY_BYTES_PER_PEER {
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
    ) -> (
        Vec<(HashOf<BlockHeader>, u64, u64)>,
        Option<MergeSidecarPost>,
    ) {
        let key = (entry_hash, reference_digest);
        if success {
            let deferred = self
                .inbound
                .remove(&key)
                .map(|assembly| {
                    assembly
                        .deferred
                        .into_values()
                        .map(|carrier| (carrier.hash, carrier.height, carrier.view))
                        .collect()
                })
                .unwrap_or_default();
            return (deferred, None);
        }
        if let Some(assembly) = self.inbound.get_mut(&key) {
            assembly.current = None;
            assembly.chunks.clear();
            assembly.received_bytes = 0;
            assembly.complete_pending_validation = false;
        }
        let request = self.begin_request(key, requester, now).ok().flatten();
        (Vec::new(), request)
    }

    /// Drop an invalid exact reference and return all affected carrier blocks.
    pub(crate) fn discard_invalid(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
    ) -> Vec<(HashOf<BlockHeader>, u64, u64)> {
        let keys = self
            .inbound
            .keys()
            .filter(|key| key.0 == entry_hash)
            .copied()
            .collect::<Vec<_>>();
        keys.into_iter()
            .filter_map(|key| self.inbound.remove(&key))
            .flat_map(|assembly| {
                assembly
                    .deferred
                    .into_values()
                    .map(|carrier| (carrier.hash, carrier.height, carrier.view))
            })
            .collect()
    }

    fn retire_inactive_outbound_attempts(&mut self, now: Instant) {
        let retired = self
            .outbound
            .iter()
            .flat_map(|(key, transfer)| {
                transfer.attempts.iter().filter_map(|(source, attempt)| {
                    attempt
                        .reply_route
                        .as_ref()
                        .is_some_and(|route| !route.is_active())
                        .then(|| {
                            (
                                key.clone(),
                                source.clone(),
                                attempt.in_flight_chunk.unwrap_or(attempt.next_chunk),
                            )
                        })
                })
            })
            .collect::<Vec<_>>();
        for (key, source, resume_chunk) in retired {
            if let Some(gate_attempt) = self
                .server_request_gates
                .get_mut(&key)
                .and_then(|gate| gate.attempts.get_mut(&source))
            {
                gate_attempt.cursor = ServerResponseCursor::Pending(resume_chunk);
                gate_attempt.inserted = now;
            }
            if let Some(transfer) = self.outbound.get_mut(&key) {
                transfer.attempts.remove(&source);
                if transfer.attempts.is_empty() {
                    self.outbound.remove(&key);
                }
            }
        }
        self.outbound_order.retain(|(key, source)| {
            self.outbound
                .get(key)
                .and_then(|transfer| transfer.attempts.get(source))
                .is_some_and(|attempt| attempt.queued)
        });
    }

    fn prune_server_gates(&mut self, now: Instant) {
        self.retire_inactive_outbound_attempts(now);
        let outbound = &self.outbound;
        self.server_request_gates.retain(|key, gate| {
            // A pending cursor is the source's bounded progress reservation,
            // including while its tenure is inactive and shared bytes have
            // been released. Only terminal no-outbound tombstones age out.
            gate.attempts.retain(|source, attempt| {
                outbound
                    .get(key)
                    .is_some_and(|transfer| transfer.attempts.contains_key(source))
                    || attempt.cursor != ServerResponseCursor::Complete
                    || now.saturating_duration_since(attempt.inserted) <= SERVER_REQUEST_GATE_TTL
            });
            !gate.attempts.is_empty()
        });
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
            .filter(|gate| gate.attempts.contains_key(source))
            .count()
    }

    fn server_gate_attempt_count(&self) -> usize {
        self.server_request_gates
            .values()
            .map(|gate| gate.attempts.len())
            .sum()
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
            .filter(|transfer| transfer.attempts.contains_key(source))
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
            .filter(|transfer| transfer.attempts.contains_key(source))
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
            Some(candidate) => gate.attempts.values().all(|attempt| {
                attempt.reply_route.as_ref().is_some_and(|prior| {
                    candidate.same_request_authority(prior)
                        && !candidate.equal_ordinal_different_tenure(prior)
                })
            }),
            None => gate
                .attempts
                .values()
                .all(|attempt| attempt.reply_route.is_none()),
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
            && self.source_outbound_count(source) < MAX_OUTBOUND_SESSIONS_PER_SOURCE
            && self.source_outbound_bytes(source).saturating_add(bytes)
                <= MAX_OUTBOUND_BYTES_PER_SOURCE
    }

    /// Rate-limit authenticated requests before any potentially expensive Kura lookup.
    ///
    /// Returns `true` when the caller must materialize the canonical response
    /// from Kura. `false` means that the semantic request already owns local
    /// work or immutable response bytes; the observed source was attached.
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
    ) -> Result<bool, MergeSidecarError> {
        self.prune_server_gates(now);
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
        let key = (sender.clone(), request.request_id);
        let request_hash = HashOf::new(request);
        let source = Self::server_request_source(sender, reply_route);
        let source_capacity = Self::route_source_capacity(reply_route)?;
        if source_capacity.is_some_and(|capacity| capacity != self.reply_source_capacity) {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
        if let Some(existing) = self.server_request_gates.get(&key).cloned() {
            if existing.request_hash != request_hash {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if existing.source_capacity != source_capacity {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if self
                .outbound
                .get(&key)
                .is_some_and(|transfer| &transfer.request != request)
            {
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
                    return Ok(false);
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
                    return Ok(false);
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
                    return Ok(false);
                }

                if prior.materialization_authorized {
                    if update != NetworkReplyRouteSourceUpdate::Exact {
                        let gate = self
                            .server_request_gates
                            .get_mut(&key)
                            .expect("existing server gate remains present");
                        let attempt = gate
                            .attempts
                            .get_mut(&source)
                            .expect("existing source gate remains present");
                        attempt.reply_route = reply_route.cloned();
                        attempt.inserted = now;
                    }
                    return Ok(false);
                }
                if update == NetworkReplyRouteSourceUpdate::Exact {
                    if !prior.materialization_retryable {
                        return Err(MergeSidecarError::UnsolicitedResponse);
                    }
                    let materialization_in_progress = existing
                        .attempts
                        .values()
                        .any(|attempt| attempt.materialization_authorized);
                    let gate = self
                        .server_request_gates
                        .get_mut(&key)
                        .expect("existing server gate remains present");
                    let attempt = gate
                        .attempts
                        .get_mut(&source)
                        .expect("existing source gate remains present");
                    attempt.materialization_authorized = true;
                    attempt.authorized_materialization_route = reply_route.cloned();
                    attempt.materialization_retryable = false;
                    attempt.inserted = now;
                    return Ok(!materialization_in_progress);
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
                attempt.materialization_authorized = true;
                attempt.authorized_materialization_route = reply_route.cloned();
                attempt.materialization_retryable = false;
                attempt.inserted = now;
                return Ok(true);
            }

            if !Self::alternate_source_is_authorized(&existing, reply_route) {
                return Err(MergeSidecarError::UnsolicitedResponse);
            }
            if source_capacity.is_some_and(|capacity| existing.attempts.len() >= capacity)
                || self.server_gate_attempt_count() >= self.server_request_gate_capacity
                || self.source_gate_count(&source) >= MAX_SERVER_REQUEST_GATES_PER_SOURCE
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
                return Ok(false);
            }

            let materialization_in_progress = existing
                .attempts
                .values()
                .any(|attempt| attempt.materialization_authorized);
            self.server_request_gates
                .get_mut(&key)
                .expect("existing server gate remains present")
                .attempts
                .insert(
                    source,
                    ServerRequestGateAttempt {
                        reply_route: reply_route.cloned(),
                        materialization_authorized: true,
                        authorized_materialization_route: reply_route.cloned(),
                        materialization_retryable: false,
                        cursor: ServerResponseCursor::Pending(0),
                        pending_flush_chunk: None,
                        inserted: now,
                    },
                );
            return Ok(!materialization_in_progress);
        }
        let source_count = self.source_gate_count(&source);
        if self.server_gate_attempt_count() >= self.server_request_gate_capacity
            || source_count >= MAX_SERVER_REQUEST_GATES_PER_SOURCE
        {
            return Err(MergeSidecarError::Capacity("server request rate gate"));
        }
        self.server_request_gates.insert(
            key,
            ServerRequestGate {
                request_hash,
                source_capacity,
                attempts: BTreeMap::from([(
                    source,
                    ServerRequestGateAttempt {
                        reply_route: reply_route.cloned(),
                        materialization_authorized: true,
                        authorized_materialization_route: reply_route.cloned(),
                        materialization_retryable: false,
                        cursor: ServerResponseCursor::Pending(0),
                        pending_flush_chunk: None,
                        inserted: now,
                    },
                )]),
            },
        );
        Ok(true)
    }

    /// Cancel a rate-gate reservation which never became an outbound transfer.
    ///
    /// Materialization performs Kura lookup and exact metadata validation only
    /// after bounded admission. Every failure on that path must release its
    /// authorization so a later authenticated delivery is not suppressed
    /// without a reply. A parked attempt may remain to preserve its bounded
    /// route history and source-local non-regressing chunk cursor.
    pub(crate) fn cancel_unmaterialized_server_request(
        &mut self,
        sender: &PeerId,
        request: &CertifiedMergeSidecarRequestV1,
    ) {
        let key = (sender.clone(), request.request_id);
        if self.outbound.contains_key(&key) {
            return;
        }
        let request_hash = HashOf::new(request);
        if let Some(gate) = self
            .server_request_gates
            .get_mut(&key)
            .filter(|gate| gate.request_hash == request_hash)
        {
            for attempt in gate
                .attempts
                .values_mut()
                .filter(|attempt| attempt.materialization_authorized)
            {
                attempt.materialization_authorized = false;
                attempt.authorized_materialization_route = None;
                attempt.materialization_retryable = true;
            }
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
    /// After the bounded duplicate window expires, an exact live delivery may authorize
    /// another durable lookup, so local admission never becomes height-long
    /// deduplication.
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
        if gate.request_hash != HashOf::new(&request)
            || !gate_attempt.materialization_authorized
            || !same_route
        {
            return Err(MergeSidecarError::UnsolicitedResponse);
        }
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
            if !attempt.materialization_authorized {
                continue;
            }
            let ServerResponseCursor::Pending(resume_chunk) = attempt.cursor else {
                continue;
            };
            if attempt
                .reply_route
                .as_ref()
                .is_some_and(|route| !route.is_active())
            {
                continue;
            }
            if remaining_global_sessions == 0
                || self.source_outbound_count(source) >= MAX_OUTBOUND_SESSIONS_PER_SOURCE
                || self
                    .source_outbound_bytes(source)
                    .saturating_add(response_len)
                    > MAX_OUTBOUND_BYTES_PER_SOURCE
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
            Self::park_authorized_server_request_attempts(gate, now);
            // A successful old-writer receipt may have completed the source
            // represented by this exact authorization while terminating local
            // materialization was in flight. That callback is a consumed
            // no-op. A still-pending authorization with no live route instead
            // lost its delivery authority and must fail closed after releasing
            // every response reservation.
            return if exact_attempt_completed {
                Ok(())
            } else {
                Err(MergeSidecarError::UnsolicitedResponse)
            };
        }
        if admitted_attempts.is_empty()
            || self.global_outbound_bytes().saturating_add(response_len)
                > self.outbound_byte_capacity
        {
            let gate = self
                .server_request_gates
                .get_mut(&key)
                .expect("validated server request gate remains present");
            Self::park_authorized_server_request_attempts(gate, now);
            return Err(MergeSidecarError::Capacity("outbound response budget"));
        }
        let gate = self
            .server_request_gates
            .get_mut(&key)
            .expect("validated server request gate remains present");
        // Shared materialization satisfied the semantic lookup, but a
        // partitioned source may not have acquired an outbound session. Retain
        // every pending source's bounded route history and source-local cursor
        // so a later delivery can attach to the shared bytes (or rematerialize
        // after they leave) without restarting. A completed source remains
        // terminal across connection tenures while this semantic gate exists.
        Self::park_authorized_server_request_attempts(gate, now);
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

    /// Emit at most `limit` response chunks in deterministic session order.
    ///
    /// The owned queue, rather than an index into a rebuilt map snapshot, makes
    /// each source attempt's service rank decrease even when another source
    /// completes or experiences backpressure. Reconnect retries only that
    /// source's retained current chunk.
    pub(crate) fn drain_outbound_chunks(
        &mut self,
        limit: usize,
        now: Instant,
    ) -> Vec<MergeSidecarPost> {
        let mut posts = Vec::new();
        while posts.len() < limit {
            let Some((key, source)) = self.outbound_order.pop_front() else {
                break;
            };
            let mut completed = false;
            let mut retired = false;
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
                if attempt
                    .reply_route
                    .as_ref()
                    .is_some_and(|route| !route.is_active())
                {
                    retired = true;
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
                            retired = true;
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
                gate_attempt.cursor = cursor;
                if completed {
                    gate_attempt.pending_flush_chunk = None;
                } else if let Some(identity) = emitted_chunk_identity {
                    gate_attempt.pending_flush_chunk = Some(identity);
                }
                if retired || completed {
                    gate_attempt.inserted = now;
                }
            }
            if retired || completed {
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
        posts
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
        if !production_reliable_flush_trace_refines_outbound_ownership_kernel(worker_trace)
            || !production_reliable_flush_two_phase_link_kernel(worker_trace, occurrence)
        {
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "accepted worker transition differs from the lane occurrence",
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

        // This is the only linearization point. Every fallible identity,
        // cursor, route, shared-state, and scalar check completed above.
        if !admission.flush_identity.claim_writer_flush_once() {
            return Ok(false);
        }
        apply_reliable_flush_application(self, &plan, now);
        let observation = observe_reliable_flush_application(self, &plan);
        let application = reliable_flush_application_projection(&plan, &observation, now);
        if !production_reliable_flush_application_refines_source_lane_kernel(application) {
            // The production caller holds `ConsensusFailStopOperation`; this
            // internal post-CAS invariant error drops that incomplete guard,
            // permanently closes exact output, and requires process restart.
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "writer flush application violated the source-lane refinement",
            ));
        }
        if !production_reliable_flush_two_phase_link_kernel(worker_trace, application) {
            // As above, a post-CAS disagreement is fail-stop. The pre-CAS
            // occurrence check makes this branch an internal projection bug,
            // never a recoverable user or network error.
            return Err(MergeSidecarError::FlushIdentityMismatch(
                "writer flush application disconnected from its accepted worker transition",
            ));
        }
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
    ) {
        for assembly in self.inbound.values_mut() {
            assembly.deferred.retain(|hash, carrier| {
                carrier.height > committed_height && pending_blocks.contains(hash)
            });
        }
        self.inbound
            .retain(|_, assembly| !assembly.deferred.is_empty());
    }

    /// Rotate stalled holders and emit bounded, indefinitely retried requests.
    pub(crate) fn tick_bounded(
        &mut self,
        requester: &PeerId,
        now: Instant,
        limit: usize,
    ) -> Vec<MergeSidecarPost> {
        self.prune_server_gates(now);
        let timed_out: Vec<_> = self
            .inbound
            .iter()
            .filter(|(_, assembly)| {
                assembly.current.as_ref().is_some_and(|attempt| {
                    now.saturating_duration_since(attempt.last_progress_at)
                        >= retry_timeout(REQUEST_TIMEOUT, assembly.attempts)
                })
            })
            .map(|(hash, _)| *hash)
            .collect();
        for hash in &timed_out {
            if let Some(assembly) = self.inbound.get_mut(hash) {
                assembly.current = None;
                assembly.chunks.clear();
                assembly.received_bytes = 0;
                assembly.complete_pending_validation = false;
            }
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
        let mut posts = Vec::new();
        while posts.len() < limit {
            let request_ready = !idle.is_empty();
            let response_ready = !self.outbound.is_empty();
            if !request_ready && !response_ready {
                break;
            }
            let contended = request_ready && response_ready;
            let response_first = response_ready && (!request_ready || self.tick_response_next);
            let mut emitted = false;

            if response_first {
                if let Some(post) = self.drain_outbound_chunks(1, now).pop() {
                    posts.push(post);
                    emitted = true;
                    if contended {
                        self.tick_response_next = false;
                    }
                }
            } else {
                while let Some(hash) = idle.pop_front() {
                    if let Ok(Some(post)) = self.begin_request(hash, requester, now) {
                        posts.push(post);
                        emitted = true;
                        if contended {
                            self.tick_response_next = true;
                        }
                        break;
                    }
                }
            }

            // A nominally ready class can become ineligible while bounded
            // per-peer reservations are inspected. Preserve useful capacity
            // by trying the other class without advancing its fairness turn.
            if !emitted && response_first {
                while let Some(hash) = idle.pop_front() {
                    if let Ok(Some(post)) = self.begin_request(hash, requester, now) {
                        posts.push(post);
                        emitted = true;
                        break;
                    }
                }
            } else if !emitted && let Some(post) = self.drain_outbound_chunks(1, now).pop() {
                posts.push(post);
                emitted = true;
            }

            if !emitted {
                break;
            }
        }
        posts
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
struct MergeSigningGuardRecordV1 {
    version: u8,
    context: MergeSigningContextV1,
    message_digest: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
struct MergeSigningHighWaterV1 {
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
}

impl MergeSigningGuard {
    /// Open the guard under the Kura root and fail closed on malformed records.
    #[cfg(test)]
    pub(crate) fn open(store_root: &Path) -> Result<Self, MergeSidecarError> {
        Self::open_with_committed_frontier(store_root, 0, 0)
    }

    /// Open and reconcile the guard against the exact latest globally ordered
    /// merge epoch recovered from canonical Kura/state.
    #[cfg(test)]
    pub(crate) fn open_with_committed_epoch(
        store_root: &Path,
        committed_epoch: u64,
    ) -> Result<Self, MergeSidecarError> {
        Self::open_with_committed_frontier(store_root, committed_epoch, 0)
    }

    /// Open against the exact globally finalized merge epoch and carrier height.
    pub(crate) fn open_with_committed_frontier(
        store_root: &Path,
        committed_epoch: u64,
        committed_carrier_height: u64,
    ) -> Result<Self, MergeSidecarError> {
        let directory = store_root.join(SIGNING_GUARD_DIR);
        ensure_regular_directory(&directory)?;
        Self::reconcile_temps(&directory)?;
        let durable_high_water =
            Self::read_high_water(&directory)?.unwrap_or(MergeSigningHighWaterV1 {
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
        };
        guard.validate_all()?;
        guard.advance_committed_frontier(committed_epoch, committed_carrier_height)?;
        Ok(guard)
    }

    fn high_water_path(directory: &Path) -> PathBuf {
        directory.join(SIGNING_GUARD_HIGH_WATER_FILE)
    }

    fn high_water_temp_path(directory: &Path) -> PathBuf {
        directory.join(SIGNING_GUARD_HIGH_WATER_TEMP)
    }

    fn decode_high_water(path: &Path) -> Result<MergeSigningHighWaterV1, MergeSidecarError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > MAX_SIGNING_GUARD_RECORD_BYTES as u64
        {
            return Err(MergeSidecarError::SigningGuard(format!(
                "unsafe signing-guard high-water file {}",
                path.display()
            )));
        }
        let bytes =
            fs::read(path).map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let high_water = norito::decode_from_bytes::<MergeSigningHighWaterV1>(&bytes)
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
    ) -> Result<Option<MergeSigningHighWaterV1>, MergeSidecarError> {
        let path = Self::high_water_path(directory);
        if !path.exists() {
            return Ok(None);
        }
        Self::decode_high_water(&path).map(Some)
    }

    fn reconcile_temps(directory: &Path) -> Result<(), MergeSidecarError> {
        let high_water_temp = Self::high_water_temp_path(directory);
        if high_water_temp.exists() {
            let metadata = fs::symlink_metadata(&high_water_temp)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || metadata.len() > MAX_SIGNING_GUARD_RECORD_BYTES as u64
            {
                return Err(MergeSidecarError::SigningGuard(
                    "unsafe signing-guard high-water temp".to_owned(),
                ));
            }
            // The canonical committed epoch supplied by Kura/state is the
            // authority on restart. A temp may be partial at any pre-rename
            // crash boundary, so it is always safe to discard before
            // re-publishing the canonical high-water.
            fs::remove_file(&high_water_temp)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        }

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
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
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

    fn read_record(path: &Path) -> Result<MergeSigningGuardRecordV1, MergeSidecarError> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > MAX_SIGNING_GUARD_RECORD_BYTES as u64
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
        let record = norito::decode_from_bytes::<MergeSigningGuardRecordV1>(&bytes)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        let canonical = norito::to_bytes(&record)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if canonical != bytes || record.version != SIGNING_GUARD_VERSION {
            return Err(MergeSidecarError::SigningGuard(
                "non-canonical or unsupported signing-guard record".to_owned(),
            ));
        }
        Ok(record)
    }

    fn validate_all(&self) -> Result<(), MergeSidecarError> {
        let mut count = 0_usize;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let path = item.path();
            let name = item.file_name();
            let name = name.to_string_lossy();
            if name == SIGNING_GUARD_HIGH_WATER_FILE {
                let high_water = Self::decode_high_water(&path)?;
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
            count = count.saturating_add(1);
            if count > MAX_SIGNING_GUARD_RECORDS {
                return Err(MergeSidecarError::SigningGuard(
                    "signing-guard record count exceeds hard limit".to_owned(),
                ));
            }
            let record = Self::read_record(&path)?;
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
            let record = MergeSigningHighWaterV1 {
                version: SIGNING_GUARD_VERSION,
                committed_epoch,
                committed_carrier_height,
            };
            let bytes = norito::to_bytes(&record)
                .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            let temp = Self::high_water_temp_path(&self.directory);
            if let Err(error) = fs::remove_file(&temp)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                return Err(MergeSidecarError::SigningGuard(error.to_string()));
            }
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
            let record = Self::read_record(&path)?;
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
        let record = Self::read_record(&path)?;
        if &record.context != context {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record context/path mismatch".to_owned(),
            ));
        }
        Ok(Some(record.message_digest))
    }

    /// Durably authorize one digest, returning an error for a conflicting digest.
    pub(crate) fn authorize(
        &self,
        context: MergeSigningContextV1,
        message_digest: Hash,
    ) -> Result<(), MergeSidecarError> {
        if context.epoch_id <= self.committed_epoch
            || context.carrier_height <= self.committed_carrier_height
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let path = self.record_path(&context);
        if path.exists() {
            let existing = Self::read_record(&path)?;
            return if existing.context == context && existing.message_digest == message_digest {
                Ok(())
            } else {
                Err(MergeSidecarError::LocalSigningEquivocation)
            };
        }
        let mut count = 0_usize;
        for item in fs::read_dir(&self.directory)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?
        {
            let item = item.map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
            if item
                .file_name()
                .to_string_lossy()
                .ends_with(&format!(".{SIGNING_GUARD_RECORD_EXT}"))
            {
                count = count.saturating_add(1);
                if count >= MAX_SIGNING_GUARD_RECORDS {
                    break;
                }
            }
        }
        if count >= MAX_SIGNING_GUARD_RECORDS {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record count reached hard limit".to_owned(),
            ));
        }
        let record = MergeSigningGuardRecordV1 {
            version: SIGNING_GUARD_VERSION,
            context,
            message_digest,
        };
        let bytes = norito::to_bytes(&record)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if bytes.len() > MAX_SIGNING_GUARD_RECORD_BYTES {
            return Err(MergeSidecarError::SigningGuard(
                "signing-guard record exceeds hard byte limit".to_owned(),
            ));
        }
        let temp = path.with_extension("norito.tmp");
        if let Err(error) = fs::remove_file(&temp)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(MergeSidecarError::SigningGuard(error.to_string()));
        }
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
            let existing = Self::read_record(&path)?;
            return if existing.message_digest == record.message_digest
                && existing.context == record.context
            {
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

    fn peer(label: &[u8]) -> PeerId {
        PeerId::new(
            KeyPair::try_from_seed(label.to_vec(), Algorithm::BlsNormal)
                .expect("derive test key")
                .public_key()
                .clone(),
        )
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
        let (mut flush_control, flush_ack) =
            NetworkReplyFlushAckTestFixture::for_reply(&canonical_post, route);
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
        request_label: &[u8],
        encoded_len: usize,
    ) -> CertifiedMergeSidecarRequestV1 {
        let mut request = base.clone();
        request.requester = requester;
        request.request_id = Hash::new(request_label);
        request.encoded_len = encoded_len as u64;
        request
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
        let (deferred, retry) = transport.finish_completed(
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
            true,
            &requester,
            now,
        );
        assert_eq!(deferred.len(), 1);
        assert!(retry.is_none());
        assert_eq!(transport.inbound_len(), 0);
    }

    #[test]
    fn timeout_rotates_to_another_qc_holder() {
        let (mut transport, requester, reference, first, now) = start_session(1, 3);
        let posts = transport.tick_bounded(&requester, now + REQUEST_TIMEOUT, usize::MAX);
        let next = posts
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Chunk(_) => None,
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
            transport.retain_pending_blocks(&pending_blocks, 1);
            request = transport
                .tick_bounded(&requester, now, usize::MAX)
                .into_iter()
                .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                    CertifiedMergeSidecarMessage::Request(request) => Some(request),
                    CertifiedMergeSidecarMessage::Chunk(_) => None,
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
        let (deferred, retry) = transport.finish_completed(
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
            true,
            &requester,
            now,
        );
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

        transport.retain_pending_blocks(&BTreeSet::from([replacement]), 1);
        let responder = request.responder.clone();
        assert!(matches!(
            transport
                .ingest_chunk(&responder, chunks(&request, &[1]).remove(0), now)
                .expect("complete retained fetch"),
            ChunkIngestOutcome::Complete(_)
        ));
        let (deferred, _) = transport.finish_completed(
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
            true,
            &requester,
            now,
        );
        assert_eq!(deferred, vec![(replacement, 2, 1)]);
        assert!(!deferred.iter().any(|(hash, _, _)| *hash == original));

        let (mut transport, _, _, _, _) = start_session(1, 2);
        transport.retain_pending_blocks(&BTreeSet::from([original]), 2);
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit first exact request")
        );
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

        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("deduplicate the exact active delivery")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit request through source A")
        );
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

        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("attach independent source B to shared bytes")
        );
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
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&reconnected_a), &local_peer, now,)
                .expect("reattach source A at its retained source cursor")
        );
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
            assert_eq!(
                server
                    .admit_server_request(&requester, &request, Some(route), &local_peer, now,)
                    .expect("admit one independent authenticated source"),
                index == 0,
                "only the first source authorizes shared materialization"
            );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit one authenticated source")
        );
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts exact shared materialization")
        );
        server
            .enqueue_response(request.clone(), Some(route_a.clone()), vec![0xA6; len], now)
            .expect("materialize one shared two-chunk response");
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("source B attaches its independent cursor to shared bytes")
        );
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
        for (label, disconnected) in [
            ("source owner", &disconnected_source_owner),
            ("delivery route", &disconnected_delivery_route),
            ("writer occurrence", &disconnected_writer_occurrence),
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
    fn equal_ordinal_different_tenure_alternate_source_is_rejected_atomically() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub_a = peer(b"ordinal collision hub a");
        let hub_b = peer(b"ordinal collision hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit the original authenticated source")
        );
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

        let forged = routes
            .forge_equal_ordinal_different_tenure(&route_a, requester.clone(), hub_b)
            .expect("forge an actor-global ordinal collision for the adversarial test");
        assert!(route_a.equal_ordinal_different_tenure(&forged));
        let forged_source = ServerRequestSource::Authenticated(forged.source_key());
        assert_ne!(source_a, forged_source);

        let key = (requester.clone(), request.request_id);
        let gate_attempts_before = server.server_gate_attempt_count();
        let outbound_attempts_before = server.outbound_attempt_count();
        let outbound_bytes_before = server.global_outbound_bytes();
        let outbound_order_before = server.outbound_order.len();
        assert!(matches!(
            server.admit_server_request(&requester, &request, Some(&forged), &local_peer, now,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));

        assert_eq!(server.server_gate_attempt_count(), gate_attempts_before);
        assert_eq!(server.outbound_attempt_count(), outbound_attempts_before);
        assert_eq!(server.global_outbound_bytes(), outbound_bytes_before);
        assert_eq!(server.outbound_order.len(), outbound_order_before);
        let gate = &server.server_request_gates[&key];
        assert_eq!(gate.attempts.len(), 1);
        assert!(!gate.attempts.contains_key(&forged_source));
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
        assert!(!transfer.attempts.contains_key(&forged_source));
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
    fn inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let hub = peer(b"source teardown hub");
        let mut routes = NetworkReplyRouteTestFixture::new(hub);
        let route = routes.mint(requester.clone());
        let source = ServerRequestSource::Authenticated(route.source_key());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit initial source")
        );
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
        assert!(server.tick_bounded(&local_peer, now, 0).is_empty());
        assert!(server.outbound.is_empty());
        assert!(server.outbound_order.is_empty());
        assert_eq!(server.source_outbound_count(&source), 0);
        assert_eq!(server.source_outbound_bytes(&source), 0);

        let reconnect_at = now + SERVER_REQUEST_GATE_TTL + Duration::from_nanos(1);
        let reconnected = routes.mint(requester.clone());
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&reconnected),
                    &local_peer,
                    reconnect_at,
                )
                .expect("delayed reconnect rematerializes bytes at the retained cursor")
        );
        server
            .enqueue_response(
                request,
                Some(reconnected.clone()),
                vec![0xA4; len],
                reconnect_at,
            )
            .expect("queue rematerialized response at the retained cursor");
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
    }

    #[test]
    fn later_delivery_preserves_the_current_source_cursor() {
        let len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let (_, requester, _, request, now) = start_session(len, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"later delivery hub"));
        let first_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now)
                .expect("admit first delivery")
        );
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
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("update only this source delivery")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now)
                .expect("admit first delivery")
        );
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
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("rebind only this source delivery")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now)
                .expect("admit old tenure")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&reconnected), &local_peer, now)
                .expect("reauthorize materialization at the retained cursor")
        );
        server
            .enqueue_response(request, Some(reconnected.clone()), vec![0xC7; len], now)
            .expect("queue rematerialized bytes for the new tenure");
        let new_one = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("new tenure retries the retained current chunk");
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
            assert!(
                server
                    .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now,)
                    .expect("admit the overlapping old tenure")
            );
            server
                .enqueue_response(request.clone(), Some(old_route.clone()), vec![0xC8], now)
                .expect("materialize the overlapping response");
            let old_post = server
                .drain_outbound_chunks(1, now)
                .pop()
                .expect("hand the old current item to exact output");
            let old_receipt = reply_chunk_admission(&old_post);
            let reconnected = routes.mint(requester.clone());
            assert!(
                !server
                    .admit_server_request(
                        &requester,
                        &request,
                        Some(&reconnected),
                        &local_peer,
                        now,
                    )
                    .expect("overlapping reconnect requeues the retained current item")
            );
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
            assert!(
                server
                    .admit_server_request(&requester, &request, Some(&old_route), &local_peer, now,)
                    .expect("admit the prune-race source")
            );
            server
                .enqueue_response(request.clone(), Some(old_route.clone()), vec![0xD1], now)
                .expect("materialize the prune-race response");
            let old_post = server
                .drain_outbound_chunks(1, now)
                .pop()
                .expect("hand the final old-tenure chunk to exact output");
            let old_receipt = reply_chunk_admission(&old_post);
            assert!(routes.retire(&old_route));
            assert!(server.tick_bounded(&local_peer, now, 0).is_empty());
            assert!(server.outbound.is_empty());
            assert_eq!(
                server.server_request_gates[&key].attempts[&source].cursor,
                ServerResponseCursor::Pending(0)
            );
            assert!(
                server
                    .acknowledge_outbound_chunk(&old_receipt, now)
                    .expect("the byte-free gate accepts the old successful flush after pruning")
            );
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
            assert!(
                !server
                    .admit_server_request(
                        &requester,
                        &request,
                        Some(&reconnected),
                        &local_peer,
                        now,
                    )
                    .expect("the completed source remains terminal after reconnect")
            );
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
                assert!(
                    server
                        .admit_server_request(requester, request, Some(route), &local_peer, now,)
                        .expect("admit an independent rematerialization source")
                );
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
            assert!(server.tick_bounded(&local_peer, now, 0).is_empty());
            assert!(!server.outbound.contains_key(&key_a));
            assert!(server.outbound.contains_key(&key_b));
            let reconnected_a = routes.mint(requester_a.clone());
            assert!(
                server
                    .admit_server_request(
                        &requester_a,
                        &request_a,
                        Some(&reconnected_a),
                        &local_peer,
                        now,
                    )
                    .expect("reconnect authorizes terminating rematerialization")
            );
            assert!(matches!(
                server.enqueue_response(
                    request_a.clone(),
                    Some(reconnected_a.clone()),
                    vec![0xFF],
                    now,
                ),
                Err(MergeSidecarError::FlushIdentityMismatch(_))
            ));
            assert!(
                server
                    .acknowledge_outbound_chunk(&old_a_receipt, now)
                    .expect("old flush wins before rematerialization or reconnect redrain")
            );
            assert!(
                !server
                    .acknowledge_outbound_chunk(&old_a_receipt, now)
                    .expect("the pre-rematerialization receipt advances exactly once")
            );
            server
                .enqueue_response(request_a, Some(reconnected_a), vec![0xE1], now)
                .expect("the completed in-flight materialization callback is a benign no-op");
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
    fn later_delivery_updates_pending_work_without_losing_materialized_output() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"pending delivery hub"));
        let admitted_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&admitted_route),
                    &local_peer,
                    now,
                )
                .expect("start one semantic materialization")
        );

        let later_route = routes
            .redeliver(&admitted_route)
            .expect("mint later delivery while local work is pending");
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now)
                .expect("coalesce the later delivery into pending work")
        );
        server
            .enqueue_response(request, Some(admitted_route), vec![0x7A], now)
            .expect("the original work authorization remains consumable");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("materialized output keeps the later same-source delivery route");
        assert!(matches!(
            &post,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_delivery(&later_route)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0 && chunk.bytes.as_slice() == [0x7A])
        ));
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn reconnect_during_materialization_keeps_old_authorization_but_emits_new_tenure() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"materialization reconnect hub"));
        let admitted_route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&admitted_route),
                    &local_peer,
                    now,
                )
                .expect("authorize one immutable materialization")
        );
        assert!(routes.retire(&admitted_route));
        let reconnected = routes.mint(requester.clone());
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&reconnected), &local_peer, now)
                .expect("new tenure reuses the already-running materialization")
        );
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
                .is_some_and(|route| route.same_delivery(&reconnected))
        );
        server
            .enqueue_response(request, Some(admitted_route), vec![0x6C], now)
            .expect("the original authorization may finish without a second Kura lookup");
        let post = server
            .drain_outbound_chunks(1, now)
            .pop()
            .expect("finished bytes emit only on the reconnected tenure");
        assert!(matches!(
            &post,
            MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            } if route.same_tenure(&reconnected)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 0 && chunk.bytes.as_slice() == [0x6C])
        ));
        assert!(acknowledge_reply_chunk(&mut server, &post, now));
    }

    #[test]
    fn conflicting_server_request_id_reuse_is_rejected_before_materialization() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("admit first exact request")
        );
        let mut conflicting = request;
        conflicting.reference_digest = Hash::new(b"conflicting sidecar reference");
        assert!(matches!(
            server.admit_server_request(&requester, &conflicting, None, &local_peer, now,),
            Err(MergeSidecarError::UnsolicitedResponse)
        ));
        assert_eq!(server.server_request_gates.len(), 1);
    }

    #[test]
    fn failed_materialization_releases_rate_gate_for_exact_retry() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("reserve the request before durable lookup")
        );
        server.cancel_unmaterialized_server_request(&requester, &request);
        let parked = server
            .server_request_gates
            .values()
            .next()
            .and_then(|gate| gate.attempts.values().next())
            .expect("failed lookup retains one bounded retryable attempt");
        assert!(!parked.materialization_authorized);
        assert!(parked.materialization_retryable);
        assert!(
            server
                .admit_server_request(&requester, &request, None, &local_peer, now)
                .expect("the same occurrence remains admissible after failed lookup")
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("authorize exact response materialization")
        );
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit source A before materialization")
        );
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("independent authenticated source remains admissible")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&prior_route), &local_peer, now,)
                .expect("admit first exact request")
        );
        server
            .enqueue_response(request.clone(), Some(prior_route.clone()), vec![0x11], now)
            .expect("queue singleton response");
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&sibling_route), &local_peer, now,)
                .expect("attach an independent sibling to the materialized response")
        );
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
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&prior_route), &local_peer, now,)
                .expect("an exact completed-source duplicate remains terminal")
        );
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));

        let later_route = routes
            .redeliver(&prior_route)
            .expect("mint later delivery for the same source");
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&later_route), &local_peer, now,)
                .expect("later delivery preserves the terminal cursor")
        );
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
        assert!(routes.retire(&later_route));
        let reconnected_route = routes.mint_via(requester.clone(), hub_a.clone());
        assert!(
            !server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&reconnected_route),
                    &local_peer,
                    now,
                )
                .expect("reconnect preserves the completed source cursor")
        );
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
        assert!(
            !server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&rematerialized_route),
                    &local_peer,
                    now,
                )
                .expect("completed reconnect without shared bytes remains terminal")
        );
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Complete
        );
        assert!(server.drain_outbound_chunks(usize::MAX, now).is_empty());
        assert!(server.outbound.is_empty());
    }

    #[test]
    fn exact_delivery_retry_rematerializes_after_rate_gate_expiry() {
        let (_, requester, _, request, now) = start_session(1, 3);
        let local_peer = request.responder.clone();
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let route = routes.mint(requester.clone());
        let mut server = MergeSidecarTransport::new();
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, now)
                .expect("admit first exact request")
        );
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

        let retry_at = now + SERVER_REQUEST_GATE_TTL + Duration::from_nanos(1);
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route), &local_peer, retry_at,)
                .expect("expired delivery dedup admits exact durable rematerialization")
        );
        server
            .enqueue_response(request, Some(route.clone()), vec![0x11], retry_at)
            .expect("same live delivery rematerializes from durable source");
        let retry = server.drain_outbound_chunks(usize::MAX, retry_at);
        assert!(matches!(
            retry.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(emitted),
                ..
            }] if emitted.same_delivery(&route)
        ));
        assert!(
            !server
                .acknowledge_outbound_chunk(&stale_first_admission, retry_at)
                .expect("a consumed old receipt is a harmless no-op"),
            "a cloned receipt from the expired gate cannot advance its byte-identical replacement"
        );
        assert!(acknowledge_reply_chunk(&mut server, &retry[0], retry_at));
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("admit request through source A")
        );
        server
            .enqueue_response(request.clone(), Some(route_a), vec![0x11], now)
            .expect("queue first response");
        let first = server.drain_outbound_chunks(usize::MAX, now);
        assert_eq!(first.len(), 1);
        assert!(acknowledge_reply_chunk(&mut server, &first[0], now));

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("new alternate source authorizes rematerialization")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("reserve the configured single source")
        );
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
        assert!(
            server
                .admit_server_request(&requester, &request, Some(&first_route), &local_peer, now,)
                .expect("admit first configured source")
        );
        server
            .enqueue_response(request.clone(), Some(first_route), vec![0xA9], now)
            .expect("materialize shared response bytes");
        for index in 1..source_capacity {
            let hub = peer(format!("configured geometry hub {index}").as_bytes());
            let route = routes.mint_via(requester.clone(), hub);
            assert!(
                !server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("attach configured alternate source")
            );
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
            assert!(
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("reserve one bounded gate for hub A")
            );
        }

        let rejected_requester = peer(b"gate cap rejected origin");
        let rejected = routed_server_request(
            &base,
            rejected_requester.clone(),
            b"gate cap rejected request",
            1,
        );
        let rejected_route = routes.mint_via(rejected_requester.clone(), hub_a);
        assert!(matches!(
            server.admit_server_request(
                &rejected_requester,
                &rejected,
                Some(&rejected_route),
                &local_peer,
                now,
            ),
            Err(MergeSidecarError::Capacity("server request rate gate"))
        ));

        let independent_requester = peer(b"gate cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"gate cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        assert!(
            server
                .admit_server_request(
                    &independent_requester,
                    &independent,
                    Some(&independent_route),
                    &local_peer,
                    now,
                )
                .expect("independent hub retains its own gate reservation")
        );
        server
            .enqueue_response(
                independent,
                Some(independent_route.clone()),
                vec![0x11],
                now,
            )
            .expect("independent hub materializes its response");
        assert!(matches!(
            server.drain_outbound_chunks(1, now).as_slice(),
            [MergeSidecarPost {
                reply_route: Some(route),
                ..
            }] if route.same_delivery(&independent_route)
        ));
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts the shared materialization")
        );
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("source B joins the same semantic materialization")
        );
        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let filler_requester = peer(format!("session saturation origin {index}").as_bytes());
            let filler = routed_server_request(
                &base,
                filler_requester.clone(),
                format!("session saturation request {index}").as_bytes(),
                1,
            );
            let filler_route = routes.mint_via(filler_requester.clone(), hub_a.clone());
            assert!(
                server
                    .admit_server_request(
                        &filler_requester,
                        &filler,
                        Some(&filler_route),
                        &local_peer,
                        now,
                    )
                    .expect("reserve source A's bounded response session")
            );
            server
                .enqueue_response(filler, Some(filler_route), vec![0x81], now)
                .expect("fill source A's bounded response session");
        }

        server
            .enqueue_response(request.clone(), Some(route_a), vec![0x82], now)
            .expect("source B remains eligible for the shared materialized bytes");

        let key = (requester, request.request_id);
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
        assert!(server.drain_outbound_chunks(3, now).iter().any(|post| {
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

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts the shared materialization")
        );
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("source B joins the same semantic materialization")
        );
        let filler_requester = peer(b"byte saturation origin");
        let filler = routed_server_request(
            &base,
            filler_requester.clone(),
            b"byte saturation request",
            MAX_OUTBOUND_BYTES_PER_SOURCE,
        );
        let filler_route = routes.mint_via(filler_requester.clone(), hub_a);
        assert!(
            server
                .admit_server_request(
                    &filler_requester,
                    &filler,
                    Some(&filler_route),
                    &local_peer,
                    now,
                )
                .expect("reserve source A's exact byte corridor")
        );
        server
            .enqueue_response(
                filler,
                Some(filler_route),
                vec![0x91; MAX_OUTBOUND_BYTES_PER_SOURCE],
                now,
            )
            .expect("fill source A's exact byte corridor");

        server
            .enqueue_response(request.clone(), Some(route_a), vec![0x92], now)
            .expect("source B retains the shared materialized bytes");

        let key = (requester, request.request_id);
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
        assert!(server.drain_outbound_chunks(2, now).iter().any(|post| {
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
        }));
    }

    #[test]
    fn partitioned_materialization_preserves_rejected_source_resume_cursor() {
        let (_, _, _, base, now) = start_session(1, 3);
        let local_peer = base.responder.clone();
        let requester = peer(b"partitioned resume origin");
        let response_len = MAX_CERTIFIED_MERGE_CHUNK_BYTES + 1;
        let request = routed_server_request(
            &base,
            requester.clone(),
            b"partitioned resume request",
            response_len,
        );
        let response_bytes = vec![0xA5; response_len];
        let hub_a = peer(b"partitioned resume hub a");
        let hub_b = peer(b"partitioned resume hub b");
        let mut routes = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let route_a = routes.mint_via(requester.clone(), hub_a.clone());
        let source_a = ServerRequestSource::Authenticated(route_a.source_key());
        let mut server = MergeSidecarTransport::new();

        assert!(
            server
                .admit_server_request(&requester, &request, Some(&route_a), &local_peer, now)
                .expect("source A starts the original response")
        );
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
        assert_eq!(
            server.outbound[&(requester.clone(), request.request_id)].attempts[&source_a]
                .next_chunk,
            1
        );
        assert!(routes.retire(&route_a));

        let mut filler_ids = BTreeSet::new();
        for index in 0..MAX_OUTBOUND_SESSIONS_PER_SOURCE {
            let filler_requester = peer(format!("partitioned resume filler {index}").as_bytes());
            let filler = routed_server_request(
                &base,
                filler_requester.clone(),
                format!("partitioned resume filler request {index}").as_bytes(),
                1,
            );
            filler_ids.insert(filler.request_id);
            let filler_route = routes.mint_via(filler_requester.clone(), hub_a.clone());
            assert!(
                server
                    .admit_server_request(
                        &filler_requester,
                        &filler,
                        Some(&filler_route),
                        &local_peer,
                        now,
                    )
                    .expect("fill source A's independent response sessions")
            );
            server
                .enqueue_response(filler, Some(filler_route), vec![0xB6], now)
                .expect("queue one source A filler session");
        }
        assert_eq!(
            server.server_request_gates[&(requester.clone(), request.request_id)].attempts
                [&source_a]
                .cursor,
            ServerResponseCursor::Pending(1)
        );

        let route_a_reconnected = routes.mint_via(requester.clone(), hub_a);
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&route_a_reconnected),
                    &local_peer,
                    now,
                )
                .expect("source A reconnect authorizes rematerialization at its retained cursor")
        );
        assert!(matches!(
            server.enqueue_response(
                request.clone(),
                Some(route_a_reconnected.clone()),
                response_bytes.clone(),
                now,
            ),
            Err(MergeSidecarError::Capacity("outbound response budget"))
        ));
        server.cancel_unmaterialized_server_request(&requester, &request);
        let key = (requester.clone(), request.request_id);
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert!(!server.server_request_gates[&key].attempts[&source_a].materialization_authorized);

        let route_a_partitioned = routes
            .redeliver(&route_a_reconnected)
            .expect("later source A delivery retries the parked materialization");
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&route_a_partitioned),
                    &local_peer,
                    now,
                )
                .expect("source A reauthorizes after the production cancel path")
        );
        server.cancel_unmaterialized_server_request(&requester, &request);
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Pending(1),
            "durable lookup failure cannot erase the reconnect cursor"
        );
        assert!(server.server_request_gates[&key].attempts[&source_a].materialization_retryable);
        assert!(
            server
                .admit_server_request(
                    &requester,
                    &request,
                    Some(&route_a_partitioned),
                    &local_peer,
                    now,
                )
                .expect("the exact delivery retries failed terminating local work")
        );
        let route_b = routes.mint_via(requester.clone(), hub_b);
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&route_b), &local_peer, now)
                .expect("source B joins source A's semantic materialization")
        );
        server
            .enqueue_response(
                request.clone(),
                Some(route_a_partitioned.clone()),
                response_bytes,
                now,
            )
            .expect("source B acquires shared bytes while source A is saturated");
        assert!(!server.outbound[&key].attempts.contains_key(&source_a));
        assert_eq!(
            server.server_request_gates[&key].attempts[&source_a].cursor,
            ServerResponseCursor::Pending(1)
        );
        assert!(!server.server_request_gates[&key].attempts[&source_a].materialization_authorized);

        let posts = server.drain_outbound_chunks(usize::MAX, now);
        let mut released_fillers = 0usize;
        for post in &posts {
            let CertifiedMergeSidecarMessage::Chunk(chunk) = post.message.as_ref() else {
                continue;
            };
            if filler_ids.contains(&chunk.request_id) {
                assert!(acknowledge_reply_chunk(&mut server, post, now));
                released_fillers += 1;
            }
        }
        assert_eq!(released_fillers, MAX_OUTBOUND_SESSIONS_PER_SOURCE);
        assert_eq!(server.source_outbound_count(&source_a), 0);

        let exact_a = route_a_partitioned.clone();
        assert!(
            !server
                .admit_server_request(&requester, &request, Some(&exact_a), &local_peer, now)
                .expect("exact source delivery reattaches to the materialized response")
        );
        assert_eq!(server.outbound[&key].attempts[&source_a].next_chunk, 1);
        assert_eq!(
            server.outbound[&key].attempts[&source_a].in_flight_chunk,
            None
        );
        let resumed = server.drain_outbound_chunks(usize::MAX, now);
        assert!(matches!(
            resumed.as_slice(),
            [MergeSidecarPost {
                reply_route: Some(route),
                message,
                ..
            }] if route.same_delivery(&exact_a)
                && matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(chunk)
                    if chunk.chunk_index == 1)
        ));
        assert!(acknowledge_reply_chunk(&mut server, &resumed[0], now));
        assert!(server.drain_outbound_chunks(usize::MAX, now).is_empty());
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
            assert!(
                server
                    .admit_server_request(&requester, &request, Some(&route), &local_peer, now,)
                    .expect("admit bounded hub A session")
            );
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
        assert!(
            server
                .admit_server_request(
                    &rejected_requester,
                    &rejected,
                    Some(&rejected_route),
                    &local_peer,
                    now,
                )
                .expect("the cheap gate remains independently bounded")
        );
        assert!(matches!(
            server.enqueue_response(rejected, Some(rejected_route), vec![0x33], now),
            Err(MergeSidecarError::Capacity("outbound response budget"))
        ));

        let independent_requester = peer(b"session cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"session cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        assert!(
            server
                .admit_server_request(
                    &independent_requester,
                    &independent,
                    Some(&independent_route),
                    &local_peer,
                    now,
                )
                .expect("independent hub retains its own session reservation")
        );
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
        assert!(
            server
                .admit_server_request(&full_requester, &full, Some(&full_route), &local_peer, now,)
                .expect("admit the exact per-source byte bound")
        );
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
        assert!(
            server
                .admit_server_request(
                    &overflow_requester,
                    &overflow,
                    Some(&overflow_route),
                    &local_peer,
                    now,
                )
                .expect("admit bounded lookup before exact byte accounting")
        );
        assert!(matches!(
            server.enqueue_response(overflow, Some(overflow_route), vec![0x66], now),
            Err(MergeSidecarError::Capacity("outbound response budget"))
        ));

        let independent_requester = peer(b"byte cap independent origin");
        let independent = routed_server_request(
            &base,
            independent_requester.clone(),
            b"byte cap independent request",
            1,
        );
        let independent_route = routes.mint_via(independent_requester.clone(), hub_b);
        assert!(
            server
                .admit_server_request(
                    &independent_requester,
                    &independent,
                    Some(&independent_route),
                    &local_peer,
                    now,
                )
                .expect("independent hub retains its own byte reservation")
        );
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
        second.request_id = Hash::new(b"second fair outbound request");
        let mut server = MergeSidecarTransport::new();
        for request in [&first, &second] {
            assert!(
                server
                    .admit_server_request(
                        &request.requester,
                        request,
                        None,
                        &request.responder,
                        now,
                    )
                    .expect("admit fair outbound response")
            );
        }
        server
            .enqueue_response(
                first.clone(),
                None,
                vec![0x11; MAX_CERTIFIED_MERGE_CHUNK_BYTES * 3],
                now,
            )
            .expect("queue first response");
        server
            .enqueue_response(second.clone(), None, vec![0x22], now)
            .expect("queue second response");

        let posts = server.drain_outbound_chunks(2, now);
        assert_eq!(posts.len(), 2);
        let request_ids = posts
            .into_iter()
            .map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Chunk(chunk) => chunk.request_id,
                CertifiedMergeSidecarMessage::Request(_) => panic!("response emitted a request"),
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
        short.request_id = Hash::prehashed([0; Hash::LENGTH]);
        long.request_id = Hash::prehashed([u8::MAX; Hash::LENGTH]);
        let mut routes = NetworkReplyRouteTestFixture::new(peer(b"replacement fairness hub"));
        let short_route = routes.mint(short.requester.clone());
        let long_route = routes.mint(long.requester.clone());
        let mut server = MergeSidecarTransport::new();
        for (request, route) in [(&short, &short_route), (&long, &long_route)] {
            assert!(
                server
                    .admit_server_request(
                        &request.requester,
                        request,
                        Some(route),
                        &request.responder,
                        now,
                    )
                    .expect("admit initial response")
            );
        }
        server
            .enqueue_response(short.clone(), Some(short_route.clone()), vec![0x11], now)
            .expect("queue first short response");
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
        replacement.request_id = Hash::prehashed([1; Hash::LENGTH]);
        let replacement_route = routes.mint(replacement.requester.clone());
        assert!(
            server
                .admit_server_request(
                    &replacement.requester,
                    &replacement,
                    Some(&replacement_route),
                    &replacement.responder,
                    now,
                )
                .expect("admit adversarial short replacement")
        );
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
        assert!(
            transport
                .admit_server_request(
                    &response_request.requester,
                    &response_request,
                    Some(&response_route),
                    &response_request.responder,
                    now,
                )
                .expect("admit bounded response")
        );
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
            let posts = transport.tick_bounded(&requester, timed_out_at, 1);
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
        let (deferred, _) = transport.finish_completed(
            honest.entry_hash,
            certified_merge_reference_digest(&honest),
            true,
            &requester,
            now,
        );
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
        transport.release_unsent_request(&first);
        let key = (
            reference.entry_hash,
            certified_merge_reference_digest(&reference),
        );
        let assembly = transport.inbound.get(&key).expect("retained exact session");
        assert_eq!(assembly.attempts, 0);
        assert_eq!(assembly.holder_cursor, 0);

        let reissued = transport
            .tick_bounded(&requester, now, 1)
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("unsent request is immediately reissued");
        assert_eq!(reissued.responder, first.responder);
        assert_ne!(reissued.request_id, first.request_id);

        let rotated = transport
            .tick_bounded(&requester, now + REQUEST_TIMEOUT, 1)
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Chunk(_) => None,
            })
            .expect("first real attempt expires at the base timeout");
        assert_ne!(rotated.responder, reissued.responder);
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
            .into_iter()
            .find_map(|post| match Arc::unwrap_or_clone(post.message) {
                CertifiedMergeSidecarMessage::Request(request) => Some(request),
                CertifiedMergeSidecarMessage::Chunk(_) => None,
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
    fn corrupt_canonical_bytes_are_rejected() {
        let reference = reference(4, 1);
        assert!(matches!(
            decode_certified_merge_sidecar(&reference, &[1, 2, 3, 4]),
            Err(MergeSidecarError::Decode(_))
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
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), first)
            .expect("first authorization");
        guard
            .authorize(context.clone(), first)
            .expect("idempotent authorization");
        assert_eq!(
            guard.authorize(context.clone(), second),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
        drop(guard);
        let restarted = MergeSigningGuard::open(temp.path()).expect("restart guard");
        assert_eq!(
            restarted.authorize(context, second),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
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
            guard
                .authorize(context, Hash::new(epoch_id.to_le_bytes()))
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
            guard
                .authorize(context, Hash::new(carrier_height.to_le_bytes()))
                .expect("authorize exact uncommitted carrier round");
            guard
                .advance_committed_frontier(0, carrier_height)
                .expect("ordinary global block finalizes carrier height");
        }
        drop(guard);
        let restarted = MergeSigningGuard::open_with_committed_frontier(temp.path(), 0, rounds)
            .expect("restart after many ordinary blocks");
        assert_eq!(restarted.committed_carrier_height, rounds);

        let later = MergeSigningContextV1 {
            epoch_id: 1,
            view: 0,
            carrier_height: rounds + 1,
            parent_hash: HashOf::from_untyped_unchecked(Hash::new(rounds.to_le_bytes())),
            validator_set_hash: roster_hash,
        };
        restarted
            .authorize(later, Hash::new(b"later candidate"))
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
        let guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), first)
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
            restarted.authorize(context, second),
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
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"))
            .expect("authorize epoch");
        guard
            .advance_committed_epoch(1)
            .expect("commit and prune epoch");
        drop(guard);
        let restarted =
            MergeSigningGuard::open_with_committed_epoch(temp.path(), 1).expect("restart guard");
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict")),
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
        let mut guard = MergeSigningGuard::open(temp.path()).expect("open guard");
        guard
            .authorize(context.clone(), Hash::new(b"first"))
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
        let restarted = MergeSigningGuard::open_with_committed_frontier(temp.path(), 1, 2)
            .expect("restart completes stale-record GC");
        assert!(!record_path.exists());
        assert_eq!(
            restarted.authorize(context, Hash::new(b"conflict")),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
    }
}
