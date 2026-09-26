//! Canonical native lane DTO controls; no raw structure grants live authority.

use super::*;
use iroha_crypto::{Algorithm, HashOf, KeyPair, Signature};
use std::sync::OnceLock;

fn fixture() -> (FrozenLaneConsensusContextV1, Vec<KeyPair>) {
    static FIXTURE: OnceLock<(FrozenLaneConsensusContextV1, Vec<KeyPair>)> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let mut validators = (1..=4)
                .map(|seed| {
                    let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap();
                    let pop = iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap();
                    (PeerId::new(key.public_key().clone()), key, pop)
                })
                .collect::<Vec<_>>();
            validators.sort_by(|left, right| left.0.cmp(&right.0));
            (
                FrozenLaneConsensusContextV1 {
                    network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                        Hash::new(b"model lane fixture network"),
                    )),
                    protocol_version: wire::PROTOCOL_VERSION,
                    opening_global_height: 41,
                    opening_global_context_id: wire::HeightContextId(
                        HashOf::from_untyped_unchecked(Hash::new(b"opening context")),
                    ),
                    admitted_binding_hash: Hash::new(b"pinned binding"),
                    admission_priority: QueuePlanAdmissionPriorityV1::new(40, 0).unwrap(),
                    epoch: 7,
                    mode: wire::ConsensusMode::Permissioned,
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(2),
                    lane_incarnation: Hash::new(b"route incarnation"),
                    next_lane_height: 3,
                    predecessor_height: 2,
                    predecessor_hash: Some(Hash::new(b"predecessor descriptor")),
                    predecessor_applied_global_height: 40,
                    committee: validators.iter().map(|entry| entry.0.clone()).collect(),
                    validator_set_pops: validators.iter().map(|entry| entry.2.clone()).collect(),
                    nexus_amx_context_hash: Hash::new(b"AMX policy"),
                    execution_policy_hash: Hash::new(b"execution policy"),
                    da_layout: wire::recommended_data_availability_layout(),
                    leader_seed: [7; Hash::LENGTH],
                },
                validators.into_iter().map(|entry| entry.1).collect(),
            )
        })
        .clone()
}

fn decision() -> (FrozenLaneConsensusContextV1, LaneDecisionV1) {
    let (frozen, keys) = fixture();
    let value = LaneValueRefV1 {
        instance_id: Hash::new(b"untrusted fixture instance, not finality"),
        admitted_binding_hash: frozen.admitted_binding_hash,
        kind: LaneValueKindV1::Execution,
        origin_view: 0,
        origin_producer: 0,
        descriptor_hash: Hash::new(b"canonical input descriptor"),
        payload_hash: Hash::new(b"canonical input"),
        availability_hash: lane_availability_hash(
            frozen.da_layout,
            Hash::new(b"chunk root"),
            8,
            wire::expected_encoded_chunk_count(8, frozen.da_layout).unwrap(),
        )
        .unwrap(),
    };
    let statement = LaneVoteStatementV1 {
        round: LaneRoundV1 {
            instance_id: value.instance_id,
            lane_height: frozen.next_lane_height,
            voting_view: 2,
        },
        phase: LanePhaseV1::Commit,
        value: value.clone(),
    };
    let bytes = statement.signature_preimage().unwrap();
    let shares = (0..3)
        .map(|signer| LaneSignatureShareV1 {
            signer,
            signature: Signature::new(keys[signer as usize].private_key(), &bytes)
                .payload()
                .to_vec(),
        })
        .collect();
    let manifest = LaneManifestV1 {
        value,
        layout: frozen.da_layout,
        chunk_root: Hash::new(b"chunk root"),
        byte_len: 8,
        chunk_count: wire::expected_encoded_chunk_count(8, frozen.da_layout).unwrap(),
    };
    (
        frozen,
        LaneDecisionV1 {
            manifest,
            commit_qc: LaneQcV1 { statement, shares },
        },
    )
}

