struct NativeBodyRecoveryPayload {
    transaction: iroha_data_model::transaction::signed::SignedTransaction,
    request: NativeAmxAttestationRequestV2,
    receipt: NativeAmxReceipt,
    routing_plan: RoutingPlan,
    source_id: [u8; Hash::LENGTH],
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}

struct NativeBodyRecoveryFixture {
    adapter: V2LaneWorkAdapter,
    carrier: SignedBlock,
    finality: wire::finality::V2FinalityArtifact,
    manifest: crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    marker: crate::state::AppliedNativeAmxParticipantFrontierMarker,
    source_id: [u8; Hash::LENGTH],
    entrypoint_hash: HashOf<TransactionEntrypoint>,
}

fn native_body_recovery_adapter() -> (V2LaneWorkAdapter, Vec<KeyPair>, LaneId, DataSpaceId) {
    let capacity = NonZeroUsize::new(8).expect("non-zero fixture capacity");
    let limits = V2LaneWorkLimits::new(
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        capacity,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS,
        iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_BLOCK_SYNC,
        iroha_config::parameters::defaults::sumeragi::V2_AUTHENTICATED_MERGE_QC_CAPACITY,
        iroha_config::parameters::defaults::sumeragi::V2_MERGE_LEADER_BODY_FRAME_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_CARRIER_HEADROOM_BYTES,
        iroha_config::parameters::defaults::sumeragi::V2_AUTONOMOUS_PRODUCER_RECHECK,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_STUCK_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_RETRY_TIER_ATTEMPTS,
        iroha_config::parameters::defaults::sumeragi::V2_HISTORICAL_RECOVERY_MAX_RETRY_TIER,
        iroha_config::parameters::defaults::sumeragi::V2_SIDECAR_SERVICE_BURST,
        MergeSidecarLimits::defaults(),
        MergeSigningGuardLimits::defaults(),
        NativeAmxSigningGuardLimits::new(
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_CAPACITY,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        )
        .expect("default Native AMX signing limits"),
    );
    let kura = Kura::blank_kura_for_testing_with_blocks_in_memory(
        NonZeroUsize::new(1).expect("retain one carrier body"),
    );
    let (mut adapter, keys) = fixture_at_height_inner_with_limits_and_kura(
        wire::ConsensusMode::Permissioned,
        4,
        true,
        limits,
        kura,
    );
    let participant_lane = LaneId::new(1);
    let participant_dataspace = DataSpaceId::new(7);
    enable_multilane_nexus(&mut adapter, &keys, participant_lane, participant_dataspace);
    let entry = adapter
        .state
        .nexus_snapshot()
        .lane_config
        .entry(participant_lane)
        .expect("participant lane storage entry")
        .clone();
    adapter
        .kura
        .reconcile_lane_segments_for_testing(&[&entry], &[], &[])
        .expect("provision participant lane storage");
    let incarnation = adapter
        .state
        .lane_incarnation_at_height(participant_lane, adapter.context.height)
        .expect("participant lane incarnation");
    adapter
        .kura
        .install_lane_incarnation_marker_for_test(&entry, incarnation, 0)
        .expect("install participant lane incarnation marker");
    (adapter, keys, participant_lane, participant_dataspace)
}

fn native_body_recovery_payload(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
) -> NativeBodyRecoveryPayload {
    let transaction_key =
        KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::Ed25519).expect("transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.chain_id.clone(),
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let transaction_hash = transaction.hash();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(transaction_hash.as_ref());
    let mut request = native_request_with_entrypoint(
        native_request_with_distinct_participant(
            adapter,
            keys,
            participant_lane,
            participant_dataspace,
            1,
            None,
        ),
        entrypoint_hash,
    );
    request.body.source_id = source_id;
    request.participant_settlement = request
        .body
        .computed_grouped_participant_settlement(&[source_id])
        .expect("derive exact participant settlement");
    let settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&request.participant_settlement)
            .expect("hash exact participant settlement");
    request.body.participant_settlement_commitment = Hash::from(settlement_hash);
    request
        .validate_plan_binding()
        .expect("exact Native request binding");
    let prepare_qc = native_qc_for_body(request.body, keys);
    let mut commit_body = request.body;
    commit_body.phase = NativeAmxPhase::Commit;
    let leg = NativeAmxLegRecordV2 {
        lane_id: participant_lane,
        dataspace_id: participant_dataspace,
        participant_proposal: request.participant_proposal.clone(),
        participant_settlement: request.participant_settlement.clone(),
        participant_settlement_hash: settlement_hash,
        prepare_qc,
        commit_qc: native_qc_for_body(commit_body, keys),
    };
    let coordinator = RoutingDecision::new(
        request.body.coordinator_lane_id,
        request.body.coordinator_dataspace_id,
    );
    let participant = RoutingDecision::new(participant_lane, participant_dataspace);
    let routing_plan = RoutingPlan::native_amx(
        coordinator,
        vec![RouteLeg::new(participant, RouteLegRole::Participant)],
    );
    let receipt = adapter
        .assemble_native_receipt(
            source_id,
            coordinator,
            request.body.plan_digest,
            &request.coordinator_proposal,
            vec![leg],
        )
        .expect("assemble exact Native receipt");
    NativeBodyRecoveryPayload {
        transaction,
        request,
        receipt,
        routing_plan,
        source_id,
        entrypoint_hash,
    }
}

