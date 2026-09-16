//! Native immutable input/role structure; no authority is fabricated by these wire fixtures.

use super::*;
use crate::{NetworkId, block::lane_admission::*, transaction::TransactionEntrypoint};
use iroha_crypto::{Algorithm, KeyPair, Signature};
use iroha_model_base::peer::PeerId;

fn binding_fixture() -> (NetworkId, RoutingPlan, QueuePlanAdmissionBindingV1) {
    let network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"lane admission model network",
    )));
    let plan = RoutingPlan::native_amx(
        RoutingDecision::new(LaneId::new(2), DataSpaceId::new(5)),
        vec![
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(7), DataSpaceId::new(9)),
                RouteLegRole::Participant,
            ),
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(3), DataSpaceId::new(6)),
                RouteLegRole::Participant,
            ),
        ],
    );
    let validators: Vec<_> = (1..=4)
        .map(|seed| {
            let key = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
            PeerId::new(key.public_key().clone())
        })
        .collect();
    let context = QueuePlanAdmissionContextV1 {
        version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
        authority_height: 12,
        proposal_height: 13,
        predecessor_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
            b"actual predecessor",
        ))),
        routing_plan_digest: plan.digest(),
        route_incarnations: plan
            .legs()
            .into_iter()
            .map(|leg| QueuePlanRouteIncarnationV1 {
                leg,
                lane_incarnation: Hash::new(leg.route.lane_id.as_u32().to_le_bytes()),
                validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_set: validators.clone(),
                validator_count: 4,
                durability_threshold: 2,
            })
            .collect(),
    };
    let entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"untrusted input reference"));
    let binding = QueuePlanAdmissionBindingV1 {
        version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        network_id_digest: queue_plan_admission_network_id_digest(&network),
        request_id: queue_plan_synced_request_id(&network, entrypoint_hash),
        entrypoint_hash,
        signed_transaction_hash: None,
        routing_plan_digest: plan.digest(),
        admission_context: context,
        enqueue_timestamp_ms: 73,
        queue_plan_journal_version: QUEUE_PLAN_JOURNAL_CLAIM_VERSION_V1,
        durable_admission_version: QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V1,
        // Deliberately not a physical journal claim. Core must compare exact transaction bytes.
        journal_record_digest: Hash::new(b"shape-only journal digest"),
    };
    (network, plan, binding)
}

fn complete_input_model_fixture() -> LaneAdmittedInputV1 {
    let (network, _, mut binding) = binding_fixture();
    let key = KeyPair::from_seed(vec![0x37; 32], Algorithm::Ed25519);
    let signed = crate::transaction::TransactionBuilder::new(
        network,
        crate::account::AccountId::new(key.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(key.private_key());
    binding.signed_transaction_hash = Some(signed.hash());
    let entrypoint = TransactionEntrypoint::External(signed);
    binding.entrypoint_hash = entrypoint.hash();
    binding.request_id = queue_plan_synced_request_id(&network, binding.entrypoint_hash);
    LaneAdmittedInputV1 {
        entrypoint,
        certificate: QueuePlanAdmissionCertificateV1 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
            binding,
            // Pure wire fixture: this signature is deliberately not an admission attestation.
            attestations: vec![QueuePlanAdmissionAttestationV1 {
                version: QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V1,
                validator_index: 0,
                signature: Signature::new(key.private_key(), b"untrusted model fixture"),
            }],
        },
    }
}

fn payload_fixture() -> LaneInputPayloadV1 {
    let input = complete_input_model_fixture();
    let mut slots = input
        .certificate
        .binding
        .admission_context
        .route_incarnations
        .iter()
        .map(|bound| LaneInputRouteSlotV1 {
            route: bound.leg.route,
            lane_incarnation: bound.lane_incarnation,
            instance_id: Hash::new(bound.leg.route.lane_id.as_u32().to_be_bytes()),
            lane_height: 1,
        })
        .collect::<Vec<_>>();
    slots.sort_by_key(LaneInputRouteSlotV1::route_key);
    LaneInputPayloadV1 {
        descriptor: LaneInputDescriptorV1 {
            version: LANE_INPUT_VERSION_V1,
            admission_priority: QueuePlanAdmissionPriorityV1::new(14, 2).unwrap(),
            admission_carrier_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"actual first carrier",
            )),
            admitted_input_hash: Hash::new(norito::encode_canonical(&input).unwrap()),
            slots,
        },
        input,
    }
}