#[test]
fn pure_priority_context_and_full_set_roundtrip_without_admission_authority() {
    let (frozen, _) = fixture();
    frozen.validate().unwrap();
    assert_eq!(frozen.minimum_signer_count(), Ok(3));
    let set = LaneConsensusContextsV1::new(vec![frozen.clone()]).unwrap();
    let bytes = norito::encode_canonical(&set).unwrap();
    let decoded: LaneConsensusContextsV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, set);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
    let json = norito::json::to_json(&set).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneConsensusContextsV1>(&json).unwrap(),
        set
    );
    assert!(norito::json::from_str::<LaneConsensusContextsV1>("{}").is_err());
    assert!(QueuePlanAdmissionPriorityV1::new(0, 0).is_err());
    assert!(QueuePlanAdmissionPriorityV1::new(1, MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK).is_err());
    let end =
        QueuePlanAdmissionPriorityV1::new(1, MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK - 1).unwrap();
    assert!(end < QueuePlanAdmissionPriorityV1::new(2, 0).unwrap());
    let mut changed = frozen.clone();
    changed.validator_set_pops[0][0] ^= 1;
    assert!(changed.validate().is_err());
    let mut changed = frozen.clone();
    changed.predecessor_applied_global_height = 42;
    assert!(changed.canonical_hash().is_err());
    assert!(LaneConsensusContextsV1::new(vec![frozen.clone(), frozen]).is_err());
}

#[test]
fn value_origin_requires_a_published_view_and_frozen_committee_member() {
    let (frozen, decision) = decision();
    let round = decision.commit_qc.statement.round;
    let committee_len = frozen.committee.len();
    let mut value = decision.value().clone();
    value_shape(&value, round, committee_len).expect("original origin is in range");
    value.origin_view = round.voting_view + 1;
    assert!(value_shape(&value, round, committee_len).is_err());
    value.origin_view = round.voting_view;
    value_shape(&value, round, committee_len).expect("current voting view is in range");
    value.origin_producer = committee_len as u32;
    assert!(value_shape(&value, round, committee_len).is_err());
    value.origin_producer = (committee_len - 1) as u32;
    value_shape(&value, round, committee_len).expect("last committee member is in range");
}

#[test]
fn native_decision_roundtrips_and_binds_exact_commit_value_and_rs16() {
    let (frozen, decision) = decision();
    decision.validate_shape(&frozen).unwrap();
    assert_eq!(decision.value(), &decision.commit_qc.statement.value);
    let bytes = norito::encode_canonical(&decision).unwrap();
    let decoded: LaneDecisionV1 = norito::decode_canonical(&bytes).unwrap();
    assert_eq!(decoded, decision);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
    let json = norito::json::to_json(&decision).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneDecisionV1>(&json).unwrap(),
        decision
    );
    let mut changed = decision.clone();
    changed.commit_qc.statement.phase = LanePhaseV1::Prepare;
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.manifest.value.payload_hash = Hash::new(b"different body");
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.manifest.layout.chunk_size_bytes /= 2;
    changed.manifest.chunk_count =
        wire::expected_encoded_chunk_count(changed.manifest.byte_len, changed.manifest.layout)
            .unwrap();
    assert!(
        changed.validate_shape(&frozen).is_err(),
        "even valid geometry must match signed frozen layout"
    );
    let mut changed = decision.clone();
    changed.manifest.chunk_count += 1;
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.commit_qc.statement.round.lane_height += 1;
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.manifest.value.admitted_binding_hash = Hash::new(b"another group");
    changed.commit_qc.statement.value = changed.manifest.value.clone();
    assert!(changed.validate_shape(&frozen).is_err());
    for count in [0, 1, 2, 4] {
        let mut changed = decision.clone();
        changed.commit_qc.shares = (0..count)
            .map(|index| LaneSignatureShareV1 {
                signer: index,
                signature: vec![0; 96],
            })
            .collect();
        assert!(
            changed.validate_shape(&frozen).is_err(),
            "exact quorum rejects {count}"
        );
    }
    let mut changed = decision.clone();
    changed.commit_qc.shares.swap(0, 1);
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.commit_qc.shares[1] = changed.commit_qc.shares[0].clone();
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision.clone();
    changed.commit_qc.shares[0].signature.pop();
    assert!(changed.validate_shape(&frozen).is_err());
    let mut changed = decision;
    changed.commit_qc.shares[2].signer = 4;
    assert!(changed.validate_shape(&frozen).is_err());
}

