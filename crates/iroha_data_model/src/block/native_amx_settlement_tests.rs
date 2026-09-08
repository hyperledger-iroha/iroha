//! Native AMX settlement, attestation and bounded reconstruction contracts.

use super::*;

fn sample_native_amx_qc(
    phase: NativeAmxPhase,
    source_id: [u8; 32],
    plan_digest: Hash,
    coordinator: (LaneId, DataSpaceId),
    participant: (LaneId, DataSpaceId),
    mut validator_set: Vec<PeerId>,
) -> NativeAmxAttestationQcV2 {
    validator_set.sort();
    validator_set.dedup();
    let (coordinator_lane_id, coordinator_dataspace_id) = coordinator;
    let (participant_lane_id, participant_dataspace_id) = participant;
    let participant_validator_count =
        u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
    let participant_min_quorum = u32::try_from(
        validator_set
            .len()
            .saturating_sub(validator_set.len().saturating_sub(1) / 3)
            .max(1),
    )
    .expect("fixture validator quorum fits u32");
    let validator_set_hash = HashOf::new(&validator_set);
    let validator_set_pops = vec![vec![0x5A; 96]; validator_set.len()];
    let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"native-amx-model-genesis",
    )));
    let coordinator_lane_incarnation = Hash::new(b"native-amx-model-coordinator");
    let participant_lane_incarnation = Hash::new(
        [
            b"native-amx-model-participant:".as_slice(),
            &participant_lane_id.as_u32().to_be_bytes(),
        ]
        .concat(),
    );
    let coordinator_proposal_hash = Hash::new(b"native-amx-model-proposal");
    let participant_previous_block_descriptor_hash = Some(Hash::new(
        [
            b"native-amx-model-participant-parent:".as_slice(),
            &participant_lane_id.as_u32().to_be_bytes(),
        ]
        .concat(),
    ));
    let mut body = NativeAmxAttestationBodyV2 {
        round: crate::block::consensus_v2::ConsensusRound {
            context_id: crate::block::consensus_v2::HeightContextId(
                HashOf::from_untyped_unchecked(Hash::new(b"native-amx-receipt-context")),
            ),
            height: 42,
            view: 3,
        },
        epoch: 7,
        network_id,
        source_id,
        tx_entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed(source_id)),
        plan_digest,
        phase,
        coordinator_lane_id,
        coordinator_dataspace_id,
        coordinator_lane_incarnation,
        participant_lane_id,
        participant_dataspace_id,
        participant_lane_incarnation,
        participant_previous_block_height: 41,
        participant_previous_block_descriptor_hash,
        participant_lane_block_height: 42,
        participant_lane_block_view: 0,
        participant_proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
        participant_validator_set_hash: validator_set_hash,
        participant_validator_count,
        participant_min_quorum,
        authority_context_height: 42,
        planned_coordinator_block_height: 42,
        coordinator_lane_block_view: 3,
        coordinator_proposal_hash,
    };
    body.participant_proposal_hash =
        sample_native_amx_participant_proposal(&body, validator_set.clone()).proposal_hash;
    body.participant_settlement_commitment = body
        .computed_grouped_participant_settlement_commitment(None, &[body.source_id])
        .expect("single-source test fixture settlement is valid");
    NativeAmxAttestationQcV2::try_new(
        body,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash,
        validator_set,
        validator_set_pops,
        vec![0b0000_0111],
        vec![0xA5; 96],
    )
    .expect("fixture validator set and proofs must align")
}
#[test]
fn native_amx_grouped_receipt_structure_matches_rust_owned_fixture() {
    let document = grouped_native_amx_fixture_document();
    let receipt_group = document
        .pointer("/golden/receipt_group")
        .cloned()
        .expect("fixture contains receipt group");
    let commitment: LaneBlockCommitment =
        norito::json::from_value(receipt_group.clone()).expect("fixture receipt group decodes");
    commitment
        .validate_native_amx_receipts()
        .expect("Rust-owned grouped Native AMX fixture is structurally valid");
    validate_grouped_native_amx_application_evidence(&document)
        .expect("Rust-owned Native AMX application evidence is valid");
    for receipt in &commitment.native_amx_receipts {
        for leg in &receipt.legs {
            assert!(
                !leg.requires_mixed_role_anchor_validation(),
                "golden grouped legs contain their exact current entrypoint"
            );
        }
    }
    for (path, label) in [
        ("", "settlement commitment"),
        ("/native_amx_receipts/0", "receipt"),
        ("/native_amx_receipts/0/legs/0", "leg"),
        (
            "/native_amx_receipts/0/legs/0/participant_proposal",
            "participant proposal",
        ),
        (
            "/native_amx_receipts/0/legs/0/participant_proposal/descriptor",
            "participant descriptor",
        ),
        (
            "/native_amx_receipts/0/legs/0/participant_settlement",
            "participant settlement",
        ),
        ("/native_amx_receipts/0/legs/0/prepare_qc", "attestation QC"),
        (
            "/native_amx_receipts/0/legs/0/prepare_qc/body",
            "attestation body",
        ),
        (
            "/native_amx_receipts/0/legs/0/prepare_qc/body/phase",
            "attestation phase",
        ),
    ] {
        let mut mutated = receipt_group.clone();
        let target = if path.is_empty() {
            mutated.as_object_mut()
        } else {
            mutated
                .pointer_mut(path)
                .and_then(norito::json::Value::as_object_mut)
        }
        .unwrap_or_else(|| panic!("fixture contains {label} object"));
        target.insert(
            "retired_native_amx_field".to_owned(),
            norito::json::Value::Null,
        );
        assert!(
            norito::json::from_value::<LaneBlockCommitment>(mutated).is_err(),
            "unknown {label} fields must fail exact Native AMX JSON decoding"
        );
    }
    let mut receipt_shaped_source = receipt_group.clone();
    let source = receipt_shaped_source
        .pointer_mut("/native_amx_receipts/0/legs/0/participant_settlement/source_ids/0")
        .expect("flat participant settlement contains a scalar source identifier");
    *source = norito::json::Value::Object(
        [
            ("source_id".to_owned(), source.clone()),
            ("timestamp_ms".to_owned(), norito::json::Value::from(42_u64)),
        ]
        .into_iter()
        .collect(),
    );
    assert!(
        norito::json::from_value::<LaneBlockCommitment>(receipt_shaped_source).is_err(),
        "flat participant membership rejects a receipt-shaped source object"
    );
    let hint = LaneBlockProposalPayloadHintV1 {
        proposal_height: 42,
        proposal_view: 3,
        proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"native-amx-payload-hint-unknown-field",
        )),
    };
    let mut hint_json = norito::json::to_value(&hint).expect("serialize payload hint");
    hint_json
        .as_object_mut()
        .expect("payload hint is an object")
        .insert(
            "retired_native_amx_field".to_owned(),
            norito::json::Value::Null,
        );
    assert!(
        norito::json::from_value::<LaneBlockProposalPayloadHintV1>(hint_json).is_err(),
        "unknown payload-hint fields must fail exact Native AMX JSON decoding"
    );
}
fn sample_native_amx_invariant_qc() -> NativeAmxAttestationQcV2 {
    sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x81; 32],
        Hash::new(b"native-amx-validator-material-invariant"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
}
fn native_amx_qc_wire(qc: &NativeAmxAttestationQcV2) -> NativeAmxAttestationQcV2Wire {
    NativeAmxAttestationQcV2Wire {
        body: qc.body,
        validator_set_hash_version: qc.validator_set_hash_version,
        validator_set_hash: qc.validator_set_hash,
        validator_set: qc.validator_set().to_vec(),
        validator_set_pops: qc.validator_set_pops().to_vec(),
        signers_bitmap: qc.signers_bitmap.clone(),
        bls_aggregate_signature: qc.bls_aggregate_signature.clone(),
    }
}
#[test]
fn native_amx_qc_constructor_rejects_misaligned_validator_material() {
    let qc = sample_native_amx_invariant_qc();
    let validator_count = qc.validator_set().len();
    let error = NativeAmxAttestationQcV2::try_new(
        qc.body,
        qc.validator_set_hash_version,
        qc.validator_set_hash,
        qc.validator_set().to_vec(),
        Vec::new(),
        qc.signers_bitmap.clone(),
        qc.bls_aggregate_signature.clone(),
    )
    .expect_err("a validator set without one proof per validator must be rejected");
    assert_eq!(error.validator_count(), validator_count);
    assert_eq!(error.proof_count(), 0);
}

