// Canonical Network index fixtures exercise structural membership, never mint execution/finality.
/// Attach structurally checked outputs under a finite, explicit storage-test policy.
pub(crate) fn install_network_index_test_outputs(
    block: &mut SignedBlock,
    outputs: Vec<iroha_data_model::block::execution_output::ExecutionOutputV1>,
) {
    use iroha_data_model::block::output_budget::ExecutionOutputLimits;
    // Explicit finite storage-fixture policy, not a runtime admission default.
    let limits = ExecutionOutputLimits {
        max_outputs: 1024,
        max_output_bytes: 64 * 1024 * 1024,
        max_total_output_bytes: 128 * 1024 * 1024,
        max_executed_wire_bytes: 256 * 1024 * 1024,
    };
    let fragments = u64::try_from(
        outputs
            .iter()
            .filter(|output| output.result().is_ok())
            .count(),
    )
    .expect("bounded storage fixture fragment count");
    let proposal = block.canonical_resultless_proposal();
    block
        .set_execution_outputs(
            outputs,
            fragments,
            Default::default(),
            Vec::new(),
            Default::default(),
            Default::default(),
            Vec::new(),
            &limits,
        )
        .expect("canonical bounded storage-fixture outputs");
    assert_eq!(block.canonical_resultless_proposal(), proposal);
}

