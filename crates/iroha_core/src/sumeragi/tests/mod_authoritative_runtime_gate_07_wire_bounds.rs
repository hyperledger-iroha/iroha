#[test]
fn fair_v2_ingress_maximum_merge_sidecar_chunk_frame_matches_canonical_wire() {
    use crate::merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkV1,
        CertifiedMergeSidecarMessage, CertifiedMergeSidecarSemanticSequenceV1,
        CertifiedMergeSidecarServiceGenerationV1, CertifiedMergeSidecarStreamEpochV1,
        MAX_CERTIFIED_MERGE_CHUNK_BYTES,
    };
    let peers = (1_u8..=2)
        .map(|seed| {
            PeerId::new(
                KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::BlsNormal)
                    .expect("deterministic relay-node BLS key")
                    .public_key()
                    .clone(),
            )
        })
        .collect::<Vec<_>>();
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
    let maximum_frame = super::fair_v2_ingress_required_merge_sidecar_chunk_p2p_frame_bytes();
    assert!(
        maximum_frame >= exact_frame,
        "feature-independent maximum-key geometry must cover the concrete fixture: maximum={maximum_frame}, concrete={exact_frame}"
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

// Codec/classification fixtures only. These bytes grant no authentication;
// the State transport tests below use the actual four-validator signed owners.
fn native_wire_classification_fixtures() -> [BlockMessage; 2] {
    use iroha_data_model::block::lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneDecisionV1, LaneManifestV1, LaneMessageEnvelopeV1,
        LaneMessageV1, LanePhaseV1, LaneQcV1, LaneRoundV1, LaneSignatureShareV1, LaneTimeoutBodyV1,
        LaneTimeoutVoteV1, LaneValueKindV1, LaneValueRefV1, LaneVoteStatementV1,
    };
    let hash = Hash::new(b"native codec and closed-ingress fixture");
    let round = LaneRoundV1 {
        instance_id: hash,
        lane_height: 1,
        voting_view: 0,
    };
    let share = LaneSignatureShareV1 {
        signer: 0,
        signature: vec![0x71; 96],
    };
    let value = LaneValueRefV1 {
        instance_id: hash,
        admitted_binding_hash: hash,
        kind: LaneValueKindV1::Execution,
        origin_view: 0,
        origin_producer: 0,
        descriptor_hash: hash,
        payload_hash: hash,
        availability_hash: hash,
    };
    [
        BlockMessage::NativeLane(LaneMessageEnvelopeV1 {
            version: LANE_MESSAGE_VERSION_V1,
            message: LaneMessageV1::TimeoutVote(LaneTimeoutVoteV1 {
                body: LaneTimeoutBodyV1 {
                    round,
                    highest_prepare: None,
                },
                share: share.clone(),
            }),
        }),
        BlockMessage::NativeLaneDecision(Box::new(LaneDecisionV1 {
            manifest: LaneManifestV1 {
                value,
                layout: minimal_rs16_layout(),
                chunk_root: hash,
                byte_len: 1,
                chunk_count: 2,
            },
            commit_qc: LaneQcV1 {
                statement: LaneVoteStatementV1 {
                    round,
                    phase: LanePhaseV1::Commit,
                    value,
                },
                shares: (0..3)
                    .map(|signer| LaneSignatureShareV1 {
                        signer,
                        ..share.clone()
                    })
                    .collect(),
            },
        })),
    ]
}

#[test]
fn native_wire_roundtrips_canonical_control_and_decision_without_legacy_routing() {
    use iroha_p2p::network::message::{ClassifyTopic, Topic};
    for (message, tag, kind) in native_wire_classification_fixtures()
        .into_iter()
        .zip([11_u32, 12])
        .zip([
            super::FairV2IngressMessageKind::NativeLane,
            super::FairV2IngressMessageKind::NativeLaneDecision,
        ])
        .map(|((message, tag), kind)| (message, tag, kind))
    {
        assert!(message.is_native_lane());
        assert!(!message.is_lane_local());
        assert!(!message.is_live_auxiliary());
        assert_eq!(message.priority(), iroha_p2p::Priority::High);
        assert_eq!(
            super::FairV2IngressMessageKind::classify(&message),
            Some(kind)
        );
        assert_eq!(
            super::FairV2IngressClass::classify_message(&message),
            super::FairV2IngressClass::Progress
        );
        let encoded = message.encode();
        assert_eq!(u32::from_le_bytes(encoded[..4].try_into().unwrap()), tag);
        let frame = super::message::BlockMessageWire::try_preencoded(Arc::new(message)).unwrap();
        let bytes = frame.encode();
        let (decoded, consumed) =
            <super::message::BlockMessageWire as norito::core::DecodeFromSlice>::decode_from_slice(
                &bytes,
            )
            .unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded.encode(), bytes);
        assert_eq!(decoded.as_message().encode(), encoded);
        let network = crate::NetworkMessage::SumeragiBlock(Arc::new(decoded));
        assert_eq!(network.topic(), Topic::Consensus);
        let network_bytes = norito::core::to_bytes(&network).unwrap();
        let decoded: crate::NetworkMessage =
            norito::core::decode_from_bytes(&network_bytes).unwrap();
        assert_eq!(decoded.topic(), Topic::Consensus);
        assert_eq!(norito::core::to_bytes(&decoded).unwrap(), network_bytes);
    }
}

#[test]
fn native_wire_wrong_revision_cannot_enter_cached_or_nested_frames() {
    use iroha_p2p::network::message::{ClassifyTopic, Topic};
    let [BlockMessage::NativeLane(mut envelope), _] = native_wire_classification_fixtures() else {
        unreachable!("control fixture")
    };
    envelope.version += 1;
    let message = BlockMessage::NativeLane(envelope);
    assert!(super::message::BlockMessageWire::try_preencoded(Arc::new(message.clone())).is_err());
    // Raw DTO serialization is deliberately untrusted; cached network decoding
    // must still reject it before any native consumer can receive the value.
    let raw = norito::core::to_bytes(&message).unwrap();
    assert!(
        <super::message::BlockMessageWire as norito::core::DecodeFromSlice>::decode_from_slice(
            &raw
        )
        .is_err()
    );
    let network = crate::NetworkMessage::SumeragiBlock(Arc::new(
        super::message::BlockMessageWire::new(message),
    ));
    assert_eq!(network.topic(), Topic::Other);
    assert!(norito::core::to_bytes(&network).is_err());
}

#[test]
fn native_wire_ingress_stays_closed_without_a_connected_native_consumer() {
    let sender = validator_peers(1).pop().unwrap();
    let ingress = super::FairV2Ingress::new(8, 65_536, 65_536, 16_384, 16_384);
    ingress.configure_roster([sender.clone()]).unwrap();
    ingress.open().unwrap();
    for message in native_wire_classification_fixtures() {
        let original = message.encode();
        let original_ordinal = ingress.state.lock().last_admission_ordinal;
        let Err(super::FairV2IngressPushError::Rejected(rejected)) = ingress.try_push(
            InboundBlockMessage::from_authenticated_peer(message, sender.clone()),
        ) else {
            panic!("native ingress must remain closed until the sole runner cutover");
        };
        assert_eq!(
            rejected.reason,
            super::FairV2IngressRejectReason::UnsupportedEnvelope
        );
        assert_eq!(rejected.inbound.message().encode(), original);
        assert_eq!(rejected.inbound.sender(), &sender);
        assert_eq!(
            ingress.state.lock().last_admission_ordinal,
            original_ordinal
        );
        assert_eq!(ingress.len(), 0);
        assert!(ingress.state.lock().open);
    }
}