#[test]
fn model_shape_validation_does_not_grant_signature_or_finality_authority() {
    let (frozen, mut decision) = decision();
    let bytes = decision.commit_qc.statement.signature_preimage().unwrap();
    let valid = Signature::try_from_bytes(&decision.commit_qc.shares[0].signature).unwrap();
    valid
        .verify(frozen.committee[0].public_key(), &bytes)
        .unwrap();
    // Reuse a valid signature under the wrong key: exact shape still holds.
    decision.commit_qc.shares[0].signature = decision.commit_qc.shares[1].signature.clone();
    decision.validate_shape(&frozen).unwrap();
    assert!(
        Signature::try_from_bytes(&decision.commit_qc.shares[0].signature)
            .unwrap()
            .verify(frozen.committee[0].public_key(), &bytes)
            .is_err()
    );
    // The Core authenticator tests must reject this before any reducer event.
}

#[test]
fn canonical_domains_bind_value_origin_but_separate_voting_round_and_phase() {
    let (_, decision) = decision();
    let value = decision.value();
    let mut value_preimage = b"iroha:lane-reducer:value:v1\0".to_vec();
    value_preimage.extend(norito::encode_canonical(value).unwrap());
    assert_eq!(value.subject_hash().unwrap(), Hash::new(&value_preimage));
    let mut vote = decision.commit_qc.statement.clone();
    let initial = vote.signature_preimage().unwrap();
    let mut expected = b"iroha:lane-reducer:vote:v1\0".to_vec();
    expected.extend(norito::encode_canonical(&vote).unwrap());
    assert_eq!(initial, expected);
    vote.round.voting_view += 1;
    assert_ne!(vote.signature_preimage().unwrap(), initial);
    assert_eq!(
        vote.value.subject_hash().unwrap(),
        value.subject_hash().unwrap()
    );
    vote.phase = LanePhaseV1::Prepare;
    assert_ne!(vote.signature_preimage().unwrap(), initial);
    let mut changed = value.clone();
    changed.origin_view += 1;
    assert_ne!(
        changed.subject_hash().unwrap(),
        value.subject_hash().unwrap()
    );
    let mut changed = value.clone();
    changed.kind = LaneValueKindV1::AtomicGroup;
    assert_ne!(
        changed.subject_hash().unwrap(),
        value.subject_hash().unwrap()
    );
    let (frozen, _) = fixture();
    let frame = norito::encode_canonical(&frozen).unwrap();
    assert_eq!(
        frozen.canonical_hash().unwrap(),
        Hash::new_from_chunks(&[b"iroha:lane-consensus:frozen-context:v1\0", &frame])
    );
}

fn timeout_fixture() -> LaneTcV1 {
    let (_, decision) = decision();
    let mut high = decision.commit_qc;
    high.statement.phase = LanePhaseV1::Prepare; // Only shape is under test below.
    let round = high.statement.round;
    LaneTcV1 {
        round,
        votes: (0..3)
            .map(|signer| LaneTimeoutVoteV1 {
                body: LaneTimeoutBodyV1 {
                    round,
                    highest_prepare: if signer == 0 {
                        Some(high.clone())
                    } else {
                        None
                    },
                },
                share: LaneSignatureShareV1 {
                    signer,
                    signature: vec![0; 96],
                },
            })
            .collect(),
    }
}