fn network_index_block_at(height: u64, inputs: Vec<TransactionEntrypoint>) -> SignedBlock {
    use iroha_data_model::block::builder::BlockBuilder as ModelBlockBuilder;
    let header = BlockHeader::new(
        height.try_into().unwrap(),
        (height > 1).then(|| HashOf::from_untyped_unchecked(Hash::new(b"index structural parent"))),
        None,
        100 + height,
        0,
    );
    let mut builder = ModelBlockBuilder::new(header);
    for input in inputs {
        match input {
            TransactionEntrypoint::External(tx) => builder.push_transaction(tx),
            TransactionEntrypoint::SealedReveal(reveal) => {
                builder.push_sealed_transaction_reveal(reveal)
            }
            TransactionEntrypoint::SealedCommitment(commitment) => {
                builder.push_sealed_transaction_commitment(commitment)
            }
        };
    }
    let mut block = builder.build_with_signature(0, SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    attach_ok_results_to_block(&mut block);
    block
}

fn network_index_signal_input(marker: u64) -> TransactionEntrypoint {
    use iroha_model_base::metadata::Metadata;
    use iroha_primitives::json::Json;
    let mut metadata = Metadata::default();
    metadata.insert("kaigi_signal".parse().unwrap(), Json::new(norito::json!({
        "schema": "iroha-demo-kaigi-chain-signal/v1", "callId": (kaigi_signal_test_call("canonical-inputs").to_string())
    })));
    let mut tx = TransactionBuilder::new(
        test_network_id(b"canonical-index"),
        SAMPLE_GENESIS_ACCOUNT_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    tx.set_creation_time(std::time::Duration::from_millis(marker));
    TransactionEntrypoint::External(
        tx.with_metadata(metadata)
            .with_instructions([Log::new(Level::INFO, marker.to_string())])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key()),
    )
}

fn network_index_internal_output() -> iroha_data_model::block::execution_output::ExecutionOutputV1 {
    use iroha_data_model::{
        block::execution_output::*,
        events::time::{TimeEvent, TimeInterval},
    };
    let trigger_id: iroha_data_model::trigger::TriggerId = "index_timer".parse().unwrap();
    ExecutionOutputV1::Time(TimeExecutionOutputV1 {
        invocation: TimeInvocationV1 {
            schedule_index: 0,
            event: TimeEvent {
                interval: TimeInterval {
                    since_ms: 101,
                    length_ms: 1,
                },
            },
            trigger: TriggerUseV1 {
                trigger_id: trigger_id.clone(),
                registered_at_height: 1,
                action_hash: Hash::new(b"structural action"),
            },
        },
        result: iroha_data_model::transaction::TransactionResult::new(Ok(vec![
            iroha_data_model::trigger::DataTriggerStep {
                id: trigger_id,
                instructions: iroha_data_model::transaction::signed::ExecutionStep(
                    Vec::new().into(),
                ),
            },
        ])),
        failure_root: None,
        completions: Vec::new(),
    })
}

#[test]
fn canonical_network_index_joins_sealed_rejected_and_internal_outputs() {
    use iroha_data_model::{
        block::execution_output::ExecutionOutputV1, transaction::signed::SealedTransactionReveal,
    };
    let first = network_index_signal_input(1);
    let TransactionEntrypoint::External(signed) = network_index_signal_input(2) else {
        unreachable!()
    };
    let inner = signed.hash_as_entrypoint();
    let sealed = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
        Hash::new(b"sealed source"),
        signed,
        [4; 32],
    ));
    let mut block = network_index_block_at(2, vec![first.clone(), sealed.clone()]);
    let mut outputs = block.execution_outputs().to_vec();
    let ExecutionOutputV1::Network(row) = &mut outputs[0] else {
        unreachable!()
    };
    row.result = iroha_data_model::transaction::TransactionResult::new(Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted("index fixture rejection".into()),
        ),
    ));
    outputs.push(network_index_internal_output());
    install_network_index_test_outputs(&mut block, outputs);
    let height = nonzero!(2_usize);
    let mut index = super::TransactionEntrypointIndex::complete_empty();
    Kura::insert_transaction_entrypoint_heights(&mut index, height, &block);
    assert!(index.indexed_heights.contains(&height));
    assert!(index.incomplete_heights.is_empty());
    assert_eq!(index.heights_by_entrypoint.len(), 2);
    assert!(index.heights_by_entrypoint.contains_key(&first.hash()));
    assert!(index.heights_by_entrypoint.contains_key(&sealed.hash()));
    assert!(!index.heights_by_entrypoint.contains_key(&inner));
    assert_eq!(
        index
            .heights_by_result_status
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [false, true]
    );
    let by_input =
        &index.kaigi_signal_candidates[&kaigi_signal_test_call("canonical-inputs")][&height];
    assert_eq!(
        by_input.len(),
        1,
        "rejected Network and internal Time are not signal candidates"
    );
    let locator = &by_input[&1];
    assert_eq!(locator.position.network_input_index(), 1);
    assert_eq!(locator.position.entrypoint_hash(), sealed.hash());
    assert_eq!(block.execution_outputs().len(), 3);
    assert_eq!(
        index.inventories_by_height[&height].entrypoint_hashes.len(),
        2
    );
    // Internal success must not introduce a successful Network result membership.
    let mut internal_only = network_index_block_at(2, Vec::new());
    install_network_index_test_outputs(&mut internal_only, vec![network_index_internal_output()]);
    Kura::insert_transaction_entrypoint_heights(&mut index, height, &internal_only);
    assert!(index.indexed_heights.contains(&height));
    assert!(index.heights_by_result_status.is_empty());
    assert!(index.heights_by_entrypoint.is_empty());
    assert!(index.kaigi_signal_candidates.is_empty());
}