fn native_body_recovery_carrier(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    payload: &NativeBodyRecoveryPayload,
) -> SignedBlock {
    let parent_hash = adapter
        .kura
        .get_block(NonZeroUsize::new(3).expect("non-zero parent height"))
        .expect("durable carrier parent")
        .hash();
    let header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
        Some(parent_hash),
        None,
        None,
        adapter.context.height,
        0,
    );
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
    let initial_signature =
        SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
            .expect("sign initial Native carrier");
    let mut carrier = SignedBlock::presigned(
        BlockSignature::new(
            u64::try_from(leader_index).expect("leader index fits u64"),
            initial_signature,
        ),
        header,
        vec![payload.transaction.clone()],
    );
    let coordinator = payload.routing_plan.coordinator_route();
    carrier.set_execution_context(Some(BlockExecutionContextBundle::new(vec![
        ExternalExecutionContext::with_routing_plan(
            payload.entrypoint_hash,
            coordinator.lane_id,
            coordinator.dataspace_id,
            payload.routing_plan.digest(),
            crate::queue::execution_context_legs_for_routing_plan(&payload.routing_plan),
        )
        .with_native_amx_receipt(payload.receipt.clone()),
    ])));
    carrier
        .set_transaction_results(
            Vec::new(),
            &[payload.entrypoint_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach exact Native carrier result");
    let final_signature =
        SignatureOf::try_from_hash(keys[leader_index].private_key(), carrier.header().hash())
            .expect("sign finalized Native carrier");
    carrier
        .replace_signatures(
            [BlockSignature::new(
                u64::try_from(leader_index).expect("leader index fits u64"),
                final_signature,
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace Native carrier signature");
    carrier
}

fn native_body_recovery_finality(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    carrier: &SignedBlock,
) -> (
    crate::sumeragi::exec::NativeAmxApplicationManifestV1,
    wire::finality::V2FinalityArtifact,
) {
    let manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(carrier)
            .expect("derive exact Native application manifest");
    assert_eq!(manifest.count(), 1);
    let commitment =
        wire::ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"Native generic recovery parent state"),
            Hash::new(b"Native generic recovery post state"),
            Hash::new(b"Native generic recovery ordinary writes"),
            None,
            0,
            wire::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest.root(),
            manifest.count(),
            manifest.executed_block_wire_len(),
            manifest.executed_block_wire_hash(),
        )
        .expect("construct exact Native execution commitment");
    let finality = verified_finality_artifact_for_block_with_execution_commitment(
        adapter, keys, carrier, commitment,
    );
    (manifest, finality)
}

fn persist_and_evict_native_body(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    carrier: &SignedBlock,
    finality: &wire::finality::V2FinalityArtifact,
) -> crate::state::AppliedNativeAmxParticipantFrontierMarker {
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist Native carrier");
    let finality_receipt = adapter
        .kura
        .store_v2_finality_artifact(finality)
        .expect("persist Native finality");
    assert_eq!(finality_receipt.height(), carrier.header().height().get());
    assert_eq!(finality_receipt.block_hash(), carrier.hash());
    let committed = ValidBlock::committed_from_replay_signed_block(carrier.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(adapter.state.as_ref());
    adapter
        .kura
        .store_wsv_checkpoint(carrier.header().height().get(), carrier.hash(), checkpoint)
        .expect("persist Native WSV checkpoint");
    adapter
        .kura
        .store_commit_manifest(
            crate::kura::CommitManifest::new(
                carrier.header().height().get(),
                carrier.hash(),
                None,
                None,
                checkpoint,
                None,
            )
            .with_authenticated_v2_commit_authority(finality),
        )
        .expect("persist authenticated Native commit manifest");
    let marker = adapter
        .state
        .native_amx_participant_frontiers_pending_durable_evidence_snapshot_cached()
        .expect("inspect pending Native frontier")
        .into_iter()
        .next()
        .expect("one pending Native frontier");
    let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
    let mut tail_parent = carrier.hash();
    for height in 5..=6 {
        let tail = test_block(height, Some(tail_parent), None, &keys[leader_index]);
        tail_parent = tail.hash();
        adapter
            .kura
            .store_block(tail.clone())
            .expect("persist eviction tail block");
        commit_test_block_to_state(
            adapter.state.as_ref(),
            &ValidBlock::committed_from_replay_signed_block(tail),
            &adapter.context,
        );
    }
    let (carrier_height, payload_len) = adapter
        .kura
        .durable_block_payload_len_by_hash(carrier.hash())
        .expect("inspect durable carrier payload");
    let height = NonZeroUsize::new(usize::try_from(carrier_height).expect("height fits usize"))
        .expect("non-zero carrier height");
    assert_eq!(
        adapter.kura.advertise_required_replicas_for_bench(height),
        Some(payload_len),
        "fixture must install the deterministic selected-keeper quorum"
    );
    assert_eq!(
        adapter
            .kura
            .evict_block_bodies(payload_len)
            .expect("evict exact Native carrier"),
        payload_len
    );
    adapter
        .kura
        .remove_evicted_block_sidecar_for_testing(height)
        .expect("remove local Native carrier sidecar");
    assert!(adapter.kura.get_block(height).is_none());
    marker
}

fn native_body_recovery_fixture() -> NativeBodyRecoveryFixture {
    let (adapter, keys, participant_lane, participant_dataspace) = native_body_recovery_adapter();
    let payload =
        native_body_recovery_payload(&adapter, &keys, participant_lane, participant_dataspace);
    assert_eq!(
        payload.routing_plan.digest(),
        payload.request.body.plan_digest
    );
    let carrier = native_body_recovery_carrier(&adapter, &keys, &payload);
    let (manifest, finality) = native_body_recovery_finality(&adapter, &keys, &carrier);
    let marker = persist_and_evict_native_body(&adapter, &keys, &carrier, &finality);
    NativeBodyRecoveryFixture {
        adapter,
        carrier,
        finality,
        manifest,
        marker,
        source_id: payload.source_id,
        entrypoint_hash: payload.entrypoint_hash,
    }
}

#[test]
fn native_participant_missing_carrier_uses_generic_chunk_recovery_then_repairs_receipt() {
    let fixture = native_body_recovery_fixture();
    let context = &fixture.adapter.context;
    let state = fixture.adapter.state.as_ref();
    let kura = fixture.adapter.kura.as_ref();
    let planning =
        plan_lane_application_evidence_repair(context, state, kura, fixture.adapter.limits)
            .expect("plan missing Native carrier recovery");
    let LaneApplicationEvidenceRepairPlanning::RecoverCanonicalBodies(needs) = planning else {
        panic!("missing Native carrier must enter generic body recovery");
    };
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0].height, fixture.marker.application_block_height);
    assert_eq!(needs[0].block_hash, fixture.marker.application_block_hash);
    assert_eq!(
        needs[0].finality_artifact_hash,
        HashOf::new(&fixture.finality)
    );
    assert_eq!(
        needs[0].executed_block_wire_hash,
        fixture.manifest.executed_block_wire_hash()
    );

    kura.cache_block_body(&fixture.carrier)
        .expect("simulate authenticated generic chunk assembly");
    let planning =
        plan_lane_application_evidence_repair(context, state, kura, fixture.adapter.limits)
            .expect("replan Native evidence after body recovery");
    let LaneApplicationEvidenceRepairPlanning::Ready(plan) = planning else {
        panic!("recovered body must make the complete publication plan ready");
    };
    let summary = apply_lane_application_evidence_repair(state, kura, plan)
        .expect("publish exact Native application evidence");
    assert_eq!(summary.native_carriers, 1);
    assert_eq!(summary.native_routes, 1);
    assert!(
        state
            .native_amx_participant_frontiers_pending_durable_evidence_snapshot_cached()
            .expect("read repaired Native frontier")
            .is_empty()
    );
    let receipt = kura
        .read_native_amx_participant_application_receipt(
            fixture.marker.lane_id,
            fixture.marker.dataspace_id,
            fixture.marker.lane_incarnation,
            fixture.marker.lane_block_height,
        )
        .expect("read repaired Native receipt");
    assert_eq!(receipt.application_block_hash, fixture.carrier.hash());
    assert_eq!(
        receipt.executed_block_wire_hash,
        fixture.manifest.executed_block_wire_hash()
    );
    assert_eq!(receipt.source_ids, vec![fixture.source_id]);
    assert_eq!(receipt.entrypoint_hashes, vec![fixture.entrypoint_hash]);
}