fn decision_group_fixture(payload: LaneInputPayloadV1) -> LaneDecisionGroupV1 {
    use crate::block::{consensus_v2 as wire, lane_consensus::*};
    let bytes = norito::encode_canonical(&payload).unwrap();
    let layout = wire::recommended_data_availability_layout();
    let chunks = wire::encode_payload_chunks(layout, &bytes).unwrap();
    let chunk_root =
        wire::payload_chunk_root(&chunks.iter().map(Hash::new).collect::<Vec<_>>()).unwrap();
    let decisions = payload
        .descriptor
        .slots
        .iter()
        .enumerate()
        .map(|(index, slot)| {
            let value = LaneValueRefV1 {
                instance_id: slot.instance_id,
                admitted_binding_hash: payload.input.certificate.binding.canonical_hash(),
                kind: payload.validate_structure().unwrap(),
                origin_view: index as u64,
                origin_producer: 0,
                descriptor_hash: payload.descriptor.canonical_hash().unwrap(),
                payload_hash: Hash::new(&bytes),
                availability_hash: lane_availability_hash(
                    layout,
                    chunk_root,
                    bytes.len() as u64,
                    chunks.len() as u32,
                )
                .unwrap(),
            };
            LaneDecisionV1 {
                manifest: LaneManifestV1 {
                    value,
                    layout,
                    chunk_root,
                    byte_len: bytes.len() as u64,
                    chunk_count: chunks.len() as u32,
                },
                commit_qc: LaneQcV1 {
                    statement: LaneVoteStatementV1 {
                        round: LaneRoundV1 {
                            instance_id: slot.instance_id,
                            lane_height: slot.lane_height,
                            voting_view: index as u64 + 3,
                        },
                        phase: LanePhaseV1::Commit,
                        value,
                    },
                    // Structural model data deliberately grants no native quorum authority.
                    shares: Vec::new(),
                },
            }
        })
        .collect();
    LaneDecisionGroupV1 { payload, decisions }
}

#[test]
fn lane_decision_group_roundtrips_one_input_with_independent_route_views() {
    let group = decision_group_fixture(payload_fixture());
    group.validate_structure().unwrap();
    let bytes = norito::encode_canonical(&group).unwrap();
    let decoded = LaneDecisionGroupV1::decode_canonical(&bytes, bytes.len()).unwrap();
    assert_eq!(decoded, group);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
    let json = norito::json::to_json(&group).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneDecisionGroupV1>(&json).unwrap(),
        group
    );
    assert!(LaneDecisionGroupV1::decode_canonical(&bytes, bytes.len() - 1).is_err());
    assert!(LaneDecisionGroupV1::decode_canonical(&[], bytes.len()).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(LaneDecisionGroupV1::decode_canonical(&trailing, trailing.len()).is_err());
    let mut malformed = bytes;
    malformed[0] ^= 1;
    assert!(LaneDecisionGroupV1::decode_canonical(&malformed, malformed.len()).is_err());
    for mutation in 0..9 {
        let mut changed = group.clone();
        match mutation {
            0 => {
                changed.decisions.pop();
            }
            1 => changed.decisions.swap(0, 1),
            2 => changed.decisions[1] = changed.decisions[0].clone(),
            3 => changed.decisions[0].commit_qc.statement.phase = LanePhaseV1::Prepare,
            4 => changed.decisions[0].manifest.value.payload_hash = Hash::new(b"another body"),
            5 => changed.decisions[0].commit_qc.statement.round.lane_height += 1,
            6 => changed.decisions[0].manifest.byte_len += 1,
            7 => {
                changed
                    .payload
                    .descriptor
                    .admission_priority
                    .admission_index += 1
            }
            8 => changed.decisions[1].commit_qc.statement.round.voting_view = 0,
            _ => unreachable!(),
        }
        assert!(changed.validate_structure().is_err(), "mutation {mutation}");
    }
}