#[test]
fn timeout_signing_commits_highest_statement_and_preserves_full_evidence() {
    let tc = timeout_fixture();
    let original = tc.votes[0].body.clone();
    let expected = original.signature_preimage().unwrap();
    let mut changed = original.clone();
    changed.highest_prepare.as_mut().unwrap().shares[0].signature[0] ^= 1;
    assert_eq!(
        changed.signature_preimage().unwrap(),
        expected,
        "QC signer evidence does not retag a timeout vote"
    );
    assert_ne!(
        norito::encode_canonical(&changed).unwrap(),
        norito::encode_canonical(&original).unwrap()
    );
    let mut changed = original.clone();
    changed.highest_prepare = None;
    assert_ne!(
        changed.signature_preimage().unwrap(),
        expected,
        "cannot strip authenticated highest Prepare"
    );
    let body_json = norito::json::to_json(&changed).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneTimeoutBodyV1>(&body_json).unwrap(),
        changed
    );
    let only_round = format!(
        "{{\"round\":{}}}",
        norito::json::to_json(&changed.round).unwrap()
    );
    assert!(norito::json::from_str::<LaneTimeoutBodyV1>(&only_round).is_err());
}

#[test]
fn native_message_shapes_reject_cross_instance_mixed_round_and_future_high() {
    let tc = timeout_fixture();
    LaneMessageV1::TimeoutCertificate(tc.clone())
        .validate_shape(4)
        .unwrap();
    assert!(
        LaneMessageV1::TimeoutCertificate(tc.clone())
            .validate_shape(5)
            .is_err()
    );
    let mut changed = tc.clone();
    changed.votes[1].body.round.voting_view += 1;
    assert!(
        LaneMessageV1::TimeoutCertificate(changed)
            .validate_shape(4)
            .is_err()
    );
    let mut changed = tc.clone();
    changed.votes[1].share.signer = 0;
    assert!(
        LaneMessageV1::TimeoutCertificate(changed)
            .validate_shape(4)
            .is_err()
    );
    let mut changed = tc.clone();
    changed.votes[0]
        .body
        .highest_prepare
        .as_mut()
        .unwrap()
        .statement
        .round
        .voting_view += 1;
    assert!(
        LaneMessageV1::TimeoutCertificate(changed)
            .validate_shape(4)
            .is_err()
    );
    let mut changed = tc.clone();
    changed.votes[0]
        .body
        .highest_prepare
        .as_mut()
        .unwrap()
        .statement
        .phase = LanePhaseV1::Commit;
    assert!(
        LaneMessageV1::TimeoutCertificate(changed)
            .validate_shape(4)
            .is_err()
    );
    let mut changed = tc;
    changed.votes[0]
        .body
        .highest_prepare
        .as_mut()
        .unwrap()
        .statement
        .round
        .instance_id = Hash::new(b"foreign");
    assert!(
        LaneMessageV1::TimeoutCertificate(changed)
            .validate_shape(4)
            .is_err()
    );
}

