// Real NetworkMessage/BlockMessageWire classifier and decoder coverage. These
// size fixtures are untrusted control evidence, not cryptographic authority.

fn native_network_controls(committee: usize) -> Vec<BlockMessage> {
    use iroha_data_model::block::{consensus_v2 as global, lane_consensus::*};
    let quorum = 2 * ((committee - 1) / 3) + 1;
    let hash = Hash::new(b"native-network-control");
    let round = LaneRoundV1 {
        instance_id: hash,
        lane_height: 1,
        voting_view: 1,
    };
    let value = LaneValueRefV1 {
        instance_id: hash,
        admitted_binding_hash: hash,
        kind: LaneValueKindV1::AtomicGroup,
        origin_view: 1,
        origin_producer: 0,
        descriptor_hash: hash,
        payload_hash: hash,
        availability_hash: hash,
    };
    let shares = (0..quorum)
        .map(|signer| LaneSignatureShareV1 {
            signer: u32::try_from(signer).unwrap(),
            signature: vec![0xA5; crate::lane_consensus::LANE_BLS_PROOF_BYTES],
        })
        .collect::<Vec<_>>();
    let qc = LaneQcV1 {
        statement: LaneVoteStatementV1 {
            round,
            phase: LanePhaseV1::Prepare,
            value,
        },
        shares,
    };
    let tc = LaneTcV1 {
        round,
        votes: qc
            .shares
            .iter()
            .map(|share| LaneTimeoutVoteV1 {
                body: LaneTimeoutBodyV1 {
                    round,
                    highest_prepare: Some(qc.clone()),
                },
                share: share.clone(),
            })
            .collect(),
    };
    let manifest = LaneManifestV1 {
        value,
        layout: global::recommended_data_availability_layout(),
        chunk_root: hash,
        byte_len: 1,
        chunk_count: 1,
    };
    let controls = vec![
        LaneMessageV1::Proposal(LaneProposalV1 {
            body: LaneProposalBodyV1 {
                round,
                proposer: 0,
                manifest,
                justification: LaneJustificationV1::Timeout(tc.clone()),
            },
            signature: qc.shares[0].signature.clone(),
        }),
        LaneMessageV1::Vote(LaneVoteV1 {
            statement: qc.statement,
            share: qc.shares[0].clone(),
        }),
        LaneMessageV1::QuorumCertificate(qc.clone()),
        LaneMessageV1::TimeoutVote(tc.votes[0].clone()),
        LaneMessageV1::TimeoutCertificate(tc),
    ];
    let mut blocks = controls
        .into_iter()
        .map(|message| {
            BlockMessage::NativeLane(LaneMessageEnvelopeV1 {
                version: LANE_MESSAGE_VERSION_V1,
                message,
            })
        })
        .collect::<Vec<_>>();
    let mut commit_qc = qc;
    commit_qc.statement.phase = LanePhaseV1::Commit;
    blocks.push(BlockMessage::NativeLaneDecision(Box::new(LaneDecisionV1 {
        manifest,
        commit_qc,
    })));
    blocks
}

#[test]
fn native_network_controls_classify_and_decode_under_every_supported_layout() {
    use iroha_data_model::block::consensus_v2::MAX_FAULTS_PER_HEIGHT;
    // Exercise the actual nested wrappers, including the maximum nested TC
    // for every supported 3f+1 committee. Direct driver delivery cannot catch
    // rejection by the raw classifier used before P2P credit admission.
    let mut observed_layouts = std::collections::BTreeSet::new();
    for faults in 1..=MAX_FAULTS_PER_HEIGHT {
        for block in native_network_controls(3 * faults + 1) {
            let message = NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(block)));
            assert_eq!(raw_network_topic(&message), NetworkTopic::Consensus);
            assert_network_admission(&message, iroha_p2p::TransportAdmissionClass::Lane);
            for requested_flags in [
                0,
                ncore::header_flags::COMPACT_LEN,
                ncore::header_flags::PACKED_SEQ,
                ncore::header_flags::PACKED_SEQ | ncore::header_flags::COMPACT_LEN,
                ncore::header_flags::PACKED_STRUCT,
                ncore::header_flags::PACKED_STRUCT | ncore::header_flags::COMPACT_LEN,
                ncore::header_flags::PACKED_STRUCT | ncore::header_flags::PACKED_SEQ,
                ncore::header_flags::PACKED_STRUCT
                    | ncore::header_flags::PACKED_SEQ
                    | ncore::header_flags::COMPACT_LEN,
                ncore::header_flags::PACKED_STRUCT
                    | ncore::header_flags::COMPACT_LEN
                    | ncore::header_flags::FIELD_BITSET,
                ncore::header_flags::PACKED_STRUCT
                    | ncore::header_flags::PACKED_SEQ
                    | ncore::header_flags::COMPACT_LEN
                    | ncore::header_flags::FIELD_BITSET,
            ] {
                let encoded = {
                    let _layout = ncore::DecodeFlagsGuard::enter(requested_flags);
                    ncore::to_bytes(&message).unwrap()
                };
                let view = ncore::from_bytes_view(&encoded).unwrap();
                let (_, remaining) = super::inbound_enum_parts(view.as_bytes()).unwrap();
                let nested = super::inbound_owned_enum_field(remaining, view.flags()).unwrap();
                let (_, _, nested_flags) = super::inbound_sumeragi_enum_field(nested).unwrap();
                // Norito clears unused PACKED_SEQ/FIELD_BITSET flags for a
                // shape that has no corresponding dynamic fields. Compare the
                // complete nested frame to its independently encoded source.
                let expected_nested = {
                    let _layout = ncore::DecodeFlagsGuard::enter(requested_flags);
                    let NetworkMessage::SumeragiBlock(wire) = &message else {
                        unreachable!()
                    };
                    ncore::to_bytes(wire.as_message()).unwrap()
                };
                assert_eq!(nested, expected_nested);
                observed_layouts.insert(nested_flags & ncore::supported_header_flags());
                assert_eq!(
                    NetworkMessage::inbound_topic(view.as_bytes(), view.flags()).unwrap(),
                    Some(message.topic())
                );
                let limits = NetworkMessage::inbound_decode_limits(
                    view.as_bytes(),
                    encoded.len(),
                    view.flags(),
                )
                .unwrap()
                .expect("Native controls have explicit decode bounds");
                let decoded = ncore::decode_from_bytes_with_limits::<NetworkMessage>(
                    &encoded, limits,
                )
                .expect("all valid Native control geometries must pass the real bounded decoder");
                let reencoded = {
                    let _layout = ncore::DecodeFlagsGuard::enter(requested_flags);
                    ncore::to_bytes(&decoded).unwrap()
                };
                assert_eq!(reencoded, encoded);
                assert!(matches!(
                    NetworkMessage::inbound_decode_limits(
                        view.as_bytes(),
                        super::MAX_SUMERAGI_V2_CONTROL_NETWORK_FRAME_BYTES + 1,
                        view.flags()
                    ),
                    Err(ncore::Error::ArchiveLengthExceeded { .. })
                ));
            }
        }
    }
    assert_eq!(
        observed_layouts.len(),
        10,
        "all ten declared layouts must be exercised"
    );
}