#[test]
fn lane_decision_group_decodes_large_input_once_under_the_actual_frame_budget() {
    let mut payload = payload_fixture();
    let (network, _, _) = binding_fixture();
    let key = KeyPair::from_seed(vec![0x37; 32], Algorithm::Ed25519);
    let transaction = crate::transaction::TransactionBuilder::new(
        network,
        crate::account::AccountId::new(key.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([crate::isi::Log::new(
        crate::Level::INFO,
        "x".repeat(800 * 1024),
    )])
    .with_admission_intent(crate::transaction::TransactionAdmissionIntent::QueuePlanSynced)
    .sign(key.private_key());
    payload.input.certificate.binding.signed_transaction_hash = Some(transaction.hash());
    payload.input.entrypoint = TransactionEntrypoint::External(transaction);
    let binding = &mut payload.input.certificate.binding;
    binding.entrypoint_hash = payload.input.entrypoint.hash();
    binding.request_id = queue_plan_synced_request_id(&network, binding.entrypoint_hash);
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    let group = decision_group_fixture(payload);
    let input_bytes = norito::encode_canonical(&group.payload).unwrap();
    let bytes = norito::encode_canonical(&group).unwrap();
    assert!(
        bytes.len() < input_bytes.len() + 16 * 1024,
        "route decisions do not duplicate the large input"
    );
    assert_eq!(
        LaneDecisionGroupV1::decode_canonical(&bytes, bytes.len()).unwrap(),
        group
    );
    let strict = norito::DecodeLimits::new(bytes.len(), bytes.len(), bytes.len(), 64, 64);
    assert!(
        norito::with_decode_limits(strict, || LaneDecisionGroupV1::decode_canonical(
            &bytes,
            bytes.len()
        )
        .map_err(norito::Error::Message))
        .is_err()
    );
}

#[test]
fn lane_input_roundtrips_complete_body_and_all_three_dtos() {
    let payload = payload_fixture();
    assert_eq!(
        payload.validate_structure().unwrap(),
        LaneValueKindV1::AtomicGroup
    );
    macro_rules! roundtrip {
        ($ty:ty, $value:expr) => {{
            let value: $ty = $value;
            let bytes = norito::encode_canonical(&value).unwrap();
            let decoded: $ty = norito::decode_canonical(&bytes).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
            let json = norito::json::to_json(&value).unwrap();
            assert_eq!(norito::json::from_str::<$ty>(&json).unwrap(), value);
        }};
    }
    roundtrip!(LaneInputRouteSlotV1, payload.descriptor.slots[0]);
    roundtrip!(LaneInputDescriptorV1, payload.descriptor.clone());
    roundtrip!(LaneInputPayloadV1, payload);
}

#[test]
fn lane_input_single_route_and_shared_coordinator_participant_keep_one_slot() {
    let mut payload = payload_fixture();
    let binding = &mut payload.input.certificate.binding;
    binding.admission_context.route_incarnations.truncate(1);
    let plan = binding.admission_context.routing_plan().unwrap();
    binding.routing_plan_digest = plan.digest();
    binding.admission_context.routing_plan_digest = plan.digest();
    payload
        .descriptor
        .slots
        .retain(|slot| slot.route == plan.coordinator_route());
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    assert_eq!(
        payload.validate_structure().unwrap(),
        LaneValueKindV1::Execution
    );
    let binding = &mut payload.input.certificate.binding;
    let mut participant = binding.admission_context.route_incarnations[0].clone();
    participant.leg.role = RouteLegRole::Participant;
    binding
        .admission_context
        .route_incarnations
        .push(participant);
    let plan = binding.admission_context.routing_plan().unwrap();
    binding.routing_plan_digest = plan.digest();
    binding.admission_context.routing_plan_digest = plan.digest();
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    assert_eq!(
        payload.validate_structure().unwrap(),
        LaneValueKindV1::AtomicGroup
    );
    assert_eq!(payload.descriptor.slots.len(), 1);
    assert_eq!(
        plan.legs().len(),
        2,
        "both signed roles survive without a second vote slot"
    );
    let mut duplicate = payload.clone();
    duplicate
        .descriptor
        .slots
        .push(duplicate.descriptor.slots[0]);
    assert!(duplicate.validate_structure().is_err());
    payload
        .input
        .certificate
        .binding
        .admission_context
        .route_incarnations[1]
        .lane_incarnation = Hash::new(b"foreign incarnation");
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    assert!(
        payload
            .validate_structure()
            .unwrap_err()
            .contains("different incarnations")
    );
}

#[test]
fn lane_input_rejects_substitution_and_partial_or_extra_route_sets() {
    let payload = payload_fixture();
    let mut changed = payload.clone();
    changed.input.certificate.attestations[0].signature = Signature::from_bytes(&[7; 64]);
    assert!(
        changed
            .validate_structure()
            .unwrap_err()
            .contains("source hash")
    );
    let mut missing = payload.clone();
    missing.descriptor.slots.pop();
    assert!(
        missing
            .validate_structure()
            .unwrap_err()
            .contains("complete admitted route set")
    );
    let mut changed_route = payload.clone();
    changed_route.descriptor.slots[0].route.dataspace_id = DataSpaceId::new(4);
    assert!(changed_route.validate_structure().is_err());
    let mut changed_incarnation = payload.clone();
    changed_incarnation.descriptor.slots[0].lane_incarnation = Hash::new(b"other incarnation");
    assert!(changed_incarnation.validate_structure().is_err());
    let mut reordered = payload;
    reordered.descriptor.slots.reverse();
    assert!(
        reordered
            .validate_structure()
            .unwrap_err()
            .contains("unordered")
    );
}

#[test]
fn lane_input_descriptor_enforces_source_slot_and_count_bounds() {
    let descriptor = payload_fixture().descriptor;
    type Change = fn(&mut LaneInputDescriptorV1);
    let changes: &[Change] = &[
        |d| d.version += 1,
        |d| d.admission_priority.carrier_height = 0,
        |d| {
            d.admission_priority.admission_index =
                super::super::MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK as u32
        },
        |d| d.admission_carrier_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0; 32])),
        |d| d.admitted_input_hash = Hash::prehashed([0; 32]),
        |d| d.slots.clear(),
        |d| d.slots.resize(MAX_LANE_INPUT_ROUTE_SLOTS + 1, d.slots[0]),
        |d| d.slots[0].lane_height = 0,
        |d| d.slots[0].lane_incarnation = Hash::prehashed([0; 32]),
        |d| d.slots[0].instance_id = Hash::prehashed([0; 32]),
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut changed = descriptor.clone();
        change(&mut changed);
        assert!(changed.validate_structure().is_err(), "mutation {index}");
        assert!(
            changed.canonical_hash().is_err(),
            "invalid shape cannot claim a canonical identity"
        );
    }
}

