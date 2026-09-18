// Source-only batch fixtures deliberately grant no native certificate authority.

fn decision_batch_fixture() -> crate::block::lane_decision_batch::LaneDecisionBatchV1 {
    crate::block::lane_decision_batch::LaneDecisionBatchV1 {
        base_state_height: 20,
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"actual WSV base")),
        groups: vec![decision_group_fixture(payload_fixture())],
    }
}

fn decision_batch_header(
    batch: &crate::block::lane_decision_batch::LaneDecisionBatchV1,
) -> BlockHeader {
    BlockHeader::new(
        (batch.base_state_height + 1).try_into().unwrap(),
        Some(HashOf::from_untyped_unchecked(Hash::new(
            b"actual parent block",
        ))),
        None,
        1234,
        7,
    )
}

fn decision_group_on_distinct_route(route_number: u32, index: u32) -> LaneDecisionGroupV1 {
    let mut payload = payload_fixture();
    let route = RoutingDecision::new(LaneId::new(route_number), DataSpaceId::UNIVERSAL);
    let plan = RoutingPlan::single(route);
    let network = binding_fixture().0;
    let key = KeyPair::from_seed(route_number.to_le_bytes().repeat(8), Algorithm::Ed25519);
    let tx = crate::transaction::TransactionBuilder::new(
        network,
        crate::account::AccountId::new(key.public_key().clone()),
        crate::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_admission_intent(crate::transaction::TransactionAdmissionIntent::QueuePlanSynced)
    .sign(key.private_key());
    payload.input.entrypoint = TransactionEntrypoint::External(tx.clone());
    let binding = &mut payload.input.certificate.binding;
    binding.entrypoint_hash = payload.input.entrypoint.hash();
    binding.signed_transaction_hash = Some(tx.hash());
    binding.request_id = queue_plan_synced_request_id(&network, binding.entrypoint_hash);
    binding.routing_plan_digest = plan.digest();
    binding.admission_context.routing_plan_digest = plan.digest();
    let mut authority = binding.admission_context.route_incarnations[0].clone();
    authority.leg = plan.legs()[0];
    authority.lane_incarnation = Hash::new(route_number.to_be_bytes());
    binding.admission_context.route_incarnations = vec![authority.clone()];
    payload.descriptor.slots = vec![LaneInputRouteSlotV1 {
        route,
        lane_incarnation: authority.lane_incarnation,
        instance_id: Hash::new([route_number.to_be_bytes(), index.to_be_bytes()].concat()),
        lane_height: 1,
    }];
    payload.descriptor.admission_priority =
        QueuePlanAdmissionPriorityV1::new(14, usize::try_from(index).unwrap()).unwrap();
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    decision_group_fixture(payload)
}

fn refresh_group_input(group: &mut LaneDecisionGroupV1) {
    let payload = &mut group.payload;
    let binding = &mut payload.input.certificate.binding;
    binding.entrypoint_hash = payload.input.entrypoint.hash();
    binding.signed_transaction_hash = match &payload.input.entrypoint {
        TransactionEntrypoint::External(tx) => Some(tx.hash()),
        TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction().hash()),
        _ => None,
    };
    binding.request_id =
        queue_plan_synced_request_id(&binding_fixture().0, binding.entrypoint_hash);
    payload.descriptor.admitted_input_hash =
        Hash::new(norito::encode_canonical(&payload.input).unwrap());
    *group = decision_group_fixture(payload.clone());
}

