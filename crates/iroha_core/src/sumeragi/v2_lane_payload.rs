//! Mandatory RS16 materialization of exact immutable first-admission inputs.
//!
//! Local reconstruction reuses the global codec's canonical striping and Merkle
//! commitment. This is CPU-only preparation, not a durable-body receipt, an
//! availability vote, or a live-authority lease. TODO: attach the result to the
//! process-lived shared reducer's bounded storage/validation job and retain that
//! custody through completion, including certified first-carrier recovery.

use iroha_crypto::Hash;
use iroha_data_model::block::{
    consensus_v2 as wire,
    lane_consensus::{LaneManifestV1, LaneValueKindV1, LaneValueRefV1, lane_availability_hash},
    lane_input::LaneInputPayloadV1,
};

use crate::state::{VerifiedLaneContext, VerifiedLaneInputBodyV1};

/// Complete deterministic codeword and its exact unsigned native manifest.
/// Construction never grants permission to sign or report a body as durable.
#[derive(Debug)]
pub(crate) struct EncodedLaneInputV1 {
    manifest: LaneManifestV1,
    chunks: Vec<Vec<u8>>,
}
impl EncodedLaneInputV1 {
    /// Exact manifest to retain with the reducer's original proposal intent.
    pub(crate) fn manifest(&self) -> &LaneManifestV1 {
        &self.manifest
    }
    /// Transfer all physical chunks and manifest to their bounded storage owner.
    pub(crate) fn into_parts(self) -> (LaneManifestV1, Vec<Vec<u8>>) {
        (self.manifest, self.chunks)
    }
}

/// Materialize exact admitted bytes for one frozen lane and original view.
///
/// The reducer chooses when a fresh origin is permitted. A locked reproposal
/// retains its original manifest and must not substitute the current voting view.
/// Every productive effect must additionally recheck the current complete route
/// set under State's publication lease. Neither this function nor its result
/// changes view, chooses work, signs, persists, or acknowledges body readiness.
pub(crate) fn encode_lane_input(
    lane: &VerifiedLaneContext,
    body: &VerifiedLaneInputBodyV1,
    origin_view: u64,
) -> Result<EncodedLaneInputV1, String> {
    let frozen = lane.frozen();
    if body.source().validated_input().certificate().binding_hash != frozen.admitted_binding_hash
        || body.source().source().finality().height_context.network_id != frozen.network_id
    {
        return Err("native input differs from the frozen target instance".to_owned());
    }
    encode_frozen_lane_input(
        lane,
        body.payload(),
        body.kind(),
        body.canonical_bytes(),
        origin_view,
    )
}

/// Shared deterministic manifest calculation for live custody and offline evidence.
/// The caller owns first-carrier authentication; this result grants no live authority.
/// Both callers retain validated exact canonical payload bytes and its derived kind;
/// this function preserves the shared slot, instance, leader and RS16 calculation.
pub(crate) fn encode_frozen_lane_input(
    lane: &VerifiedLaneContext,
    payload: &LaneInputPayloadV1,
    kind: LaneValueKindV1,
    bytes: &[u8],
    origin_view: u64,
) -> Result<EncodedLaneInputV1, String> {
    let frozen = lane.frozen();
    let slot = payload
        .descriptor
        .slots
        .iter()
        .find(|slot| {
            slot.route.lane_id == frozen.lane_id && slot.route.dataspace_id == frozen.dataspace_id
        })
        .ok_or_else(|| "native input omits the target route".to_owned())?;
    let instance_id = Hash::from(lane.instance_id().0);
    if slot.instance_id != instance_id
        || slot.lane_incarnation != frozen.lane_incarnation
        || slot.lane_height != frozen.next_lane_height
        || payload.descriptor.admission_priority != frozen.admission_priority
        || payload.input.certificate.binding.canonical_hash() != frozen.admitted_binding_hash
    {
        return Err("native input differs from the frozen target instance".to_owned());
    }
    let context = lane.reducer_context();
    let leader = context.leader(origin_view);
    let origin_producer = context
        .roster()
        .iter()
        .position(|entry| entry.id() == leader)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or_else(|| "native origin leader is absent from its exact frozen roster".to_owned())?;
    let chunks =
        wire::encode_payload_chunks(frozen.da_layout, bytes).map_err(|error| error.to_string())?;
    let chunk_hashes = chunks.iter().map(Hash::new).collect::<Vec<_>>();
    let chunk_root = wire::payload_chunk_root(&chunk_hashes)
        .ok_or_else(|| "native RS16 input has no encoded chunks".to_owned())?;
    let byte_len = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
    let chunk_count = u32::try_from(chunks.len()).map_err(|error| error.to_string())?;
    let availability_hash =
        lane_availability_hash(frozen.da_layout, chunk_root, byte_len, chunk_count)
            .map_err(|error| error.to_string())?;
    let manifest = LaneManifestV1 {
        value: LaneValueRefV1 {
            instance_id,
            admitted_binding_hash: frozen.admitted_binding_hash,
            kind,
            origin_view,
            origin_producer,
            descriptor_hash: payload.descriptor.canonical_hash()?,
            payload_hash: Hash::new(bytes),
            availability_hash,
        },
        layout: frozen.da_layout,
        chunk_root,
        byte_len,
        chunk_count,
    };
    manifest
        .validate_availability()
        .map_err(|error| error.to_string())?;
    Ok(EncodedLaneInputV1 { manifest, chunks })
}

/// Reconstruct and compare the exact expected body/codeword behind a manifest.
///
/// This checks immutable body identity; the proposal/QC signature is checked by
/// the native authenticator. A valid result still requires durable storage,
/// current all-route eligibility and the reducer-issued completion tag.
pub(crate) fn verify_lane_input_manifest(
    lane: &VerifiedLaneContext,
    body: &VerifiedLaneInputBodyV1,
    manifest: &LaneManifestV1,
    received_body: &[u8],
) -> Result<EncodedLaneInputV1, String> {
    if received_body != body.canonical_bytes() {
        return Err(
            "received native body differs from the exact admitted input and route slots".into(),
        );
    }
    let expected = encode_lane_input(lane, body, manifest.value.origin_view)?;
    if expected.manifest() != manifest {
        return Err("native manifest differs from the exact immutable RS16 codeword".into());
    }
    Ok(expected)
}