#[test]
fn lane_input_descriptor_hash_binds_source_and_every_slot_identity() {
    let descriptor = payload_fixture().descriptor;
    let original = descriptor.canonical_hash().unwrap();
    type Change = fn(&mut LaneInputDescriptorV1);
    let changes: &[Change] = &[
        |d| d.admission_priority.carrier_height += 1,
        |d| d.admission_priority.admission_index += 1,
        |d| d.admission_carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"other carrier")),
        |d| d.admitted_input_hash = Hash::new(b"other source input"),
        |d| d.slots[0].lane_height += 1,
        |d| d.slots[0].lane_incarnation = Hash::new(b"other incarnation"),
        |d| d.slots[0].instance_id = Hash::new(b"same height new instance"),
        |d| d.slots[0].route.dataspace_id = DataSpaceId::new(4),
    ];
    for change in changes {
        let mut changed = descriptor.clone();
        change(&mut changed);
        assert_ne!(changed.canonical_hash().unwrap(), original);
    }
}

#[test]
fn lane_input_wire_requires_exact_source_and_rejects_execution_result_fields() {
    let payload = payload_fixture();
    for key in ["descriptor", "input"] {
        let mut value = norito::json::to_value(&payload).unwrap();
        value.as_object_mut().unwrap().remove(key);
        assert!(norito::json::from_value::<LaneInputPayloadV1>(value).is_err());
    }
    let mut value = norito::json::to_value(&payload).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("execution_result".to_owned(), norito::json::Value::Null);
    assert!(norito::json::from_value::<LaneInputPayloadV1>(value).is_err());
    assert!(
        norito::decode_canonical::<LaneInputPayloadV1>(
            &norito::encode_canonical(&payload.input).unwrap()
        )
        .is_err()
    );
}