#[test]
fn lane_decision_batch_roundtrips_sources_and_commits_exact_prestate_and_order() {
    use crate::block::lane_decision_batch::LaneDecisionBatchV1;
    let mut batch = decision_batch_fixture();
    batch.groups = vec![
        decision_group_on_distinct_route(10, 0),
        decision_group_on_distinct_route(11, 1),
    ];
    batch.validate_structure().unwrap();
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert_eq!(
        LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len()).unwrap(),
        batch
    );
    let json = norito::json::to_json(&batch).unwrap();
    assert_eq!(
        norito::json::from_str::<LaneDecisionBatchV1>(&json).unwrap(),
        batch
    );
    assert_eq!(
        batch.canonical_hash().unwrap(),
        Hash::new_from_chunks(&[b"iroha:lane-consensus:decision-batch:v1\0", &bytes])
    );
    assert_eq!(
        batch.entrypoint_merkle_root(),
        batch
            .groups
            .iter()
            .map(|group| group.payload.input.entrypoint.hash())
            .collect::<iroha_crypto::MerkleTree<_>>()
            .root()
    );
    let mut changed = batch.clone();
    changed.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"other pre-State"));
    assert_ne!(
        changed.canonical_hash().unwrap(),
        batch.canonical_hash().unwrap()
    );
    changed = batch.clone();
    changed.base_state_height += 1;
    assert_ne!(
        changed.canonical_hash().unwrap(),
        batch.canonical_hash().unwrap()
    );
    changed = batch.clone();
    changed.groups.reverse();
    assert!(changed.validate_structure().is_err());
    for field in ["base_state_height", "base_state_hash", "groups"] {
        let mut omitted = norito::json::to_value(&batch).unwrap();
        omitted.as_object_mut().unwrap().remove(field).unwrap();
        assert!(norito::json::from_value::<LaneDecisionBatchV1>(omitted).is_err());
    }
}

#[test]
fn lane_decision_batch_rejects_duplicate_source_signed_and_sealed_owners() {
    let mut batch = decision_batch_fixture();
    batch.groups = vec![
        decision_group_on_distinct_route(10, 0),
        decision_group_on_distinct_route(11, 1),
    ];
    batch.validate_structure().unwrap();
    for mutation in 0..9 {
        let mut changed = batch.clone();
        match mutation {
            0 => changed.base_state_height = 0,
            1 => changed.groups.clear(),
            2 => changed.groups.push(changed.groups[0].clone()),
            3 => changed.base_state_height = 13, // Every source was first admitted at H14.
            4 => {
                changed.groups[1].payload.descriptor.admission_priority =
                    changed.groups[0].payload.descriptor.admission_priority;
                let p = changed.groups[1].payload.clone();
                changed.groups[1] = decision_group_fixture(p);
            }
            5 => {
                changed.groups[1].payload.input.entrypoint =
                    changed.groups[0].payload.input.entrypoint.clone();
                refresh_group_input(&mut changed.groups[1]);
            }
            6 => {
                let TransactionEntrypoint::External(signed) =
                    changed.groups[0].payload.input.entrypoint.clone()
                else {
                    unreachable!()
                };
                changed.groups[1].payload.input.entrypoint = TransactionEntrypoint::SealedReveal(
                    crate::transaction::signed::SealedTransactionReveal::new(
                        Hash::new(b"one commitment"),
                        signed,
                        [0; 32],
                    ),
                );
                refresh_group_input(&mut changed.groups[1]);
            }
            7 => {
                for (index, group) in changed.groups.iter_mut().enumerate() {
                    let TransactionEntrypoint::External(signed) =
                        group.payload.input.entrypoint.clone()
                    else {
                        unreachable!()
                    };
                    group.payload.input.entrypoint = TransactionEntrypoint::SealedReveal(
                        crate::transaction::signed::SealedTransactionReveal::new(
                            Hash::new(b"repeated sealed identity"),
                            signed,
                            [index as u8; 32],
                        ),
                    );
                    refresh_group_input(group);
                }
            }
            8 => {
                changed.groups[1] = decision_group_on_distinct_route(10, 1);
            }
            _ => unreachable!(),
        }
        assert!(
            changed.validate_structure().is_err(),
            "source mutation {mutation}"
        );
    }
}

#[test]
fn lane_decision_batch_canonical_decode_obeys_frame_and_outer_limits() {
    use crate::block::lane_decision_batch::LaneDecisionBatchV1;
    let batch = decision_batch_fixture();
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert!(LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len() - 1).is_err());
    assert!(LaneDecisionBatchV1::decode_canonical(&[], bytes.len()).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(LaneDecisionBatchV1::decode_canonical(&trailing, trailing.len()).is_err());
    let mut malformed = bytes.clone();
    malformed[0] ^= 1;
    assert!(LaneDecisionBatchV1::decode_canonical(&malformed, malformed.len()).is_err());
    let limited = norito::with_decode_limits(
        norito::DecodeLimits::new(bytes.len(), bytes.len(), bytes.len(), 1, 64),
        || {
            LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len())
                .map_err(norito::Error::Message)
        },
    );
    assert!(
        limited.is_err(),
        "nested decoder cannot widen caller allocation budget"
    );
}