#[test]
fn native_amx_attestation_json_requires_predecessor_slot() {
    let body = sample_native_amx_invariant_qc().body;
    let mut missing = norito::json::to_value(&body).expect("serialize Native AMX attestation body");
    missing
        .as_object_mut()
        .expect("Native AMX attestation JSON object")
        .remove("participant_previous_block_descriptor_hash");
    assert!(
        norito::json::from_value::<NativeAmxAttestationBodyV2>(missing).is_err(),
        "the first-release Native AMX body must require its nullable predecessor slot"
    );
}

#[test]
fn native_amx_qc_binary_decode_preserves_layout_and_rejects_misalignment() {
    let qc = sample_native_amx_invariant_qc();
    let wire = native_amx_qc_wire(&qc);
    assert_eq!(
        qc.encode(),
        wire.encode(),
        "checked construction must retain the canonical flat V1 wire layout"
    );
    assert_eq!(
        NativeAmxAttestationQcV2::decode(&mut qc.encode().as_slice()).expect("aligned QC decodes"),
        qc
    );
    let mut malformed_wire = wire;
    malformed_wire
        .validator_set_pops
        .pop()
        .expect("fixture contains validator proofs");
    assert!(
        NativeAmxAttestationQcV2::decode(&mut malformed_wire.encode().as_slice()).is_err(),
        "binary decoding must not construct misaligned validator material"
    );
}
#[test]
fn native_amx_qc_json_decode_rejects_misaligned_validator_material() {
    let qc = sample_native_amx_invariant_qc();
    let mut value = norito::json::to_value(&qc).expect("serialize aligned QC");
    value
        .as_object_mut()
        .and_then(|object| object.get_mut("validator_set_pops"))
        .and_then(norito::json::Value::as_array_mut)
        .and_then(Vec::pop)
        .expect("fixture JSON contains validator proofs");
    assert!(
        norito::json::from_value::<NativeAmxAttestationQcV2>(value).is_err(),
        "JSON decoding must not construct misaligned validator material"
    );
    assert_eq!(
        norito::json::from_value::<NativeAmxAttestationQcV2>(
            norito::json::to_value(&qc).expect("serialize aligned QC")
        )
        .expect("aligned QC JSON decodes"),
        qc
    );
}
fn sample_native_amx_participant_proposal(
    body: &NativeAmxAttestationBodyV2,
    validator_set: Vec<PeerId>,
) -> LaneBlockProposalV1 {
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id: body.participant_lane_id,
        dataspace_id: body.participant_dataspace_id,
        lane_incarnation: body.participant_lane_incarnation,
        proposal_height: body.authority_context_height,
        previous_lane_block_height: body.participant_previous_block_height,
        previous_lane_block_descriptor_hash: body.participant_previous_block_descriptor_hash,
        lane_block_height: body.participant_lane_block_height,
        lane_block_view: body.participant_lane_block_view,
        subject_hash: Hash::new(b"native-amx-model-participant-subject"),
        payload_ownership_hash: Hash::new(b"native-amx-model-participant-ownership"),
        rbc_instance_hash: Hash::new(b"native-amx-model-participant-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: body.participant_validator_set_hash,
        validator_set,
        validator_count: body.participant_validator_count,
        min_quorum: body.participant_min_quorum,
        qc_mode_tag: "permissioned:native-amx-model".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    proposal
}
fn sample_native_amx_leg(
    source_id: [u8; 32],
    plan_digest: Hash,
    coordinator: (LaneId, DataSpaceId),
    participant: (LaneId, DataSpaceId),
    validator_set: &[PeerId],
) -> NativeAmxLegRecordV2 {
    let prepare_qc = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        source_id,
        plan_digest,
        coordinator,
        participant,
        validator_set.to_vec(),
    );
    let commit_qc = sample_native_amx_qc(
        NativeAmxPhase::Commit,
        source_id,
        plan_digest,
        coordinator,
        participant,
        validator_set.to_vec(),
    );
    let participant_proposal = sample_native_amx_participant_proposal(
        &prepare_qc.body,
        prepare_qc.validator_set().to_vec(),
    );
    debug_assert_eq!(
        prepare_qc.body.participant_proposal_hash,
        participant_proposal.proposal_hash
    );
    debug_assert_eq!(
        commit_qc.body.participant_proposal_hash,
        participant_proposal.proposal_hash
    );
    let participant_settlement = prepare_qc
        .body
        .computed_grouped_participant_settlement(None, &[prepare_qc.body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement_hash = participant_settlement
        .computed_hash()
        .expect("fixture participant settlement hashes");
    NativeAmxLegRecordV2 {
        lane_id: participant.0,
        dataspace_id: participant.1,
        participant_proposal,
        participant_settlement,
        participant_settlement_hash,
        prepare_qc,
        commit_qc,
    }
}
fn grouped_native_amx_fixture_document() -> norito::json::Value {
    norito::json::from_str(include_str!(
        "../../../../fixtures/sumeragi_v2/native_amx_v2_grouped.json"
    ))
    .expect("decode Rust-owned grouped Native AMX fixture document")
}
fn grouped_native_amx_commitment_fixture() -> LaneBlockCommitment {
    let commitment = grouped_native_amx_fixture_document()
        .get("golden")
        .and_then(|golden| golden.get("receipt_group"))
        .cloned()
        .expect("grouped Native AMX fixture contains golden receipt group");
    norito::json::from_value(commitment)
        .expect("decode Rust-owned grouped Native AMX lane commitment")
}

fn native_amx_participant_settlement_wire(
    settlement: &NativeAmxParticipantSettlement,
) -> NativeAmxParticipantSettlementWire {
    NativeAmxParticipantSettlementWire {
        lane_id: settlement.lane_id(),
        dataspace_id: settlement.dataspace_id(),
        lane_incarnation: settlement.lane_incarnation(),
        participant_lane_block_height: settlement.participant_lane_block_height(),
        authority_context_height: settlement.authority_context_height(),
        previous_native_settlement_hash: settlement.previous_native_settlement_hash(),
        source_ids: settlement.source_ids().to_vec(),
    }
}
fn flat_native_amx_participant_settlement() -> NativeAmxParticipantSettlement {
    NativeAmxParticipantSettlement::try_new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        Hash::new(b"flat Native participant incarnation"),
        7,
        40,
        None,
        vec![[0xF0; 32], [0x10; 32]],
    )
    .expect("default catalog route and FIFO sources are valid")
}
#[test]
fn native_amx_participant_settlement_has_nonrecursive_schema_and_bounded_default_stack_codec() {
    use norito::core::SerializePayload as _;
    let schema = NativeAmxParticipantSettlement::schema();
    assert!(!schema.contains_key::<LaneBlockCommitment>());
    assert!(!schema.contains_key::<NativeAmxReceipt>());
    assert!(!schema.contains_key::<NativeAmxLegRecordV2>());
    let sources = (1..=NATIVE_AMX_GROUP_SOURCES_MAX)
        .rev()
        .map(|index| {
            let mut source = [0; Hash::LENGTH];
            source[..8].copy_from_slice(&u64::try_from(index).unwrap().to_le_bytes());
            source
        })
        .collect::<Vec<_>>();
    let settlement = NativeAmxParticipantSettlement::try_new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        Hash::new(b"maximum flat participant"),
        1,
        1,
        None,
        sources.clone(),
    )
    .expect("maximum unique source group is valid");
    let exact = settlement
        .encoded_len_exact()
        .expect("flat control has exact encoded length");
    let mut payload = Vec::new();
    settlement
        .serialize(&mut norito::core::Encoder::for_buffer(&mut payload))
        .expect("serialize maximum flat control on the default test stack");
    assert_eq!(exact, payload.len());
    let encoded = norito::encode_canonical(&settlement).expect("encode maximum flat control");
    let decoded = norito::decode_from_bytes::<NativeAmxParticipantSettlement>(&encoded)
        .expect("decode maximum flat control");
    assert_eq!(decoded.source_ids(), sources);
    assert_eq!(decoded, settlement);
    assert_eq!(
        decoded.computed_hash().unwrap(),
        settlement.computed_hash().unwrap()
    );
    drop(decoded);
    drop(settlement);
}
#[test]
fn native_amx_participant_settlement_binary_and_json_decode_enforce_constructor() {
    let settlement = flat_native_amx_participant_settlement();
    let wire = native_amx_participant_settlement_wire(&settlement);
    assert_eq!(settlement.encode(), wire.encode());
    assert_eq!(
        NativeAmxParticipantSettlement::decode(&mut settlement.encode().as_slice()).unwrap(),
        settlement
    );
    let mut invalid = Vec::new();
    let mut empty = wire.clone();
    empty.source_ids.clear();
    invalid.push(empty);
    let mut oversized = wire.clone();
    oversized.source_ids = vec![[1; 32]; NATIVE_AMX_GROUP_SOURCES_MAX + 1];
    invalid.push(oversized);
    let mut duplicate = wire.clone();
    duplicate.source_ids[1] = duplicate.source_ids[0];
    invalid.push(duplicate);
    let mut zero_source = wire.clone();
    zero_source.source_ids[0] = [0; 32];
    invalid.push(zero_source);
    let mut zero_incarnation = wire.clone();
    zero_incarnation.lane_incarnation = Hash::prehashed([0; Hash::LENGTH]);
    invalid.push(zero_incarnation);
    let mut zero_lane_height = wire.clone();
    zero_lane_height.participant_lane_block_height = 0;
    invalid.push(zero_lane_height);
    let mut zero_authority = wire.clone();
    zero_authority.authority_context_height = 0;
    invalid.push(zero_authority);
    let mut zero_previous = wire.clone();
    zero_previous.previous_native_settlement_hash = Some(HashOf::from_untyped_unchecked(
        Hash::prehashed([0; Hash::LENGTH]),
    ));
    invalid.push(zero_previous);
    let mut first_with_previous = wire;
    first_with_previous.participant_lane_block_height = 1;
    first_with_previous.previous_native_settlement_hash = Some(settlement.computed_hash().unwrap());
    invalid.push(first_with_previous);
    for invalid in invalid {
        assert!(NativeAmxParticipantSettlement::decode(&mut invalid.encode().as_slice()).is_err());
        assert!(
            norito::json::from_value::<NativeAmxParticipantSettlement>(
                norito::json::to_value(&invalid).unwrap()
            )
            .is_err()
        );
        assert!(
            NativeAmxParticipantSettlement::try_new(
                invalid.lane_id,
                invalid.dataspace_id,
                invalid.lane_incarnation,
                invalid.participant_lane_block_height,
                invalid.authority_context_height,
                invalid.previous_native_settlement_hash,
                invalid.source_ids,
            )
            .is_err()
        );
    }
}
#[test]
fn native_amx_participant_settlement_rejects_removed_fields_and_missing_authority() {
    let settlement = flat_native_amx_participant_settlement();
    let canonical = norito::json::to_value(&settlement).unwrap();
    for forbidden in [
        "block_height",
        "tx_count",
        "receipts",
        "total_local_amount",
        "total_xor_due",
        "total_xor_after_haircut",
        "total_xor_variance",
        "swap_metadata",
        "nexus_fee_receipts",
        "native_amx_receipts",
    ] {
        let mut value = canonical.clone();
        value
            .as_object_mut()
            .unwrap()
            .insert(forbidden.to_owned(), norito::json!([]));
        assert!(
            norito::json::from_value::<NativeAmxParticipantSettlement>(value).is_err(),
            "removed field {forbidden} cannot reintroduce economic or recursive structure"
        );
    }
    for required in [
        "lane_id",
        "dataspace_id",
        "lane_incarnation",
        "participant_lane_block_height",
        "authority_context_height",
        "previous_native_settlement_hash",
        "source_ids",
    ] {
        let mut value = canonical.clone();
        value.as_object_mut().unwrap().remove(required);
        assert!(norito::json::from_value::<NativeAmxParticipantSettlement>(value).is_err());
    }
    assert_eq!(
        norito::json::from_value::<NativeAmxParticipantSettlement>(canonical).unwrap(),
        settlement
    );
}