// Included in lane_input::tests to reuse its explicitly unauthenticated wire fixtures.

fn economic_execution_fixture() -> crate::block::lane_execution::LaneDecisionExecutionV1 {
    use crate::block::{consensus::LaneBlockCommitment, lane_execution::LaneDecisionExecutionV1};
    let source = decision_group_fixture(payload_fixture());
    let route = source
        .payload
        .input
        .routing_plan()
        .unwrap()
        .coordinator_route();
    let slot = source
        .payload
        .descriptor
        .slots
        .iter()
        .find(|slot| slot.route == route)
        .unwrap();
    let settlement = LaneBlockCommitment {
        block_height: slot.lane_height,
        lane_id: route.lane_id,
        lane_incarnation: slot.lane_incarnation,
        dataspace_id: route.dataspace_id,
        tx_count: 0,
        total_local_amount: "0".parse().unwrap(),
        total_xor_due: "0".parse().unwrap(),
        total_xor_after_haircut: "0".parse().unwrap(),
        total_xor_variance: "0".parse().unwrap(),
        swap_metadata: None,
        receipts: vec![],
        nexus_fee_receipts: vec![],
        native_amx_receipts: vec![],
    };
    LaneDecisionExecutionV1 {
        source,
        result: crate::transaction::signed::TransactionResult::new(Ok(Default::default())),
        authenticated_signed_replay_alias: None,
        settlement,
        fastpq_transcripts: vec![],
    }
}

fn economic_batch_fixture() -> crate::block::lane_execution::LaneDecisionExecutionBatchV1 {
    crate::block::lane_execution::LaneDecisionExecutionBatchV1 {
        base_state_height: 20,
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"actual WSV base")),
        application_block_header: BlockHeader::new(
            21.try_into().unwrap(),
            Some(HashOf::from_untyped_unchecked(Hash::new(b"parent block"))),
            None,
            None,
            1234,
            7,
        ),
        executions: vec![economic_execution_fixture()],
        application_write_set_root: Hash::new(b"economic writes"),
        write_set_root: Hash::new(b"economic writes plus replay markers"),
    }
}