#[test]
fn canonical_network_index_refuses_whole_malformed_carrier_before_membership() {
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    let block = network_index_block_at(
        2,
        vec![network_index_signal_input(1), network_index_signal_input(2)],
    );
    let height = nonzero!(2_usize);
    for mutation in 0..6 {
        let mut value = norito::json::to_value(&block).unwrap();
        let result = value
            .as_object_mut()
            .unwrap()
            .get_mut("result")
            .unwrap()
            .as_object_mut()
            .unwrap();
        let mut outputs = block.execution_outputs().to_vec();
        match mutation {
            0 => {
                outputs.pop();
            }
            1 => {
                let ExecutionOutputV1::Network(row) = &mut outputs[1] else {
                    unreachable!()
                };
                row.input_index = 0;
            }
            2 => {
                outputs[1] = network_index_internal_output();
            }
            3 => {
                outputs.swap(0, 1);
            }
            4 => {
                result.insert(
                    "output_merkle".into(),
                    norito::json::to_value(
                        &iroha_crypto::MerkleTree::<ExecutionOutputV1>::default(),
                    )
                    .unwrap(),
                );
            }
            _ => {}
        }
        if mutation < 4 {
            result.insert("outputs".into(), norito::json::to_value(&outputs).unwrap());
        }
        let malformed: SignedBlock = if mutation == 5 {
            block.canonical_resultless_proposal()
        } else {
            norito::json::from_value(value).unwrap()
        };
        let mut index = super::TransactionEntrypointIndex::complete_empty();
        Kura::insert_transaction_entrypoint_heights(&mut index, height, &block);
        Kura::insert_transaction_entrypoint_heights(&mut index, height, &malformed);
        assert!(!index.complete);
        assert!(
            index.incomplete_heights.contains(&height),
            "mutation {mutation}"
        );
        assert!(index.indexed_heights.is_empty());
        assert!(index.heights_by_entrypoint.is_empty());
        assert!(index.heights_by_authority.is_empty());
        assert!(index.heights_by_timestamp_ms.is_empty());
        assert!(index.heights_by_result_status.is_empty());
        assert!(index.kaigi_signal_candidates.is_empty());
        assert!(index.inventories_by_height.is_empty());
        Kura::truncate_transaction_entrypoint_index_to(&mut index, 0);
        assert!(index.complete);
        assert!(index.incomplete_heights.is_empty());
    }
}

#[test]
fn canonical_network_index_rebuild_missing_body_and_replacement_stay_explicit() {
    let first = network_index_block_at(1, vec![network_index_signal_input(1)]);
    let second = network_index_block_at(2, vec![network_index_signal_input(2)]);
    let data: BlockData = [(first.hash(), Some(Arc::new(first))), (second.hash(), None)]
        .into_iter()
        .collect();
    let mut index = Kura::build_transaction_entrypoint_index(&data);
    assert!(!index.complete);
    assert_eq!(index.indexed_heights, BTreeSet::from([nonzero!(1_usize)]));
    assert!(index.incomplete_heights.contains(&nonzero!(2_usize)));
    Kura::insert_transaction_entrypoint_heights(&mut index, nonzero!(2_usize), &second);
    assert!(index.incomplete_heights.is_empty());
    let prior = second.network_entrypoint_at(0).unwrap().hash();
    let replacement = network_index_block_at(2, vec![network_index_signal_input(3)]);
    Kura::insert_transaction_entrypoint_heights(&mut index, nonzero!(2_usize), &replacement);
    assert!(!index.heights_by_entrypoint.contains_key(&prior));
    assert!(
        index
            .heights_by_entrypoint
            .contains_key(&replacement.network_entrypoint_at(0).unwrap().hash())
    );
    Kura::truncate_transaction_entrypoint_index_to(&mut index, 1);
    assert!(index.complete);
    assert_eq!(index.heights_by_entrypoint.len(), 1);
}

#[test]
fn canonical_network_position_roundtrips_and_refuses_retired_phase_layout() {
    use norito::codec::{DecodeAll, Encode};
    let position = kaigi_signal_test_locator(3, 7).position;
    let bytes = norito::encode_canonical(&position).unwrap();
    assert_eq!(
        norito::decode_canonical::<super::KaigiSignalCandidatePosition>(&bytes).unwrap(),
        position
    );
    #[derive(norito::Encode)]
    struct RetiredPosition {
        block_height: u64,
        execution_phase: u8,
        transaction_index: u64,
        block_hash: HashOf<BlockHeader>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
    }
    let old = RetiredPosition {
        block_height: 3,
        execution_phase: 1,
        transaction_index: 7,
        block_hash: position.block_hash(),
        entrypoint_hash: position.entrypoint_hash(),
    };
    assert!(super::KaigiSignalCandidatePosition::decode_all(&mut old.encode().as_slice()).is_err());
}