#[test]
fn native_amx_participant_settlement_rejects_unlinked_six_field_binary_shape() {
    #[derive(Encode)]
    struct UnlinkedParticipantSettlement {
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        participant_lane_block_height: u64,
        authority_context_height: u64,
        source_ids: Vec<[u8; Hash::LENGTH]>,
    }
    let settlement = flat_native_amx_participant_settlement();
    let unlinked = UnlinkedParticipantSettlement {
        lane_id: settlement.lane_id(),
        dataspace_id: settlement.dataspace_id(),
        lane_incarnation: settlement.lane_incarnation(),
        participant_lane_block_height: settlement.participant_lane_block_height(),
        authority_context_height: settlement.authority_context_height(),
        source_ids: settlement.source_ids().to_vec(),
    };
    assert!(
        NativeAmxParticipantSettlement::decode(&mut unlinked.encode().as_slice()).is_err(),
        "the omitted predecessor field is not a second binary representation of None"
    );
}
#[test]
fn native_amx_participant_settlement_hash_binds_exact_type_domain_and_source_order() {
    let settlement = flat_native_amx_participant_settlement();
    let bytes = norito::encode_canonical(&settlement).unwrap();
    let domain = b"iroha:native-amx:participant-settlement:v1";
    let length = u64::try_from(domain.len()).unwrap().to_le_bytes();
    let expected = Hash::new_from_chunks(&[&length, domain, &bytes]);
    assert_eq!(Hash::from(settlement.computed_hash().unwrap()), expected);
    assert_ne!(
        settlement.computed_hash().unwrap(),
        HashOf::new(&settlement)
    );
    let mut reversed = settlement.source_ids().to_vec();
    reversed.reverse();
    let reversed = NativeAmxParticipantSettlement::try_new(
        settlement.lane_id(),
        settlement.dataspace_id(),
        settlement.lane_incarnation(),
        settlement.participant_lane_block_height(),
        settlement.authority_context_height(),
        settlement.previous_native_settlement_hash(),
        reversed,
    )
    .unwrap();
    assert_ne!(
        settlement.computed_hash().unwrap(),
        reversed.computed_hash().unwrap()
    );
}

