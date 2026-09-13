//! Native semantic recovery maxima including canonical relay and peer framing.
//! These are shape/admission bounds, never transaction or authority validation.
use super::NetworkMessage;
use crate::merge_sidecar::*;
use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_data_model::merge::{MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLedgerEntry};
use iroha_model_base::peer::PeerId;
use norito::core as ncore;
use std::{num::NonZeroU64, sync::Arc};

/// Complete canonical P2P frame maxima of the two semantic recovery classes.
/// Topic caps remain independently enforced; ordinary RS16 data does not use
/// these bounds even when it shares `ConsensusChunk` with a sidecar chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryFrameMaxima {
    /// Maximum of Request, Close, CloseAck and GenerationHint, all wrapped.
    pub control: usize,
    /// One full 64 KiB sidecar chunk plus all canonical envelope fields.
    pub data: usize,
}

fn maximum_shapes(peer: &PeerId) -> Result<[CertifiedMergeSidecarMessage; 5], ncore::Error> {
    let (algorithm, _) = peer.public_key().to_bytes();
    if algorithm != Algorithm::BlsNormal {
        return Err(ncore::Error::Message(
            "recovery transport maxima require the mandatory BLS node identity".to_owned(),
        ));
    }
    let hash = Hash::new(b"iroha:transport-admission:fixed-width-shape:v1");
    let entry_hash: HashOf<MergeLedgerEntry> = HashOf::from_untyped_unchecked(hash);
    let generation = CertifiedMergeSidecarServiceGenerationV1(NonZeroU64::MAX);
    let epoch = CertifiedMergeSidecarStreamEpochV1(NonZeroU64::MAX);
    let sequence = CertifiedMergeSidecarSemanticSequenceV1(NonZeroU64::MAX);
    // Every field except the chunk Vec is fixed width or a mandatory BLS PeerId.
    // This is an encoding witness, not a fabricated valid semantic request.
    Ok([
        CertifiedMergeSidecarMessage::Request(CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: generation,
            stream_epoch: epoch,
            semantic_sequence: sequence,
            closed_through: u64::MAX,
            request_id: hash,
            entry_hash,
            encoded_len: MAX_MERGE_LEDGER_ENTRY_BYTES as u64,
            epoch_id: u64::MAX,
            reference_digest: hash,
            requester: peer.clone(),
            responder: peer.clone(),
        }),
        CertifiedMergeSidecarMessage::Close(CertifiedMergeSidecarCloseV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: generation,
            stream_epoch: epoch,
            closed_through: u64::MAX,
            close_id: hash,
            requester: peer.clone(),
            responder: peer.clone(),
        }),
        CertifiedMergeSidecarMessage::CloseAck(CertifiedMergeSidecarCloseAckV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: generation,
            stream_epoch: epoch,
            closed_through: u64::MAX,
            close_id: hash,
            requester: peer.clone(),
            responder: peer.clone(),
        }),
        CertifiedMergeSidecarMessage::GenerationHint(CertifiedMergeSidecarGenerationHintV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            observed_generation: generation,
            current_generation: generation,
            observed_message_hash: hash,
            hint_id: hash,
            requester: peer.clone(),
            responder: peer.clone(),
        }),
        CertifiedMergeSidecarMessage::Chunk(CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: generation,
            stream_epoch: epoch,
            semantic_sequence: sequence,
            request_id: hash,
            entry_hash,
            encoded_len: MAX_MERGE_LEDGER_ENTRY_BYTES as u64,
            epoch_id: u64::MAX,
            reference_digest: hash,
            requester: peer.clone(),
            responder: peer.clone(),
            chunk_index: (MAX_CERTIFIED_MERGE_CHUNKS - 1) as u32,
            chunk_count: MAX_CERTIFIED_MERGE_CHUNKS as u32,
            bytes: vec![0; MAX_CERTIFIED_MERGE_CHUNK_BYTES],
        }),
    ])
}