#[test]
fn native_network_control_raw_gate_rejects_unknown_version_kind_and_truncation() {
    use iroha_data_model::block::lane_consensus::LANE_MESSAGE_VERSION_V1;
    for flags in [
        0,
        ncore::header_flags::COMPACT_LEN,
        ncore::header_flags::PACKED_STRUCT
            | ncore::header_flags::FIELD_BITSET
            | ncore::header_flags::COMPACT_LEN,
    ] {
        let mut block = native_network_controls(4).remove(3);
        let BlockMessage::NativeLane(envelope) = &mut block else {
            unreachable!()
        };
        envelope.version = LANE_MESSAGE_VERSION_V1 + 1;
        let framed = {
            let _layout = ncore::DecodeFlagsGuard::enter(flags);
            ncore::to_bytes(&block).unwrap()
        };
        assert_eq!(
            super::inbound_sumeragi_topic(&framed).unwrap(),
            NetworkTopic::Other
        );
        let BlockMessage::NativeLane(envelope) = &mut block else {
            unreachable!()
        };
        envelope.version = LANE_MESSAGE_VERSION_V1;
        let (mut bare, actual_flags) = {
            let _layout = ncore::DecodeFlagsGuard::enter(flags);
            norito::codec::encode_with_header_flags(&block)
        };
        let (_, rest) = super::inbound_enum_parts(&bare).unwrap();
        let native = super::inbound_enum_field(rest, actual_flags).unwrap();
        let (_, inner) = super::inbound_two_field_struct(native, actual_flags, 2).unwrap();
        let offset = inner.as_ptr() as usize - bare.as_ptr() as usize;
        bare[offset..offset + 4].copy_from_slice(&5_u32.to_le_bytes());
        let framed =
            ncore::frame_bare_with_header_flags::<BlockMessage>(&bare, actual_flags).unwrap();
        assert!(super::inbound_sumeragi_topic(&framed).is_err());
        assert!(super::inbound_native_lane_topic(&[], actual_flags).is_err());
        assert!(super::inbound_sumeragi_topic(&framed[..framed.len() - 1]).is_err());
    }
}

#[test]
fn native_network_control_decoder_bounds_nested_signature_allocation() {
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    let mut block = native_network_controls(4).remove(3);
    let BlockMessage::NativeLane(envelope) = &mut block else {
        unreachable!()
    };
    let LaneMessageV1::TimeoutVote(vote) = &mut envelope.message else {
        unreachable!()
    };
    vote.share
        .signature
        .resize(crate::lane_consensus::LANE_BLS_PROOF_BYTES + 1, 0xA5);
    let message = NetworkMessage::SumeragiBlock(Arc::new(BlockMessageWire::new(block)));
    let encoded = ncore::to_bytes(&message).unwrap();
    let view = ncore::from_bytes_view(&encoded).unwrap();
    let limits =
        NetworkMessage::inbound_decode_limits(view.as_bytes(), encoded.len(), view.flags())
            .unwrap()
            .unwrap();
    assert!(
        ncore::decode_from_bytes_with_limits::<NetworkMessage>(&encoded, limits).is_err(),
        "oversized nested signature must not allocate through the Native decode policy"
    );
}