#[test]
fn native_amx_participant_settlement_matches_independent_sdk_array_vectors() {
    // These vectors were independently encoded by the SDKs after qualifying
    // fixed-array field framing against the Rust-generated grouped corpus.
    // Exercise both Option shapes so a shared SDK mistake cannot qualify itself.
    let incarnation = Hash::prehashed([1; Hash::LENGTH]);
    for (height, previous, expected) in [
        (
            1,
            None,
            "hash:350CB3C0D8728E39820775AC522B345C84631FA81BA164F72FB70043657012CF#EB51",
        ),
        (
            2,
            None,
            "hash:C3196EEEB6B5795424F82CDCCA551495E9F459E75EE274EE957FEC51EEE69393#EC1E",
        ),
        (
            2,
            Some(HashOf::from_untyped_unchecked(incarnation)),
            "hash:1F71F0A536D50BB281A9C0FC3A9BF7AA5070F8860BF24173671FFB00D500DED5#1CF3",
        ),
    ] {
        let settlement = NativeAmxParticipantSettlement::try_new(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            incarnation,
            height,
            2,
            previous,
            vec![[0xF0; Hash::LENGTH], [0x10; Hash::LENGTH]],
        )
        .expect("bounded FIFO participant settlement");
        assert_eq!(
            Hash::from(settlement.computed_hash().unwrap()),
            norito::json::from_value::<Hash>(norito::json::Value::String(expected.to_owned()))
                .expect("canonical SDK hash literal")
        );
        let bytes = norito::encode_canonical(&settlement).unwrap();
        assert_eq!(bytes.len(), if previous.is_some() { 282 } else { 249 });
        assert_eq!(
            norito::decode_canonical::<NativeAmxParticipantSettlement>(&bytes).unwrap(),
            settlement
        );
    }
}

