#[test]
fn fair_v2_ingress_maximum_merge_sidecar_chunk_frame_matches_canonical_wire() {
    use crate::merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
        CertifiedMergeSidecarMessage, CertifiedMergeSidecarSemanticSequenceV1,
        CertifiedMergeSidecarServiceGenerationV1, CertifiedMergeSidecarStreamEpochV1,
        MAX_CERTIFIED_MERGE_CHUNK_BYTES,
    };
    let peers = validator_peers(2);
    let requester = peers.first().expect("requester fixture").clone();
    let responder = peers.get(1).expect("responder fixture").clone();
    let (_, requester_key_bytes) = requester
        .public_key()
        .try_to_bytes()
        .expect("requester key is canonical");
    let (_, responder_key_bytes) = responder
        .public_key()
        .try_to_bytes()
        .expect("responder key is canonical");
    assert_eq!(
        requester_key_bytes.len(),
        responder_key_bytes.len(),
        "the exact fixture helper takes one shared embedded key width"
    );
    let message = crate::NetworkMessage::CertifiedMergeSidecar(Arc::new(
        CertifiedMergeSidecarMessage::Chunk(CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
            stream_epoch: CertifiedMergeSidecarStreamEpochV1(std::num::NonZeroU64::MIN),
            semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(std::num::NonZeroU64::MAX),
            request_id: Hash::new(b"maximum-sidecar-request"),
            entry_hash: HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(
                b"maximum-sidecar-entry",
            )),
            encoded_len: u64::MAX,
            epoch_id: u64::MAX,
            reference_digest: Hash::new(b"maximum-sidecar-reference"),
            requester: requester.clone(),
            responder: responder.clone(),
            chunk_index: u32::MAX,
            chunk_count: u32::MAX,
            bytes: vec![0xA5; MAX_CERTIFIED_MERGE_CHUNK_BYTES],
        }),
    ));
    let required_network_message_bytes =
        super::fair_v2_ingress_required_merge_sidecar_chunk_network_message_bytes_for_key(
            requester_key_bytes.len(),
        )
        .expect("fixture wire geometry is representable");
    assert_eq!(
        message.encoded_len(),
        required_network_message_bytes,
        "allocation-free geometry must equal the maximum wrapped canonical chunk"
    );
    let exact_frame =
        iroha_p2p::network::data_frame_wire_len(&responder, Some(&requester), &message);
    assert_eq!(
        iroha_p2p::network::data_frame_wire_len_from_payload_len::<crate::NetworkMessage>(
            &responder,
            Some(&requester),
            required_network_message_bytes,
        ),
        exact_frame,
        "allocation-free P2P geometry must equal the encoded fixture frame"
    );
    assert!(
        super::fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes() >= exact_frame,
        "feature-independent maximum-key geometry must cover the concrete fixture"
    );
}
#[test]
fn fair_v2_ingress_minimal_layout_enforces_exact_block_sync_frame_boundary() {
    let layout = minimal_rs16_layout();
    let required_block_sync = super::fair_v2_ingress_required_block_sync_p2p_frame_bytes(layout);
    let required_sidecar = super::fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes();
    assert_ne!(required_sidecar, usize::MAX);
    assert!(
        required_block_sync >= required_sidecar,
        "minimal DA geometry must retain the layout-neutral 64-KiB sidecar requirement"
    );
    let validator = validator_peers(1).pop().expect("validator fixture");
    let network_id = crate::sumeragi::synthetic_network_id("minimal-sidecar-frame-test");
    let required_control_message = super::fair_v2_ingress_required_proposal_bytes(layout, 1)
        .max(super::fair_v2_ingress_required_commit_certificate_response_bytes(1));
    let required_consensus = super::fair_v2_ingress_required_p2p_frame_bytes(
        super::fair_v2_ingress_required_recovery_request_bytes(&network_id, 1),
    )
    .max(super::fair_v2_ingress_required_lane_p2p_frame_bytes(
        super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES,
    ));
    let required_control =
        super::fair_v2_ingress_required_p2p_frame_bytes(required_control_message);
    let required_outbound = iroha_p2p::frame_queue_charge(
        required_consensus
            .max(required_control)
            .max(required_block_sync),
    )
    .expect("minimal-context outbound charge is representable");
    let ordinary_bytes = super::MAX_LANE_PROGRESS_MESSAGE_WIRE_BYTES;
    let certified_bytes = super::fair_v2_ingress_required_certified_fence_escape_bytes(1);
    let completion_bytes = super::MAX_LANE_COMPLETION_MESSAGE_WIRE_BYTES;
    let source_bytes = ordinary_bytes
        .checked_add(certified_bytes)
        .and_then(|bytes| bytes.checked_add(completion_bytes))
        .expect("test source geometry fits usize");
    let byte_capacity = source_bytes
        .checked_mul(2)
        .expect("validator partition fits usize");
    let ingress_with_transport_caps = |block_sync, outbound_high| {
        super::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            7,
            byte_capacity,
            source_bytes,
            certified_bytes,
            0,
            completion_bytes,
            required_consensus,
            required_control,
            block_sync,
            outbound_high,
            None,
        )
    };
    let exact = ingress_with_transport_caps(required_block_sync, required_outbound);
    exact
        .configure_roster_for_context([validator.clone()], &network_id, layout)
        .expect("the exact transport frame caps must activate");
    exact.open().expect("the exact transport caps must open");
    let short = ingress_with_transport_caps(
        required_block_sync
            .checked_sub(1)
            .expect("required frame is non-zero"),
        required_outbound,
    );
    let error = short
        .configure_roster_for_context([validator.clone()], &network_id, layout)
        .expect_err("one byte below the exact BlockSync frame cap must fail closed");
    assert_eq!(error.configured(), required_block_sync - 1);
    assert_eq!(error.required(), required_block_sync);
    assert_eq!(
        error.kind,
        super::FairV2IngressCapacityKind::BlockSyncFrameBytes
    );
    assert_eq!(short.open(), Err(error));
    let outbound_short = ingress_with_transport_caps(required_block_sync, required_outbound - 1);
    let outbound_error = outbound_short
        .configure_roster_for_context([validator], &network_id, layout)
        .expect_err("one byte below the exact outbound-high cap must fail closed");
    assert_eq!(outbound_error.configured(), required_outbound - 1);
    assert_eq!(outbound_error.required(), required_outbound);
    assert_eq!(
        outbound_error.kind,
        super::FairV2IngressCapacityKind::OutboundHighFrameBytes
    );
    assert_eq!(outbound_short.open(), Err(outbound_error));
}