#[test]
fn lane_economic_transcript_roundtrips_one_input_and_derives_exact_result_roots() {
    use crate::block::lane_execution::LaneDecisionExecutionBatchV1;
    use iroha_crypto::MerkleTree;
    let batch = economic_batch_fixture();
    batch.validate_structure().unwrap();
    let wire = norito::encode_canonical(&batch).unwrap();
    assert_eq!(
        LaneDecisionExecutionBatchV1::decode_canonical(&wire, wire.len()).unwrap(),
        batch
    );
    let json = norito::json::to_json(&batch).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneDecisionExecutionBatchV1>(&json).unwrap(),
        batch
    );
    let entry = batch.executions[0].source.payload.input.entrypoint.hash();
    let result = batch.executions[0].result.hash();
    assert_eq!(
        batch.entrypoint_merkle_root(),
        [entry].into_iter().collect::<MerkleTree<_>>().root()
    );
    assert_eq!(
        batch.result_merkle_root(),
        [result].into_iter().collect::<MerkleTree<_>>().root()
    );
    let mut changed = batch.clone();
    changed.executions[0].result = crate::transaction::signed::TransactionResult::new(Err(
        crate::transaction::error::TransactionRejectionReason::Validation(
            crate::ValidationFail::NotPermitted("actual deterministic rejection".into()),
        ),
    ));
    assert_eq!(
        changed.entrypoint_merkle_root(),
        batch.entrypoint_merkle_root()
    );
    assert_ne!(changed.result_merkle_root(), batch.result_merkle_root());
    assert_ne!(
        changed.executions[0].canonical_hash().unwrap(),
        batch.executions[0].canonical_hash().unwrap()
    );
    assert_ne!(
        changed.canonical_hash().unwrap(),
        batch.canonical_hash().unwrap()
    );
}

#[test]
fn lane_economic_transcript_marker_identity_has_no_write_root_cycle() {
    let batch = economic_batch_fixture();
    let original = batch.application_identity().unwrap();
    let mut changed = batch.clone();
    changed.write_set_root = Hash::new(b"different marker inclusive writes");
    assert_eq!(changed.application_identity().unwrap(), original);
    assert_ne!(
        changed.canonical_hash().unwrap(),
        batch.canonical_hash().unwrap()
    );
    changed.application_write_set_root = Hash::new(b"different actual economic writes");
    assert_ne!(changed.application_identity().unwrap(), original);
    for mutation in 0..4 {
        let mut changed = batch.clone();
        match mutation {
            0 => {
                changed.base_state_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"different WSV"))
            }
            1 => {
                changed.application_block_header = BlockHeader::new(
                    21.try_into().unwrap(),
                    Some(HashOf::from_untyped_unchecked(Hash::new(
                        b"different parent",
                    ))),
                    None,
                    None,
                    1234,
                    7,
                )
            }
            2 => {
                changed.application_block_header = BlockHeader::new(
                    21.try_into().unwrap(),
                    batch.application_block_header.prev_block_hash(),
                    None,
                    None,
                    1235,
                    7,
                )
            }
            3 => {
                changed.application_block_header = BlockHeader::new(
                    21.try_into().unwrap(),
                    batch.application_block_header.prev_block_hash(),
                    None,
                    None,
                    1234,
                    8,
                )
            }
            _ => unreachable!(),
        }
        assert_ne!(changed.application_identity().unwrap(), original);
        assert_ne!(
            changed.canonical_hash().unwrap(),
            batch.canonical_hash().unwrap()
        );
    }
}