#[test]
fn every_message_roundtrips_canonical_norito_and_json_with_bounded_decode() {
    let (_, decision) = decision();
    let tc = timeout_fixture();
    let mut round = tc.round;
    round.voting_view += 1;
    let body = LaneProposalBodyV1 {
        round,
        proposer: 0,
        manifest: decision.manifest.clone(),
        justification: LaneJustificationV1::Timeout(tc.clone()),
    };
    let proposal_bytes = body.signature_preimage().unwrap();
    assert!(proposal_bytes.starts_with(b"iroha:lane-reducer:proposal:v1\0"));
    let vote = LaneVoteV1 {
        statement: decision.commit_qc.statement.clone(),
        share: decision.commit_qc.shares[0].clone(),
    };
    let messages = vec![
        LaneMessageV1::Proposal(LaneProposalV1 {
            body,
            signature: vec![0; 96],
        }),
        LaneMessageV1::Vote(vote),
        LaneMessageV1::QuorumCertificate(decision.commit_qc),
        LaneMessageV1::TimeoutVote(tc.votes[0].clone()),
        LaneMessageV1::TimeoutCertificate(tc),
    ];
    for message in messages {
        let envelope = LaneMessageEnvelopeV1 {
            version: LANE_MESSAGE_VERSION_V1,
            message,
        };
        let bytes = norito::encode_canonical(&envelope).unwrap();
        let decoded = LaneMessageEnvelopeV1::decode_canonical(&bytes, bytes.len(), 4).unwrap();
        assert_eq!(decoded, envelope);
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
        assert!(LaneMessageEnvelopeV1::decode_canonical(&bytes, bytes.len() - 1, 4).is_err());
        let json = norito::json::to_json(&envelope).unwrap();
        assert_eq!(
            norito::json::from_str::<LaneMessageEnvelopeV1>(&json).unwrap(),
            envelope
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(LaneMessageEnvelopeV1::decode_canonical(&trailing, trailing.len(), 4).is_err());
        let mut future = envelope;
        future.version += 1;
        let bytes = norito::encode_canonical(&future).unwrap();
        assert!(LaneMessageEnvelopeV1::decode_canonical(&bytes, bytes.len(), 4).is_err());
    }
}
#[test]
fn canonical_lane_schema_frame_vectors() {
    use norito::schema::identity::{NoritoSchema, frame_hash};
    macro_rules! check {
        ($ty:ty, $name:literal, $hash:expr) => {
            assert_eq!(<$ty as NoritoSchema>::nominal_name(), $name);
            assert_eq!(frame_hash::<$ty>(), $hash);
        };
    }
    check!(
        QueuePlanAdmissionPriorityV1,
        "iroha_data_model::block::lane_consensus::QueuePlanAdmissionPriorityV1",
        [
            0x47, 0xe8, 0x55, 0x81, 0x4d, 0x02, 0xdc, 0x93, 0xa3, 0x1b, 0x90, 0xc4, 0x5a, 0xde,
            0xb9, 0xd0
        ]
    );
    check!(
        FrozenLaneConsensusContextV1,
        "iroha_data_model::block::lane_consensus::FrozenLaneConsensusContextV1",
        [
            0x8c, 0xeb, 0x0e, 0x2c, 0x3d, 0xdb, 0x0f, 0x04, 0x38, 0x48, 0x67, 0xb7, 0x38, 0x97,
            0xc3, 0xce
        ]
    );
    check!(
        LaneConsensusContextsV1,
        "iroha_data_model::block::lane_consensus::LaneConsensusContextsV1",
        [
            0x37, 0x6b, 0x93, 0xb5, 0x8d, 0x41, 0x74, 0x8d, 0x5f, 0x68, 0x0e, 0xd2, 0x83, 0xac,
            0x1b, 0xbc
        ]
    );
    check!(
        LaneRoundV1,
        "iroha_data_model::block::lane_consensus::LaneRoundV1",
        [
            0x7c, 0xe3, 0xd5, 0x67, 0x9f, 0x70, 0x30, 0x6f, 0x54, 0x84, 0xd2, 0x27, 0xa9, 0x5e,
            0xe8, 0x5b
        ]
    );
    check!(
        LaneValueKindV1,
        "iroha_data_model::block::lane_consensus::LaneValueKindV1",
        [
            0x49, 0x76, 0xd5, 0xac, 0x41, 0xec, 0x39, 0xfd, 0x56, 0xbd, 0xb5, 0x4f, 0xa1, 0x98,
            0x36, 0x58
        ]
    );
    check!(
        LaneValueRefV1,
        "iroha_data_model::block::lane_consensus::LaneValueRefV1",
        [
            0x42, 0x89, 0xe1, 0xa3, 0xc7, 0x34, 0xc0, 0x14, 0xbf, 0xff, 0x22, 0x3f, 0x3b, 0xdf,
            0x49, 0xe1
        ]
    );
    check!(
        LaneManifestV1,
        "iroha_data_model::block::lane_consensus::LaneManifestV1",
        [
            0x22, 0xb9, 0x42, 0xcf, 0x0a, 0x9e, 0x84, 0x8f, 0x97, 0xeb, 0x47, 0x62, 0x18, 0xd4,
            0xd3, 0xbb
        ]
    );
    check!(
        LanePhaseV1,
        "iroha_data_model::block::lane_consensus::LanePhaseV1",
        [
            0x1f, 0x48, 0x2e, 0x4f, 0xac, 0xff, 0xed, 0x99, 0xa5, 0x76, 0x23, 0xbc, 0x92, 0x94,
            0xb3, 0x6c
        ]
    );
    check!(
        LaneVoteStatementV1,
        "iroha_data_model::block::lane_consensus::LaneVoteStatementV1",
        [
            0x45, 0xad, 0x95, 0xa2, 0xfe, 0x81, 0xe5, 0x05, 0x5d, 0xf3, 0xac, 0x68, 0x0b, 0x3b,
            0x2a, 0xd1
        ]
    );
    check!(
        LaneSignatureShareV1,
        "iroha_data_model::block::lane_consensus::LaneSignatureShareV1",
        [
            0xad, 0x18, 0xac, 0xfd, 0xa6, 0xc9, 0xb7, 0x86, 0x6a, 0xfb, 0xf9, 0x8b, 0x9f, 0x0d,
            0x54, 0x82
        ]
    );
    check!(
        LaneVoteV1,
        "iroha_data_model::block::lane_consensus::LaneVoteV1",
        [
            0xb7, 0x08, 0xd2, 0x23, 0x53, 0x00, 0xf4, 0x06, 0x99, 0xab, 0xab, 0x88, 0x07, 0xb7,
            0x04, 0xaf
        ]
    );
    check!(
        LaneQcV1,
        "iroha_data_model::block::lane_consensus::LaneQcV1",
        [
            0x9d, 0x6d, 0xcf, 0x16, 0x23, 0x11, 0x04, 0x9d, 0x46, 0x14, 0x6a, 0x63, 0xa8, 0x65,
            0x0f, 0x2c
        ]
    );
    check!(
        LaneTimeoutBodyV1,
        "iroha_data_model::block::lane_consensus::LaneTimeoutBodyV1",
        [
            0xf9, 0xfd, 0x74, 0xbc, 0x09, 0x24, 0x70, 0xfb, 0x4c, 0x35, 0xdf, 0xb0, 0x7d, 0x18,
            0xf8, 0xd0
        ]
    );
    check!(
        LaneTimeoutVoteV1,
        "iroha_data_model::block::lane_consensus::LaneTimeoutVoteV1",
        [
            0xd5, 0x5a, 0x43, 0xb7, 0x1c, 0x1c, 0x64, 0x5f, 0x12, 0xdb, 0x32, 0xef, 0x84, 0x8b,
            0x61, 0x06
        ]
    );
    check!(
        LaneTcV1,
        "iroha_data_model::block::lane_consensus::LaneTcV1",
        [
            0x28, 0xaf, 0x19, 0xc8, 0x49, 0x9b, 0x37, 0x26, 0xf7, 0x41, 0x45, 0xfa, 0x58, 0xe7,
            0xd6, 0x76
        ]
    );
    check!(
        LaneJustificationV1,
        "iroha_data_model::block::lane_consensus::LaneJustificationV1",
        [
            0x3f, 0x85, 0x79, 0xd1, 0x48, 0x69, 0xdb, 0xf4, 0xc9, 0x0c, 0x8c, 0xe9, 0xd2, 0x73,
            0x4b, 0x6e
        ]
    );
    check!(
        LaneProposalBodyV1,
        "iroha_data_model::block::lane_consensus::LaneProposalBodyV1",
        [
            0x44, 0x75, 0x57, 0xf8, 0x32, 0xe9, 0x0e, 0x11, 0xf1, 0xd8, 0x29, 0xfb, 0xe5, 0x82,
            0x29, 0xa1
        ]
    );
    check!(
        LaneProposalV1,
        "iroha_data_model::block::lane_consensus::LaneProposalV1",
        [
            0x4e, 0xe9, 0x73, 0x6e, 0x3d, 0xe9, 0x89, 0x70, 0x87, 0x7d, 0x97, 0x20, 0x76, 0x26,
            0x60, 0xb9
        ]
    );
    check!(
        LaneMessageV1,
        "iroha_data_model::block::lane_consensus::LaneMessageV1",
        [
            0x27, 0xe9, 0x55, 0xe1, 0x38, 0xf8, 0x8e, 0x87, 0x85, 0x28, 0x9e, 0x62, 0xfc, 0xd3,
            0xd3, 0x97
        ]
    );
    check!(
        LaneMessageEnvelopeV1,
        "iroha_data_model::block::lane_consensus::LaneMessageEnvelopeV1",
        [
            0x6a, 0x38, 0x88, 0x96, 0x27, 0xcc, 0x86, 0xbf, 0x42, 0xcc, 0x44, 0x15, 0x25, 0x49,
            0xcd, 0x50
        ]
    );
    check!(
        LaneDecisionV1,
        "iroha_data_model::block::lane_consensus::LaneDecisionV1",
        [
            0x54, 0xe0, 0x7c, 0xb1, 0xaa, 0x16, 0x79, 0xab, 0x82, 0xe9, 0x8f, 0x72, 0xef, 0xfe,
            0xd6, 0x7b
        ]
    );
    check!(
        (LaneRoundV1, Option<LaneVoteStatementV1>),
        "(iroha_data_model::block::lane_consensus::LaneRoundV1, core::option::Option<iroha_data_model::block::lane_consensus::LaneVoteStatementV1>)",
        [
            0x69, 0x8d, 0x84, 0x69, 0xba, 0x6f, 0x28, 0x46, 0x40, 0x49, 0xd1, 0x49, 0x61, 0xdc,
            0xf3, 0x00
        ]
    );
}

#[test]
fn native_availability_commitment_binds_every_manifest_field_to_the_vote() {
    let (frozen, decision) = decision();
    let manifest = decision.manifest;
    let mut exact = b"iroha:lane-reducer:availability:v1\0".to_vec();
    exact.extend(
        norito::encode_canonical(&(
            manifest.layout,
            manifest.chunk_root,
            manifest.byte_len,
            manifest.chunk_count,
        ))
        .unwrap(),
    );
    assert_eq!(manifest.value.availability_hash, Hash::new(exact));
    for field in 0..4 {
        let mut changed = manifest;
        match field {
            0 => changed.chunk_root = Hash::new(b"substituted nonzero RS16 root"),
            1 => changed.byte_len += 1, // Still the same valid stripe/count.
            2 => changed.layout.chunk_size_bytes /= 2,
            3 => {
                changed.byte_len = u64::from(changed.layout.chunk_size_bytes)
                    * u64::from(changed.layout.data_shards)
                    + 1
            }
            _ => unreachable!(),
        }
        changed.chunk_count =
            wire::expected_encoded_chunk_count(changed.byte_len, changed.layout).unwrap();
        assert!(
            changed.validate_availability().is_err(),
            "unchanged voted digest rejects field {field}"
        );
        changed.value.availability_hash = lane_availability_hash(
            changed.layout,
            changed.chunk_root,
            changed.byte_len,
            changed.chunk_count,
        )
        .unwrap();
        changed.validate_availability().unwrap();
        assert_ne!(
            changed.value.subject_hash().unwrap(),
            manifest.value.subject_hash().unwrap()
        );
        let mut statement = decision.commit_qc.statement;
        statement.value = changed.value;
        let bytes = statement.signature_preimage().unwrap();
        for share in &decision.commit_qc.shares {
            assert!(
                Signature::try_from_bytes(&share.signature)
                    .unwrap()
                    .verify(frozen.committee[share.signer as usize].public_key(), &bytes)
                    .is_err(),
                "old signatures reject recomputed manifest digest"
            );
        }
    }
    assert!(
        lane_availability_hash(
            manifest.layout,
            Hash::prehashed([0; 32]),
            manifest.byte_len,
            manifest.chunk_count
        )
        .is_err()
    );
    assert!(
        lane_availability_hash(
            manifest.layout,
            manifest.chunk_root,
            0,
            manifest.chunk_count
        )
        .is_err()
    );
    assert!(
        lane_availability_hash(
            manifest.layout,
            manifest.chunk_root,
            manifest.byte_len,
            manifest.chunk_count + 1
        )
        .is_err()
    );
}