fn network_index_native_block() -> SignedBlock {
    use iroha_data_model::block::{
        consensus_v2 as wire, lane_admission::*, lane_consensus::*,
        lane_decision_batch::LaneDecisionBatchV1, lane_input::*,
    };
    use iroha_model_base::peer::PeerId;
    let network = test_network_id(b"canonical-index");
    let input = network_index_signal_input(7);
    let signed_hash = match &input {
        TransactionEntrypoint::External(tx) => tx.hash(),
        _ => unreachable!(),
    };
    let route = crate::queue::RoutingDecision::new(LaneId::new(2), DataSpaceId::UNIVERSAL);
    let plan = crate::queue::RoutingPlan::single(route);
    let validators: Vec<_> = (1..=4)
        .map(|seed| {
            PeerId::new(
                KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .public_key()
                    .clone(),
            )
        })
        .collect();
    let incarnation = Hash::new(b"structural native route");
    let instance = Hash::new(b"structural native instance");
    let binding = QueuePlanAdmissionBindingV1 {
        version: QUEUE_PLAN_ADMISSION_BINDING_VERSION_V1,
        network_id_digest: queue_plan_admission_network_id_digest(&network),
        request_id: queue_plan_synced_request_id(&network, input.hash()),
        entrypoint_hash: input.hash(),
        signed_transaction_hash: Some(signed_hash),
        routing_plan_digest: plan.digest(),
        admission_context: QueuePlanAdmissionContextV1 {
            version: QUEUE_PLAN_ADMISSION_CONTEXT_VERSION_V1,
            authority_height: 1,
            proposal_height: 2,
            predecessor_block_hash: Some(HashOf::from_untyped_unchecked(Hash::new(
                b"structural admission predecessor",
            ))),
            routing_plan_digest: plan.digest(),
            route_incarnations: vec![QueuePlanRouteIncarnationV1 {
                leg: plan.legs()[0],
                lane_incarnation: incarnation,
                validator_set_hash_version:
                    iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validators),
                validator_set: validators,
                validator_count: 4,
                durability_threshold: 2,
            }],
        },
        enqueue_timestamp_ms: 7,
        queue_plan_journal_version: QUEUE_PLAN_JOURNAL_CLAIM_VERSION_V1,
        durable_admission_version: QUEUE_PLAN_DURABLE_ADMISSION_VERSION_V1,
        journal_record_digest: Hash::new(b"structural index fixture, not physical admission"),
    };
    let admitted = LaneAdmittedInputV1 {
        entrypoint: input,
        certificate: QueuePlanAdmissionCertificateV1 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V1,
            binding,
            attestations: Vec::new(),
        },
    };
    let payload = LaneInputPayloadV1 {
        descriptor: LaneInputDescriptorV1 {
            version: LANE_INPUT_VERSION_V1,
            admission_priority: QueuePlanAdmissionPriorityV1::new(2, 0).unwrap(),
            admission_carrier_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"structural first carrier",
            )),
            admitted_input_hash: Hash::new(norito::encode_canonical(&admitted).unwrap()),
            slots: vec![LaneInputRouteSlotV1 {
                route,
                lane_incarnation: incarnation,
                instance_id: instance,
                lane_height: 1,
            }],
        },
        input: admitted,
    };
    let bytes = norito::encode_canonical(&payload).unwrap();
    let layout = wire::recommended_data_availability_layout();
    let chunks = wire::encode_payload_chunks(layout, &bytes).unwrap();
    let root = wire::payload_chunk_root(&chunks.iter().map(Hash::new).collect::<Vec<_>>()).unwrap();
    let value = LaneValueRefV1 {
        instance_id: instance,
        admitted_binding_hash: payload.input.certificate.binding.canonical_hash(),
        kind: payload.validate_structure().unwrap(),
        origin_view: 0,
        origin_producer: 0,
        descriptor_hash: payload.descriptor.canonical_hash().unwrap(),
        payload_hash: Hash::new(&bytes),
        availability_hash: lane_availability_hash(
            layout,
            root,
            bytes.len() as u64,
            chunks.len() as u32,
        )
        .unwrap(),
    };
    // The index validates DTO structure only; this fixture cannot authenticate native consensus.
    let decision = LaneDecisionV1 {
        manifest: LaneManifestV1 {
            value,
            layout,
            chunk_root: root,
            byte_len: bytes.len() as u64,
            chunk_count: chunks.len() as u32,
        },
        commit_qc: LaneQcV1 {
            statement: LaneVoteStatementV1 {
                round: LaneRoundV1 {
                    instance_id: instance,
                    lane_height: 1,
                    voting_view: 0,
                },
                phase: LanePhaseV1::Commit,
                value,
            },
            shares: Vec::new(),
        },
    };
    let batch = LaneDecisionBatchV1 {
        base_state_height: 2,
        base_state_hash: HashOf::from_untyped_unchecked(Hash::new(b"structural WSV base")),
        groups: vec![LaneDecisionGroupV1 {
            payload,
            decisions: vec![decision],
        }],
    };
    let mut block = network_index_block_at(3, Vec::new()).canonical_resultless_proposal();
    block.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_native_lane_decisions(batch),
    ));
    attach_ok_results_to_block(&mut block);
    block
}