/// Count real native maximum-shaped serializations and their exact canonical
/// BLS relay/Data envelopes under the mandatory transport encode layout.
///
/// # Errors
/// Rejects a non-BLS identity, serialization failure or arithmetic overflow.
/// The active node key supplies only an identity of the required encoded width;
/// no private key, signature operation, network request or state read is needed.
pub fn recovery_frame_maxima(peer: &PeerId) -> Result<RecoveryFrameMaxima, ncore::Error> {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    let shapes = maximum_shapes(peer)?;
    let mut result = RecoveryFrameMaxima {
        control: 0,
        data: 0,
    };
    for (i, shape) in shapes.into_iter().enumerate() {
        let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(shape));
        let bare = ncore::encoded_payload_len(&message)?;
        let direct =
            iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<NetworkMessage>(bare);
        let broadcast = iroha_p2p::network::broadcast_data_frame_wire_len_from_payload_len::<
            NetworkMessage,
        >(bare);
        let complete = direct.max(broadcast);
        if complete == usize::MAX {
            return Err(ncore::Error::LengthMismatch);
        }
        if i == 4 {
            result.data = complete;
        } else {
            result.control = result.control.max(complete);
        }
    }
    Ok(result)
}

fn availability_shape(
    peer: &PeerId,
    index: u32,
    sender: u32,
) -> Result<NetworkMessage, ncore::Error> {
    use crate::sumeragi::message::{BlockMessage, BlockMessageWire};
    use iroha_data_model::block::consensus_v2 as wire;
    if peer.public_key().algorithm() != Algorithm::BlsNormal {
        return Err(ncore::Error::Message(
            "availability maximum requires a BLS node identity".to_owned(),
        ));
    }
    Ok(NetworkMessage::SumeragiBlock(Arc::new(
        BlockMessageWire::new(BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
                manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"availability maximum shape",
                )),
                index,
                bytes: vec![0; wire::MAX_DA_CHUNK_SIZE_BYTES as usize],
                sender,
                signature: vec![0; wire::MAX_CONSENSUS_SIGNATURE_BYTES],
            }),
        ))),
    )))
}