#[test]
fn lane_decision_batch_large_body_is_encoded_once_and_decodes_at_carrier_cap() {
    use crate::block::lane_decision_batch::LaneDecisionBatchV1;
    let mut batch = decision_batch_fixture();
    let payload = &mut batch.groups[0].payload;
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
    batch.groups[0] = decision_group_fixture(payload.clone());
    let payload_bytes = norito::encode_canonical(&batch.groups[0].payload).unwrap();
    let bytes = norito::encode_canonical(&batch).unwrap();
    assert!(
        bytes.len() < payload_bytes.len() + 20 * 1024,
        "the complete input occurs once even with three route Decisions"
    );
    assert_eq!(
        LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len()).unwrap(),
        batch
    );
    assert!(LaneDecisionBatchV1::decode_canonical(&bytes, bytes.len() - 1).is_err());
    // Exercise the enclosing canonical carrier, including the owned native slot.
    // A large single input must remain decodable and appear only once on wire.
    let header = decision_batch_header(&batch);
    let bundle =
        crate::block::BlockExecutionContextBundle::default().with_native_lane_decisions(batch);
    let mut builder = crate::block::builder::BlockBuilder::new(header);
    builder.set_execution_context(Some(bundle));
    let carrier = builder.build(Default::default());
    let wire = carrier.encode_wire().unwrap();
    assert!(wire.len() < bytes.len() + 4096);
    assert_eq!(
        crate::block::decode_framed_signed_block(&wire).unwrap(),
        carrier
    );
    let limited = norito::with_decode_limits(
        norito::DecodeLimits::new(wire.len(), wire.len(), wire.len(), 1, 64),
        || {
            crate::block::decode_framed_signed_block(&wire)
                .map_err(|error| norito::Error::Message(error.to_string()))
        },
    );
    assert!(
        limited.is_err(),
        "the enclosing carrier cannot widen its caller's allocation limit"
    );
}

#[test]
fn native_decision_carrier_field_is_required_and_hash_bound() {
    use crate::block::BlockExecutionContextBundle;
    let batch = decision_batch_fixture();
    let bundle = BlockExecutionContextBundle::default().with_native_lane_decisions(batch.clone());
    assert!(!bundle.is_empty());
    bundle.validate_native_lane_decisions_shape().unwrap();
    let bytes = norito::encode_canonical(&bundle).unwrap();
    assert_eq!(
        norito::decode_canonical::<BlockExecutionContextBundle>(&bytes).unwrap(),
        bundle
    );
    let json = norito::json::to_value(&bundle).unwrap();
    assert_eq!(
        norito::json::from_value::<BlockExecutionContextBundle>(json.clone()).unwrap(),
        bundle
    );
    let mut omitted = json;
    omitted
        .as_object_mut()
        .unwrap()
        .remove("native_lane_decisions")
        .unwrap();
    assert!(norito::json::from_value::<BlockExecutionContextBundle>(omitted).is_err());
    let empty = BlockExecutionContextBundle::default();
    let mut omitted_null = norito::json::to_value(&empty).unwrap();
    omitted_null
        .as_object_mut()
        .unwrap()
        .remove("native_lane_decisions")
        .unwrap();
    assert!(norito::json::from_value::<BlockExecutionContextBundle>(omitted_null).is_err());
    let mut changed = bundle.clone();
    changed
        .native_lane_decisions
        .as_mut()
        .unwrap()
        .base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"substituted pre-State"));
    assert_ne!(HashOf::new(&changed), HashOf::new(&bundle));
    assert_ne!(HashOf::new(&empty), HashOf::new(&bundle));
}