#[test]
fn lane_economic_transcript_rejects_foreign_output_owners_and_competing_routes() {
    let batch = economic_batch_fixture();
    for mutation in 0..10 {
        let mut changed = batch.clone();
        match mutation {
            0 => changed.executions[0].settlement.lane_id = LaneId::new(999),
            1 => changed.executions[0].settlement.block_height += 1,
            2 => changed.executions[0].settlement.tx_count = 2,
            3 => {
                changed.executions[0].authenticated_signed_replay_alias =
                    Some(Hash::new(b"forged alias"))
            }
            4 => changed.executions[0].fastpq_transcripts.push(
                crate::fastpq::TransferTranscriptBundle {
                    entry_hash: Hash::new(b"foreign entry"),
                    transcripts: vec![],
                },
            ),
            5 => changed.executions.push(changed.executions[0].clone()),
            6 => changed.base_state_height = 0,
            7 => changed.executions.clear(),
            8 => {
                changed
                    .application_block_header
                    .set_execution_context_hash(Some(HashOf::from_untyped_unchecked(Hash::new(
                        b"cyclic payload reference",
                    ))));
            }
            9 => {
                changed.application_block_header = BlockHeader::new(
                    20.try_into().unwrap(),
                    batch.application_block_header.prev_block_hash(),
                    None,
                    None,
                    1234,
                    7,
                )
            }
            _ => unreachable!(),
        }
        assert!(
            changed.validate_structure().is_err(),
            "output mutation {mutation}"
        );
    }
    let stripped =
        crate::block::lane_execution::LaneDecisionExecutionBatchV1::application_header_from_carrier(
            &batch.application_block_header,
        );
    assert_eq!(stripped, batch.application_block_header);
}

#[test]
fn lane_economic_transcript_canonical_decode_obeys_frame_and_outer_limits() {
    use crate::block::lane_execution::LaneDecisionExecutionBatchV1;
    let batch = economic_batch_fixture();
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert!(LaneDecisionExecutionBatchV1::decode_canonical(&bytes, bytes.len() - 1).is_err());
    assert!(LaneDecisionExecutionBatchV1::decode_canonical(&[], bytes.len()).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(LaneDecisionExecutionBatchV1::decode_canonical(&trailing, trailing.len()).is_err());
    let mut malformed = bytes.clone();
    malformed[0] ^= 1;
    assert!(LaneDecisionExecutionBatchV1::decode_canonical(&malformed, malformed.len()).is_err());
    let limited = norito::with_decode_limits(
        norito::DecodeLimits::new(bytes.len(), bytes.len(), bytes.len(), 1, 64),
        || {
            LaneDecisionExecutionBatchV1::decode_canonical(&bytes, bytes.len())
                .map_err(norito::Error::Message)
        },
    );
    assert!(
        limited.is_err(),
        "nested decoder cannot widen caller allocation budget"
    );
}

#[test]
fn lane_economic_transcript_large_body_is_encoded_once_and_decodes_at_carrier_cap() {
    use crate::block::lane_execution::LaneDecisionExecutionBatchV1;
    let mut batch = economic_batch_fixture();
    let payload = &mut batch.executions[0].source.payload;
    let (network, _, _) = binding_fixture();
    let key = KeyPair::from_seed(vec![0x37; 32], Algorithm::Ed25519);
    let transaction = crate::transaction::TransactionBuilder::new(
        network,
        crate::account::AccountId::new(key.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([crate::isi::Log::new(
        crate::Level::INFO,
        "x".repeat(800 * 1024),
    )])
    .with_admission_intent(crate::transaction::TransactionAdmissionIntent::QueuePlanSynced)
    .sign(key.private_key());
    payload.input.certificate.binding.signed_transaction_hash = Some(transaction.hash());
    payload.input.entrypoint = TransactionEntrypoint::External(transaction);
    let binding = &mut payload.input.certificate.binding;
    binding.entrypoint_hash = payload.input.entrypoint.hash();
    binding.request_id = queue_plan_synced_request_id(&network, binding.entrypoint_hash);
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    batch.executions[0].source = decision_group_fixture(payload.clone());
    let payload_bytes = norito::encode_canonical(&batch.executions[0].source.payload).unwrap();
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert!(
        bytes.len() < payload_bytes.len() + 20 * 1024,
        "the complete input occurs once even with three route Decisions and economic results"
    );
    assert_eq!(
        LaneDecisionExecutionBatchV1::decode_canonical(&bytes, bytes.len()).unwrap(),
        batch
    );
    assert!(LaneDecisionExecutionBatchV1::decode_canonical(&bytes, bytes.len() - 1).is_err());
}