/// Derive the complete ordinary RS16 frame bound from its native protocol shape.
/// This authenticates no chunk; it funds the largest structurally admissible
/// chunk/signature plus every canonical outer envelope before native decoding.
///
/// # Errors
/// Rejects a non-BLS node identity, encoding failure or arithmetic overflow.
pub fn availability_frame_maximum(peer: &PeerId) -> Result<usize, ncore::Error> {
    let _flags = ncore::DecodeFlagsGuard::enter(ncore::default_encode_flags());
    use iroha_data_model::block::consensus_v2 as wire;
    let bare = ncore::encoded_payload_len(&availability_shape(
        peer,
        wire::MAX_DA_CHUNK_COUNT - 1,
        (wire::MAX_VALIDATORS_PER_HEIGHT - 1) as u32,
    )?)?;
    let direct =
        iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<NetworkMessage>(bare);
    let broadcast =
        iroha_p2p::network::broadcast_data_frame_wire_len_from_payload_len::<NetworkMessage>(bare);
    let maximum = direct.max(broadcast);
    if maximum == usize::MAX {
        return Err(ncore::Error::LengthMismatch);
    }
    Ok(maximum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;

    #[test]
    fn actual_native_semantic_maxima_fit_shipping_actor_post_writer_and_receive_geometry() {
        let key = KeyPair::try_from_seed(vec![68; 32], Algorithm::BlsNormal).unwrap();
        iroha_p2p::network::assert_native_semantic_geometry_for_test::<NetworkMessage>(
            &key.public_key().clone().into(),
        );
    }

    #[test]
    fn maxima_equal_materialized_signed_canonical_peer_relay_frames() {
        let key = KeyPair::try_from_seed(vec![61; 32], Algorithm::BlsNormal).unwrap();
        let peer = PeerId::from(key.public_key().clone());
        let maximum = recovery_frame_maxima(&peer).unwrap();
        let mut actual_control = 0;
        let mut actual_data = 0;
        for shape in maximum_shapes(&peer).unwrap() {
            let is_data = matches!(&shape, CertifiedMergeSidecarMessage::Chunk(_));
            for target in [None, Some(peer.clone())] {
                let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(shape.clone()));
                let actual = iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                    &key, target, message,
                )
                .unwrap();
                if is_data {
                    actual_data = actual_data.max(actual);
                } else {
                    actual_control = actual_control.max(actual);
                }
            }
        }
        assert_eq!(maximum.control, actual_control);
        assert_eq!(maximum.data, actual_data);
        assert!(maximum.data > MAX_CERTIFIED_MERGE_CHUNK_BYTES);
        assert!(
            maximum.data < 128 * 1024,
            "actual complete sidecar envelope stays far below ordinary RS16 topic maximum"
        );
    }

    #[test]
    fn sidecar_counter_extrema_fit_the_materialized_maximum() {
        let key = KeyPair::try_from_seed(vec![65; 32], Algorithm::BlsNormal).unwrap();
        let peer = PeerId::from(key.public_key().clone());
        let maximum = recovery_frame_maxima(&peer).unwrap();
        let mut shapes = maximum_shapes(&peer).unwrap();
        let CertifiedMergeSidecarMessage::Chunk(chunk) = &mut shapes[4] else {
            unreachable!()
        };
        for index in [0, (MAX_CERTIFIED_MERGE_CHUNKS - 1) as u32] {
            chunk.chunk_index = index;
            for epoch in [0, u64::MAX] {
                chunk.epoch_id = epoch;
                let message = NetworkMessage::CertifiedMergeSidecar(Arc::new(
                    CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
                ));
                let actual = iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                    &key,
                    Some(peer.clone()),
                    message,
                )
                .unwrap();
                assert!(actual <= maximum.data);
            }
        }
    }

    #[test]
    fn sidecar_size_witness_is_key_value_independent_and_rejects_other_algorithms() {
        let a = KeyPair::try_from_seed(vec![62; 32], Algorithm::BlsNormal).unwrap();
        let b = KeyPair::try_from_seed(vec![63; 32], Algorithm::BlsNormal).unwrap();
        assert_eq!(
            recovery_frame_maxima(&a.public_key().clone().into()).unwrap(),
            recovery_frame_maxima(&b.public_key().clone().into()).unwrap()
        );
        let foreign = KeyPair::try_from_seed(vec![64; 32], Algorithm::Ed25519).unwrap();
        assert!(recovery_frame_maxima(&foreign.public_key().clone().into()).is_err());
    }
    #[test]
    fn availability_maximum_matches_actual_signed_relay_envelope_and_counter_extrema() {
        use iroha_data_model::block::consensus_v2 as wire;
        let key = KeyPair::try_from_seed(vec![66; 32], Algorithm::BlsNormal).unwrap();
        let peer = PeerId::from(key.public_key().clone());
        let maximum = availability_frame_maximum(&peer).unwrap();
        let shape = availability_shape(
            &peer,
            wire::MAX_DA_CHUNK_COUNT - 1,
            (wire::MAX_VALIDATORS_PER_HEIGHT - 1) as u32,
        )
        .unwrap();
        assert!(
            maximum > wire::MAX_DA_CHUNK_SIZE_BYTES as usize + wire::MAX_CONSENSUS_SIGNATURE_BYTES
        );
        assert!(maximum < 512 * 1024);
        let mut observed = 0;
        for target in [None, Some(peer.clone())] {
            observed = observed.max(
                iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                    &key,
                    target,
                    shape.clone(),
                )
                .unwrap(),
            );
        }
        assert_eq!(observed, maximum);
        for index in [0, wire::MAX_DA_CHUNK_COUNT - 1] {
            for sender in [0, (wire::MAX_VALIDATORS_PER_HEIGHT - 1) as u32] {
                let shape = availability_shape(&peer, index, sender).unwrap();
                for target in [None, Some(peer.clone())] {
                    let actual = iroha_p2p::network::materialized_signed_data_frame_len_for_test(
                        &key,
                        target,
                        shape.clone(),
                    )
                    .unwrap();
                    assert!(actual <= maximum);
                }
            }
        }
        let foreign = KeyPair::try_from_seed(vec![67; 32], Algorithm::Ed25519).unwrap();
        assert!(availability_frame_maximum(&foreign.public_key().clone().into()).is_err());
    }
}