#[test]
fn canonical_network_index_projects_native_source_once_and_rejects_mixed_body() {
    let block = network_index_native_block();
    assert_eq!(block.external_entrypoint_count(), 0);
    assert!(block.header().merkle_root().is_none());
    assert_eq!(block.network_entrypoint_count(), 1);
    let source = block.network_entrypoint_at(0).unwrap();
    let height = nonzero!(3_usize);
    let mut index = super::TransactionEntrypointIndex::complete_empty();
    Kura::insert_transaction_entrypoint_heights(&mut index, height, &block);
    assert!(index.incomplete_heights.is_empty());
    assert_eq!(
        index
            .heights_by_entrypoint
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        [source.hash()]
    );
    let page = Kura::collect_kaigi_signal_candidate_locators(
        &index,
        &kaigi_signal_test_call("canonical-inputs"),
        3,
        None,
        nonzero!(1_usize),
    )
    .unwrap();
    assert_eq!(page.candidates.len(), 1);
    assert_eq!(page.candidates[0].position.network_input_index(), 0);
    assert_eq!(page.candidates[0].position.entrypoint_hash(), source.hash());
    let mut mixed = block.clone();
    mixed.set_external_entrypoints(vec![network_index_signal_input(99)]);
    Kura::insert_transaction_entrypoint_heights(&mut index, height, &mixed);
    assert!(index.incomplete_heights.contains(&height));
    assert!(index.heights_by_entrypoint.is_empty());
    assert!(index.kaigi_signal_candidates.is_empty());
    let mut missing = norito::json::to_value(&block).unwrap();
    let context = missing
        .as_object_mut()
        .unwrap()
        .get_mut("payload")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .get_mut("execution_context")
        .unwrap()
        .as_object_mut()
        .unwrap();
    let batch = context
        .get_mut("native_lane_decisions")
        .unwrap()
        .as_object_mut()
        .unwrap();
    batch
        .get_mut("groups")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .clear();
    let missing: SignedBlock = norito::json::from_value(missing).unwrap();
    Kura::insert_transaction_entrypoint_heights(&mut index, height, &missing);
    assert!(index.incomplete_heights.contains(&height));
    assert!(index.indexed_heights.is_empty());
}