#[test]
fn native_decision_carrier_rejects_parallel_external_authority() {
    use crate::block::{BlockExecutionContextBundle, ExternalExecutionContext};
    let batch = decision_batch_fixture();
    let source = &batch.groups[0].payload;
    let slot = &source.descriptor.slots[0];
    let external = ExternalExecutionContext::new(
        source.input.entrypoint.hash(),
        slot.route.lane_id,
        slot.route.dataspace_id,
    );
    let mixed =
        BlockExecutionContextBundle::new(vec![external]).with_native_lane_decisions(batch.clone());
    assert!(mixed.validate_native_lane_decisions_shape().is_err());
    let mut malformed = BlockExecutionContextBundle::default().with_native_lane_decisions(batch);
    malformed
        .native_lane_decisions
        .as_mut()
        .unwrap()
        .groups
        .clear();
    assert!(malformed.validate_native_lane_decisions_shape().is_err());
}

// Test-only exact obsolete DTO shape. No decoder or production alias survives.
#[derive(norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::lane_execution::LaneDecisionExecutionV1")]
struct ObsoleteOutputBearingExecution {
    source: LaneDecisionGroupV1,
    result_hash: HashOf<crate::transaction::signed::TransactionResult>,
    authenticated_signed_replay_alias: Option<Hash>,
    settlement: crate::block::consensus::LaneBlockCommitment,
    fastpq_transcripts_hash: Hash,
}
#[derive(norito::Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::lane_execution::LaneDecisionExecutionBatchV1")]
struct ObsoleteOutputBearingBatch {
    base_state_height: u64,
    base_state_hash: HashOf<BlockHeader>,
    application_block_header: BlockHeader,
    executions: Vec<ObsoleteOutputBearingExecution>,
    application_write_set_root: Hash,
    write_set_root: Hash,
}

#[test]
fn lane_decision_batch_rejects_obsolete_output_layout_and_unknown_claim_fields() {
    use crate::block::lane_decision_batch::LaneDecisionBatchV1;
    let batch = decision_batch_fixture();
    let source = batch.groups[0].clone();
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
    let settlement = crate::block::consensus::LaneBlockCommitment {
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
    let obsolete = ObsoleteOutputBearingBatch {
        base_state_height: batch.base_state_height,
        base_state_hash: batch.base_state_hash,
        application_block_header: decision_batch_header(&batch),
        executions: vec![ObsoleteOutputBearingExecution {
            source,
            result_hash: crate::transaction::signed::TransactionResult::new(Ok(Default::default()))
                .hash(),
            authenticated_signed_replay_alias: None,
            settlement,
            fastpq_transcripts_hash: Hash::new(b"obsolete output claim"),
        }],
        application_write_set_root: Hash::new(b"obsolete economic prefix"),
        write_set_root: Hash::new(b"obsolete marker-inclusive prefix"),
    };
    let old = norito::encode_canonical(&obsolete).unwrap();
    assert!(LaneDecisionBatchV1::decode_canonical(&old, old.len()).is_err());
    for field in [
        "executions",
        "application_block_header",
        "application_write_set_root",
        "write_set_root",
        "result_hash",
        "fastpq_transcripts_hash",
        "authenticated_signed_replay_alias",
        "settlement",
    ] {
        let mut value = norito::json::to_value(&batch).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert(field.into(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<LaneDecisionBatchV1>(value).is_err(),
            "obsolete {field}"
        );
    }
    let bundle =
        crate::block::BlockExecutionContextBundle::default().with_native_lane_decisions(batch);
    let mut old_name = norito::json::to_value(&bundle).unwrap();
    let value = old_name
        .as_object_mut()
        .unwrap()
        .remove("native_lane_decisions")
        .unwrap();
    old_name
        .as_object_mut()
        .unwrap()
        .insert("native_lane_execution".into(), value);
    assert!(
        norito::json::from_value::<crate::block::BlockExecutionContextBundle>(old_name).is_err()
    );
}

#[test]
fn lane_decision_batch_group_and_height_bounds_fail_before_source_expansion() {
    let mut batch = decision_batch_fixture();
    batch.groups = vec![batch.groups[0].clone(); crate::nexus::MAX_ACTIVE_EXECUTION_LANES + 1];
    assert!(
        batch
            .validate_structure()
            .unwrap_err()
            .contains("group count")
    );
    batch = decision_batch_fixture();
    batch.base_state_height = u64::MAX;
    assert!(batch.validate_structure().is_err());
}