#[test]
fn native_amx_participant_settlement_hash_authenticates_sparse_native_history() {
    let first = flat_native_amx_participant_settlement();
    assert_eq!(first.participant_lane_block_height(), 7);
    assert_eq!(
        first.previous_native_settlement_hash(),
        None,
        "the first Native control may follow ordinary lane blocks"
    );
    let previous_hash = first.computed_hash().unwrap();
    let linked = NativeAmxParticipantSettlement::try_new(
        first.lane_id(),
        first.dataspace_id(),
        first.lane_incarnation(),
        9,
        42,
        Some(previous_hash),
        first.source_ids().to_vec(),
    )
    .expect("a prior Native control need not occupy the preceding lane height");
    let unlinked = NativeAmxParticipantSettlement::try_new(
        first.lane_id(),
        first.dataspace_id(),
        first.lane_incarnation(),
        9,
        42,
        None,
        first.source_ids().to_vec(),
    )
    .expect("State must authenticate a claimed first Native control above height one");
    assert_ne!(
        linked.computed_hash().unwrap(),
        unlinked.computed_hash().unwrap()
    );
    let encoded = norito::encode_canonical(&linked).unwrap();
    let decoded = norito::decode_from_bytes::<NativeAmxParticipantSettlement>(&encoded).unwrap();
    assert_eq!(
        decoded.previous_native_settlement_hash(),
        Some(previous_hash)
    );
    assert_eq!(decoded, linked);
    assert_eq!(
        norito::json::from_value::<NativeAmxParticipantSettlement>(
            norito::json::to_value(&linked).unwrap()
        )
        .unwrap(),
        linked
    );
    let absent_json = norito::json::to_value(&unlinked).unwrap();
    assert!(
        absent_json
            .get("previous_native_settlement_hash")
            .unwrap()
            .is_null()
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "this ordered fail-closed fixture validator follows the complete Native AMX evidence pipeline and preserves first-error intent across its canonical anchors"
)]
fn validate_grouped_native_amx_application_evidence(
    document: &norito::json::Value,
) -> Result<(), &'static str> {
    use crate::block::consensus_v2::{ExecutionCommitment, NativeAmxApplicationManifestLeafV1};
    let evidence = document
        .pointer("/golden/application_evidence")
        .ok_or("fixture is missing application evidence")?;
    let execution: ExecutionCommitment = norito::json::from_value(
        evidence
            .get("execution_commitment")
            .cloned()
            .ok_or("fixture is missing execution commitment")?,
    )
    .map_err(|_| "execution commitment is malformed")?;
    execution
        .validate()
        .map_err(|_| "execution commitment is invalid")?;
    let artifacts = evidence
        .get("manifest_artifacts")
        .and_then(norito::json::Value::as_array)
        .ok_or("manifest artifacts are malformed")?;
    if artifacts.len() != 1 || execution.native_amx_application_manifest_count != 1 {
        return Err("fixture must contain one separate-participant manifest");
    }
    let artifact = &artifacts[0];
    if artifact
        .get("version")
        .and_then(norito::json::Value::as_u64)
        != Some(1)
        || artifact
            .get("manifest_leaf_count")
            .and_then(norito::json::Value::as_u64)
            != Some(1)
        || artifact
            .get("leaf_index")
            .and_then(norito::json::Value::as_u64)
            != Some(0)
    {
        return Err("manifest artifact geometry is invalid");
    }
    let leaf: NativeAmxApplicationManifestLeafV1 = norito::json::from_value(
        artifact
            .get("leaf")
            .cloned()
            .ok_or("manifest leaf is missing")?,
    )
    .map_err(|_| "manifest leaf is malformed")?;
    leaf.validate().map_err(|_| "manifest leaf is invalid")?;
    let leaf_hash = HashOf::new(&leaf);
    let advertised_leaf_hash: Hash = norito::json::from_value(
        artifact
            .get("leaf_hash")
            .cloned()
            .ok_or("manifest leaf hash is missing")?,
    )
    .map_err(|_| "manifest leaf hash is malformed")?;
    let manifest_root: Hash = norito::json::from_value(
        artifact
            .get("manifest_root")
            .cloned()
            .ok_or("manifest root is missing")?,
    )
    .map_err(|_| "manifest root is malformed")?;
    let proof: MerkleProof<NativeAmxApplicationManifestLeafV1> = norito::json::from_value(
        artifact
            .get("proof")
            .cloned()
            .ok_or("manifest proof is missing")?,
    )
    .map_err(|_| "manifest proof is malformed")?;
    let typed_root =
        HashOf::<MerkleTree<NativeAmxApplicationManifestLeafV1>>::from_untyped_unchecked(
            manifest_root,
        );
    let manifest_leaf_count =
        NonZeroU64::new(u64::from(execution.native_amx_application_manifest_count))
            .ok_or("manifest commitment leaf count is zero")?;
    let manifest_commitment = MerkleTreeCommitment::new(typed_root, manifest_leaf_count);
    if Hash::from(leaf_hash) != advertised_leaf_hash
        || manifest_root != execution.native_amx_application_manifest_root
        || leaf.executed_block_wire_hash != execution.executed_block_wire_hash
        || !proof.verify(&leaf_hash, &manifest_commitment)
    {
        return Err("manifest proof does not authenticate the leaf");
    }
    let active = evidence
        .get("active_lane_incarnations")
        .and_then(norito::json::Value::as_array)
        .and_then(|rows| rows.first())
        .ok_or("active incarnation is missing")?;
    let active_incarnation: Hash = norito::json::from_value(
        active
            .get("lane_incarnation")
            .cloned()
            .ok_or("active incarnation hash is missing")?,
    )
    .map_err(|_| "active incarnation hash is malformed")?;
    if active.get("lane_id").and_then(norito::json::Value::as_u64)
        != Some(u64::from(leaf.lane_id.as_u32()))
        || active
            .get("dataspace_id")
            .and_then(norito::json::Value::as_u64)
            != Some(leaf.dataspace_id.as_u64())
        || active_incarnation != leaf.lane_incarnation
    {
        return Err("manifest leaf targets a stale incarnation");
    }
    let commitment: LaneBlockCommitment = norito::json::from_value(
        document
            .pointer("/golden/receipt_group")
            .cloned()
            .ok_or("receipt group is missing")?,
    )
    .map_err(|_| "receipt group is malformed")?;
    if leaf.lane_id == commitment.lane_id && leaf.dataspace_id == commitment.dataspace_id {
        return Err("same-route coordinator has separate application evidence");
    }
    let carrier_entrypoints: Vec<Hash> = norito::json::from_value(
        evidence
            .get("carrier_entrypoint_hashes")
            .cloned()
            .ok_or("carrier entrypoints are missing")?,
    )
    .map_err(|_| "carrier entrypoints are malformed")?;
    if leaf.members.len() != commitment.native_amx_receipts.len() {
        return Err("manifest source count differs from receipt group");
    }
    for (member, receipt) in leaf.members.iter().zip(&commitment.native_amx_receipts) {
        if member.source_id != receipt.source_id {
            return Err("manifest source order differs from receipt group");
        }
        let leg = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == leaf.lane_id && leg.dataspace_id == leaf.dataspace_id)
            .ok_or("manifest participant route is missing from receipt")?;
        let descriptor = &leg.participant_proposal.descriptor;
        let participant_height_matches_descriptor =
            descriptor.lane_block_height == leaf.participant_height;
        if descriptor.lane_incarnation != leaf.lane_incarnation
            || !participant_height_matches_descriptor
            || descriptor.lane_block_view != leaf.participant_view
            || descriptor.previous_lane_block_height != leaf.predecessor_height
            || descriptor.previous_lane_block_descriptor_hash != leaf.predecessor_descriptor_hash
            || descriptor.descriptor_hash != leaf.descriptor_hash
            || leg.participant_proposal.proposal_hash != leaf.proposal_hash
            || leg.participant_settlement_hash != leaf.settlement_hash
            || leg.participant_settlement.previous_native_settlement_hash()
                != leaf.previous_native_settlement_hash
            || leg.prepare_qc.body.source_id != member.source_id
            || leg.prepare_qc.body.tx_entrypoint_hash != member.entrypoint_hash
            || !descriptor
                .accepted_transaction_hashes
                .iter()
                .all(|hash| carrier_entrypoints.contains(hash))
        {
            return Err("manifest participant identity or mixed-role anchor differs");
        }
    }
    let diagnostics: SumeragiDiagnosticsStatus = norito::json::from_value(
        document
            .pointer("/golden/expected_diagnostics")
            .cloned()
            .ok_or("diagnostics projection is missing")?,
    )
    .map_err(|_| "diagnostics projection is malformed")?;
    let row = diagnostics
        .native_amx_participant_applications
        .first()
        .ok_or("diagnostics application row is missing")?;
    if row.lane_id != leaf.lane_id
        || row.dataspace_id != leaf.dataspace_id
        || row.lane_incarnation != leaf.lane_incarnation
        || row.participant_height != leaf.participant_height
        || row.participant_view != leaf.participant_view
        || row.predecessor_height != leaf.predecessor_height
        || row.predecessor_descriptor_hash != leaf.predecessor_descriptor_hash
        || row.descriptor_hash != leaf.descriptor_hash
        || row.proposal_hash != leaf.proposal_hash
        || row.settlement_hash != leaf.settlement_hash
        || row.source_count != leaf.members.len() as u64
        || row.application_block_height != Some(leaf.application_block_height)
        || row.application_block_hash != Some(leaf.application_block_hash)
    {
        return Err("diagnostics row differs from application manifest");
    }
    Ok(())
}
fn refresh_native_amx_participant_proposal(leg: &mut NativeAmxLegRecordV2) {
    leg.participant_proposal.descriptor.descriptor_hash = leg
        .participant_proposal
        .descriptor
        .computed_descriptor_hash();
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    for qc in [&mut leg.prepare_qc, &mut leg.commit_qc] {
        qc.body.participant_lane_block_view = leg.participant_proposal.descriptor.lane_block_view;
        qc.body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
    }
}
fn remove_grouped_native_amx_fixture_path(
    document: &mut norito::json::Value,
    path: &str,
    control_id: &str,
) {
    let (parent_path, token) = path
        .rsplit_once('/')
        .unwrap_or_else(|| panic!("control `{control_id}` remove path has a parent"));
    let token = token.replace("~1", "/").replace("~0", "~");
    match document
        .pointer_mut(parent_path)
        .unwrap_or_else(|| panic!("control `{control_id}` remove parent resolves"))
    {
        norito::json::Value::Object(object) => {
            assert!(
                object.remove(&token).is_some(),
                "control `{control_id}` removes an existing field"
            );
        }
        norito::json::Value::Array(array) => {
            let index = token
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("control `{control_id}` remove index is canonical"));
            assert!(
                index < array.len(),
                "control `{control_id}` removes an existing member"
            );
            array.remove(index);
        }
        _ => panic!("control `{control_id}` remove parent is a container"),
    }
}
fn apply_grouped_native_amx_fixture_mutation(
    document: &mut norito::json::Value,
    mutation: &norito::json::Value,
    control_id: &str,
) {
    let operation = mutation
        .get("op")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("control `{control_id}` mutation has an operation"));
    let path = mutation
        .get("path")
        .and_then(norito::json::Value::as_str)
        .unwrap_or_else(|| panic!("control `{control_id}` mutation has a path"));
    match operation {
        "replace" => {
            let replacement = mutation
                .get("value")
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` replace has a value"));
            *document
                .pointer_mut(path)
                .unwrap_or_else(|| panic!("control `{control_id}` replace path resolves")) =
                replacement;
        }
        "remove" => remove_grouped_native_amx_fixture_path(document, path, control_id),
        "swap" => {
            let value = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| panic!("control `{control_id}` swap has geometry"));
            let left = value
                .get("left")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` swap left index is bounded"));
            let right = value
                .get("right")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` swap right index is bounded"));
            let array = document
                .pointer_mut(path)
                .and_then(norito::json::Value::as_array_mut)
                .unwrap_or_else(|| panic!("control `{control_id}` swap path is an array"));
            assert!(
                left < array.len() && right < array.len(),
                "control `{control_id}` swaps existing members"
            );
            array.swap(left, right);
        }
        "copy" => {
            let source_path = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .and_then(|value| value.get("from"))
                .and_then(norito::json::Value::as_str)
                .unwrap_or_else(|| panic!("control `{control_id}` copy has a source path"));
            let replacement = document
                .pointer(source_path)
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` copy source resolves"));
            *document
                .pointer_mut(path)
                .unwrap_or_else(|| panic!("control `{control_id}` copy target resolves")) =
                replacement;
        }
        "repeat" => {
            let value = mutation
                .get("value")
                .and_then(norito::json::Value::as_object)
                .unwrap_or_else(|| panic!("control `{control_id}` repeat has geometry"));
            let source_index = value
                .get("source_index")
                .and_then(norito::json::Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` repeat source index is bounded"));
            let count = value
                .get("count")
                .and_then(norito::json::Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or_else(|| panic!("control `{control_id}` repeat count is bounded"));
            assert!(
                count <= NATIVE_AMX_GROUP_SOURCES_MAX + 1,
                "control `{control_id}` repeat remains bounded"
            );
            let array = document
                .pointer_mut(path)
                .and_then(norito::json::Value::as_array_mut)
                .unwrap_or_else(|| panic!("control `{control_id}` repeat path is an array"));
            let source = array
                .get(source_index)
                .cloned()
                .unwrap_or_else(|| panic!("control `{control_id}` repeat source exists"));
            *array = vec![source; count];
        }
        _ => panic!("control `{control_id}` uses supported mutation `{operation}`"),
    }
}
#[test]
fn native_amx_receipt_negative_corpus_fails_closed() {
    const EXPECTED_RECEIPT_CONTROLS: usize = 47;
    let canonical = grouped_native_amx_fixture_document();
    let controls = canonical
        .get("negative_controls")
        .and_then(norito::json::Value::as_array)
        .expect("fixture contains negative controls");
    let mut evaluated = 0_usize;
    for control in controls {
        if control
            .get("validator")
            .and_then(norito::json::Value::as_str)
            != Some("receipt_group")
        {
            continue;
        }
        evaluated = evaluated.saturating_add(1);
        let id = control
            .get("id")
            .and_then(norito::json::Value::as_str)
            .expect("control has id");
        let mut mutated = canonical.clone();
        for mutation in control
            .get("mutations")
            .and_then(norito::json::Value::as_array)
            .expect("control has mutations")
        {
            apply_grouped_native_amx_fixture_mutation(&mut mutated, mutation, id);
        }
        let receipt_group = mutated
            .pointer("/golden/receipt_group")
            .cloned()
            .unwrap_or_else(|| panic!("control `{id}` retains the receipt group"));
        if matches!(
            id,
            "coherent_duplicate_validator_set" | "coherent_over_quorum_requirement"
        ) {
            assert_eq!(
                mutated.pointer("/golden/expected_diagnostics/lane_settlement_commitments/0",),
                Some(&receipt_group),
                "coherent committee control `{id}` rebuilds the diagnostics projection"
            );
            validate_grouped_native_amx_application_evidence(&mutated).unwrap_or_else(|error| {
                panic!("coherent committee control `{id}` preserves application evidence: {error}")
            });
            let commitment: LaneBlockCommitment = norito::json::from_value(receipt_group.clone())
                .unwrap_or_else(|error| {
                    panic!("coherent committee control `{id}` remains decodable: {error}")
                });
            assert!(
                commitment.validate_native_amx_receipts().is_err(),
                "coherent committee control `{id}` must fail only receipt validation"
            );
            continue;
        }
        let rejected = norito::json::from_value::<LaneBlockCommitment>(receipt_group.clone())
            .map_or(true, |commitment| {
                commitment.validate_native_amx_receipts().is_err()
                    || norito::json::to_value(&commitment)
                        .map_or(true, |canonical| canonical != receipt_group)
            });
        assert!(
            rejected,
            "receipt-group negative control `{id}` must fail closed in Rust"
        );
    }
    assert_eq!(
        evaluated, EXPECTED_RECEIPT_CONTROLS,
        "Rust must execute every declared receipt-group negative control"
    );
}
#[test]
fn native_amx_application_evidence_negative_corpus_fails_closed() {
    const EXPECTED_APPLICATION_EVIDENCE_CONTROLS: usize = 11;
    let canonical = grouped_native_amx_fixture_document();
    validate_grouped_native_amx_application_evidence(&canonical)
        .expect("the canonical application evidence must be valid before mutation");
    let controls = canonical
        .get("negative_controls")
        .and_then(norito::json::Value::as_array)
        .expect("fixture contains negative controls");
    let mut evaluated = 0_usize;
    for control in controls {
        if control
            .get("validator")
            .and_then(norito::json::Value::as_str)
            != Some("application_evidence")
        {
            continue;
        }
        evaluated = evaluated.saturating_add(1);
        let id = control
            .get("id")
            .and_then(norito::json::Value::as_str)
            .expect("control has id");
        let mut mutated = canonical.clone();
        for mutation in control
            .get("mutations")
            .and_then(norito::json::Value::as_array)
            .expect("control has mutations")
        {
            apply_grouped_native_amx_fixture_mutation(&mut mutated, mutation, id);
        }
        assert!(
            validate_grouped_native_amx_application_evidence(&mutated).is_err(),
            "application evidence negative control `{id}` must fail closed"
        );
    }
    assert_eq!(
        evaluated, EXPECTED_APPLICATION_EVIDENCE_CONTROLS,
        "Rust must execute every declared application-evidence negative control"
    );
}
#[test]
fn native_amx_application_evidence_rejects_coherently_wrong_manifest_count() {
    let mut document = grouped_native_amx_fixture_document();
    *document
            .pointer_mut(
                "/golden/application_evidence/execution_commitment/native_amx_application_manifest_count",
            )
            .expect("execution manifest count exists") = norito::json::Value::from(2_u64);
    *document
        .pointer_mut("/golden/application_evidence/manifest_artifacts/0/manifest_leaf_count")
        .expect("artifact manifest count exists") = norito::json::Value::from(2_u64);
    assert_eq!(
        validate_grouped_native_amx_application_evidence(&document),
        Err("fixture must contain one separate-participant manifest"),
        "the same singleton root and proof must not be rebound to a coherent wrong count"
    );
}
#[test]
fn native_amx_grouped_receipts_reject_duplicate_bounds_and_same_route_drift() {
    let mut duplicate = grouped_native_amx_commitment_fixture();
    duplicate.native_amx_receipts[1].source_id = duplicate.native_amx_receipts[0].source_id;
    assert_eq!(
        duplicate.validate_native_amx_receipts(),
        Err("Native AMX receipt sources must be unique")
    );
    let oversized = grouped_native_amx_commitment_fixture();
    let settlement = &oversized.native_amx_receipts[0].legs[0].participant_settlement;
    let mut wire = native_amx_participant_settlement_wire(settlement);
    wire.source_ids = vec![wire.source_ids[0]; NATIVE_AMX_GROUP_SOURCES_MAX + 1];
    assert!(
        NativeAmxParticipantSettlement::decode(&mut wire.encode().as_slice()).is_err(),
        "oversized membership cannot be constructed through binary decoding"
    );
    let mut same_route_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut same_route_drift.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    leg.participant_proposal.descriptor.lane_block_view = leg
        .participant_proposal
        .descriptor
        .lane_block_view
        .saturating_add(1);
    refresh_native_amx_participant_proposal(leg);
    assert_eq!(
        same_route_drift.validate_native_amx_receipts(),
        Err("Native AMX same-route leg differs from the coordinator identity")
    );
}
#[test]
fn native_amx_grouped_receipts_accept_fifo_order_and_route_local_participant_groups() {
    fn receipt(
        source_id: [u8; 32],
        plan_digest: Hash,
        coordinator: (LaneId, DataSpaceId),
        participant: (LaneId, DataSpaceId),
        validators: &[PeerId],
    ) -> NativeAmxReceipt {
        let leg =
            sample_native_amx_leg(source_id, plan_digest, coordinator, participant, validators);
        let body = leg.prepare_qc.body;
        NativeAmxReceipt {
            version: NATIVE_AMX_RECEIPT_VERSION_V2,
            source_id,
            network_id: body.network_id,
            plan_digest,
            lane_id: coordinator.0,
            dataspace_id: coordinator.1,
            lane_incarnation: body.coordinator_lane_incarnation,
            authority_context_height: body.authority_context_height,
            lane_block_height: body.planned_coordinator_block_height,
            lane_block_view: body.coordinator_lane_block_view,
            coordinator_proposal_hash: body.coordinator_proposal_hash,
            legs: vec![leg],
        }
    }
    let validators = sample_roster();
    let coordinator = (LaneId::new(1), DataSpaceId::new(7));
    let first = receipt(
        [0xF0; 32],
        Hash::new(b"FIFO first Native AMX plan"),
        coordinator,
        (LaneId::new(2), DataSpaceId::new(8)),
        &validators,
    );
    let second = receipt(
        [0x10; 32],
        Hash::new(b"FIFO second Native AMX plan"),
        coordinator,
        (LaneId::new(3), DataSpaceId::new(9)),
        &validators,
    );
    assert!(
        first.source_id > second.source_id,
        "fixture is not hash-sorted"
    );
    let commitment = LaneBlockCommitment {
        block_height: 42,
        lane_id: coordinator.0,
        lane_incarnation: first.lane_incarnation,
        dataspace_id: coordinator.1,
        tx_count: 2,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: vec![first, second],
    };
    commitment
        .validate_native_amx_receipts()
        .expect("FIFO coordinator order and route-local participant controls are valid");
}
#[test]
fn native_amx_grouped_receipts_reject_cross_context_height_drift() {
    let mut commitment_height_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut commitment_height_drift.native_amx_receipts[0];
    receipt.lane_block_height = receipt.lane_block_height.saturating_add(1);
    assert_eq!(
        commitment_height_drift.validate_native_amx_receipts(),
        Err("Native AMX receipt coordinator identity is invalid"),
        "a receipt lane height belongs to the containing lane commitment, not an unrelated receipt field"
    );
    let mut proposal_context_drift = grouped_native_amx_commitment_fixture();
    let receipt = &mut proposal_context_drift.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    leg.participant_proposal.descriptor.proposal_height = leg
        .participant_proposal
        .descriptor
        .proposal_height
        .saturating_add(1);
    refresh_native_amx_participant_proposal(leg);
    assert_eq!(
        proposal_context_drift.validate_native_amx_receipts(),
        Err("Native AMX participant leg identity is internally inconsistent"),
        "a participant proposal height is bound to the coordinator authority context"
    );
}
#[test]
fn native_amx_mixed_role_marker_defers_only_separate_participant_anchor() {
    let mut mixed_role = grouped_native_amx_commitment_fixture();
    let receipt = &mut mixed_role.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) != coordinator_route)
        .expect("fixture contains a separate participant leg");
    let current_entrypoint = Hash::from(leg.prepare_qc.body.tx_entrypoint_hash);
    let position = leg
        .participant_proposal
        .descriptor
        .accepted_transaction_hashes
        .iter()
        .position(|hash| *hash == current_entrypoint)
        .expect("fixture participant contains current entrypoint");
    leg.participant_proposal
        .descriptor
        .accepted_transaction_hashes[position] = Hash::new(b"mixed-role executable anchor member");
    refresh_native_amx_participant_proposal(leg);
    assert!(leg.requires_mixed_role_anchor_validation());
    mixed_role
        .validate_native_amx_receipts()
        .expect("separate participant may defer exact block-wide anchor validation");
    let mut same_route = grouped_native_amx_commitment_fixture();
    let receipt = &mut same_route.native_amx_receipts[0];
    let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
    let leg = receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == coordinator_route)
        .expect("fixture contains same-route coordinator leg");
    let current_entrypoint = Hash::from(leg.prepare_qc.body.tx_entrypoint_hash);
    let position = leg
        .participant_proposal
        .descriptor
        .accepted_transaction_hashes
        .iter()
        .position(|hash| *hash == current_entrypoint)
        .expect("fixture coordinator contains current entrypoint");
    leg.participant_proposal
        .descriptor
        .accepted_transaction_hashes[position] = Hash::new(b"invalid same-route mixed-role member");
    refresh_native_amx_participant_proposal(leg);
    assert!(leg.requires_mixed_role_anchor_validation());
    assert_eq!(
        same_route.validate_native_amx_receipts(),
        Err("Native AMX same-route leg differs from the coordinator identity")
    );
}
#[test]
fn native_amx_grouped_receipts_reject_qc_and_group_membership_drift() {
    let mut malformed_bitmap = grouped_native_amx_commitment_fixture();
    malformed_bitmap.native_amx_receipts[0].legs[0]
        .prepare_qc
        .signers_bitmap = vec![0b0000_0011];
    assert_eq!(
        malformed_bitmap.validate_native_amx_receipts(),
        Err("Native AMX participant QC is structurally invalid")
    );
    let mut duplicate_leg = grouped_native_amx_commitment_fixture();
    duplicate_leg.native_amx_receipts[0].legs[1] =
        duplicate_leg.native_amx_receipts[0].legs[0].clone();
    assert_eq!(
        duplicate_leg.validate_native_amx_receipts(),
        Err("Native AMX receipt contains duplicate participant routes")
    );
    let mut group_drift = grouped_native_amx_commitment_fixture();
    group_drift.native_amx_receipts[0].legs[0]
        .participant_settlement
        .source_ids
        .swap(0, 1);
    assert_eq!(
        group_drift.validate_native_amx_receipts(),
        Err("Native AMX participant settlement is structurally invalid")
    );
}
#[test]
fn native_amx_receipts_change_lane_block_commitment_hash_inputs() {
    let plan_digest = Hash::new(b"test-native-amx-plan");
    let source_id = [0xAB; 32];
    let coordinator_lane_id = LaneId::new(0);
    let coordinator_dataspace_id = DataSpaceId::UNIVERSAL;
    let validators = sample_roster();
    let base = LaneBlockCommitment {
        block_height: 42,
        lane_id: coordinator_lane_id,
        lane_incarnation: Hash::new(b"amx-commitment-test-incarnation"),
        dataspace_id: coordinator_dataspace_id,
        tx_count: 1,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: vec![NativeAmxReceipt {
            version: 2,
            source_id,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"native-amx-model-genesis",
            ))),
            plan_digest,
            lane_id: coordinator_lane_id,
            dataspace_id: coordinator_dataspace_id,
            lane_incarnation: Hash::new(b"native-amx-model-coordinator"),
            authority_context_height: 42,
            lane_block_height: 7,
            lane_block_view: 2,
            coordinator_proposal_hash: Hash::new(b"native-amx-model-proposal"),
            legs: vec![
                sample_native_amx_leg(
                    source_id,
                    plan_digest,
                    (coordinator_lane_id, coordinator_dataspace_id),
                    (LaneId::new(7), DataSpaceId::new(7)),
                    &validators,
                ),
                sample_native_amx_leg(
                    source_id,
                    plan_digest,
                    (coordinator_lane_id, coordinator_dataspace_id),
                    (LaneId::new(8), DataSpaceId::new(8)),
                    &validators,
                ),
            ],
        }],
    };
    let mut changed = base.clone();
    changed.native_amx_receipts[0].legs[1].commit_qc.body.phase = NativeAmxPhase::Prepare;
    assert_ne!(Hash::new(base.encode()), Hash::new(changed.encode()));
}
#[test]
fn native_amx_participant_settlement_declares_canonical_schema_identity() {
    // This finite wire type was introduced after the immutable compiler capture.
    let name = "iroha_data_model::block::consensus::NativeAmxParticipantSettlement";
    let hash = norito::core::schema_hash_for_name(name);
    assert_eq!(
        <NativeAmxParticipantSettlement as norito::NoritoSchema>::nominal_name(),
        name
    );
    assert_eq!(
        <NativeAmxParticipantSettlement as norito::NoritoSchema>::frame_name(),
        name
    );
    assert_eq!(
        norito::schema::identity::frame_hash::<NativeAmxParticipantSettlement>(),
        hash
    );
    assert_eq!(
        <NativeAmxParticipantSettlement as norito::NoritoSerialize>::schema_hash(),
        hash
    );
    assert_eq!(
        <NativeAmxParticipantSettlement as norito::NoritoDeserialize>::schema_hash(),
        hash
    );
}
#[test]
fn native_amx_v2_grouped_participant_settlement_is_exact_zero_effect_evidence() {
    assert_eq!(
        <NativeAmxParticipantSettlement as norito::NoritoSchema>::nominal_name(),
        "iroha_data_model::block::consensus::NativeAmxParticipantSettlement"
    );
    let source_id = [0xC7; 32];
    let fifo_sources = [[0xC8; 32], source_id];
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        source_id,
        Hash::new(b"v2-zero-effect-settlement-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;
    let settlement = body
        .computed_grouped_participant_settlement(None, &fifo_sources)
        .expect("FIFO-ordered grouped participant control");
    assert_eq!(
        settlement.participant_lane_block_height(),
        body.participant_lane_block_height
    );
    assert_eq!(settlement.lane_id(), body.participant_lane_id);
    assert_eq!(
        settlement.lane_incarnation(),
        body.participant_lane_incarnation
    );
    assert_eq!(settlement.dataspace_id(), body.participant_dataspace_id);
    assert_eq!(
        settlement.authority_context_height(),
        body.authority_context_height
    );
    assert_eq!(settlement.tx_count(), 2);
    assert_eq!(settlement.source_ids(), fifo_sources);
    assert_eq!(
        Hash::from(settlement.computed_hash().unwrap()),
        body.computed_grouped_participant_settlement_commitment(None, &fifo_sources)
            .unwrap()
    );
    let previous_native_settlement_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
        b"preceding Native settlement before an ordinary block",
    )));
    let linked = body
        .computed_grouped_participant_settlement(previous_native_settlement_hash, &fifo_sources)
        .expect("the grouped constructor retains the explicit Native history link");
    assert_eq!(
        linked.previous_native_settlement_hash(),
        previous_native_settlement_hash
    );
    assert_ne!(
        linked.computed_hash().unwrap(),
        settlement.computed_hash().unwrap()
    );
    assert_eq!(
        Hash::from(linked.computed_hash().unwrap()),
        body.computed_grouped_participant_settlement_commitment(
            previous_native_settlement_hash,
            &fifo_sources
        )
        .unwrap()
    );
    let encoded = norito::encode_canonical(&settlement).unwrap();
    assert_eq!(
        norito::decode_from_bytes::<NativeAmxParticipantSettlement>(&encoded).unwrap(),
        settlement
    );
}

#[test]
fn native_amx_v2_leg_rejects_removed_recursive_settlement_layout() {
    use norito::core::{DecodeFlagsGuard, header_flags};

    // This encode-only fixture supplies the removed field at its original nested boundary.
    // Current QCs are retained so rejection cannot be attributed to stale QC validation.
    #[derive(norito::codec::Encode)]
    struct RemovedSettlementLeg {
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        participant_proposal: LaneBlockProposalV1,
        participant_settlement: LaneBlockCommitment,
        participant_settlement_hash: HashOf<LaneBlockCommitment>,
        prepare_qc: NativeAmxAttestationQcV2,
        commit_qc: NativeAmxAttestationQcV2,
    }

    let leg = sample_native_amx_leg(
        [0xC9; 32],
        Hash::new(b"native-amx-removed-settlement-layout"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        &sample_roster(),
    );
    let settlement = &leg.participant_settlement;
    let removed = RemovedSettlementLeg {
        lane_id: leg.lane_id,
        dataspace_id: leg.dataspace_id,
        participant_proposal: leg.participant_proposal.clone(),
        participant_settlement: LaneBlockCommitment {
            block_height: settlement.participant_lane_block_height(),
            lane_id: settlement.lane_id(),
            lane_incarnation: settlement.lane_incarnation(),
            dataspace_id: settlement.dataspace_id(),
            tx_count: settlement.tx_count(),
            total_local_amount: Quantity::zero(),
            total_xor_due: Quantity::zero(),
            total_xor_after_haircut: Quantity::zero(),
            total_xor_variance: Quantity::zero(),
            swap_metadata: None,
            receipts: settlement
                .source_ids()
                .iter()
                .map(|source_id| LaneSettlementReceipt {
                    source_id: *source_id,
                    local_amount: Quantity::zero(),
                    xor_due: Quantity::zero(),
                    xor_after_haircut: Quantity::zero(),
                    xor_variance: Quantity::zero(),
                    timestamp_ms: settlement.authority_context_height(),
                })
                .collect(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        },
        participant_settlement_hash: HashOf::from_untyped_unchecked(Hash::from(
            leg.participant_settlement_hash,
        )),
        prepare_qc: leg.prepare_qc.clone(),
        commit_qc: leg.commit_qc.clone(),
    };

    for requested in [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT
            | header_flags::PACKED_SEQ
            | header_flags::COMPACT_LEN
            | header_flags::FIELD_BITSET,
    ] {
        let _flags = DecodeFlagsGuard::enter(requested);
        let canonical = norito::to_bytes(&leg).expect("encode the current finite leg");
        assert_eq!(
            norito::decode_from_bytes::<NativeAmxLegRecordV2>(&canonical)
                .expect("current finite leg must decode in the advertised layout"),
            leg
        );
        let (payload, flags) = norito::codec::encode_with_header_flags(&removed);
        // Use the current root identity to exercise the nested layout check itself.
        let framed =
            norito::core::frame_bare_with_header_flags::<NativeAmxLegRecordV2>(&payload, flags)
                .expect("frame the removed leg layout");
        assert!(
            norito::decode_from_bytes::<NativeAmxLegRecordV2>(&framed).is_err(),
            "accepted removed recursive settlement layout {flags:#x}"
        );
    }
}

#[test]
fn native_amx_v2_grouped_participant_settlement_rejects_invalid_source_groups() {
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x31; 32],
        Hash::new(b"v2-invalid-source-group-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;
    assert!(
        body.computed_grouped_participant_settlement(None, &[])
            .is_err()
    );
    assert!(
        body.computed_grouped_participant_settlement(None, &[[0x32; 32]])
            .is_err()
    );
    assert_eq!(
        body.computed_grouped_participant_settlement(None, &[body.source_id, body.source_id]),
        Err("Native AMX participant source group must be unique")
    );
    let reverse_hash_order = [[0x32; 32], body.source_id];
    let reverse_settlement = body
        .computed_grouped_participant_settlement(None, &reverse_hash_order)
        .expect("candidate order is independent of source hash order");
    assert_eq!(reverse_settlement.source_ids(), reverse_hash_order);
    assert!(
        body.computed_grouped_participant_settlement(
            None,
            &vec![body.source_id; NATIVE_AMX_GROUP_SOURCES_MAX + 1]
        )
        .is_err()
    );
}
#[test]
fn native_amx_v2_attestation_preimage_binds_round_and_epoch() {
    let body = sample_native_amx_qc(
        NativeAmxPhase::Prepare,
        [0x31; 32],
        Hash::new(b"v2-context-bound-plan"),
        (LaneId::new(1), DataSpaceId::new(7)),
        (LaneId::new(2), DataSpaceId::new(8)),
        sample_roster(),
    )
    .body;
    let preimage = body.signature_preimage();
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            body.signature_preimage(),
            preimage,
            "Native AMX signature identity must ignore the caller's ambient Norito layout"
        );
    }
    let mut another_view = body;
    another_view.round.view = another_view.round.view.saturating_add(1);
    let mut another_epoch = body;
    another_epoch.epoch = another_epoch.epoch.saturating_add(1);
    assert!(preimage.starts_with(b"iroha:native-amx:v2"));
    assert_ne!(preimage, another_view.signature_preimage());
    assert_ne!(preimage, another_epoch.signature_preimage());
}
