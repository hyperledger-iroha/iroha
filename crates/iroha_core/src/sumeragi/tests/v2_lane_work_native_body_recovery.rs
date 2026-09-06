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
    native_body_recovery_adapter_with_kura(locked_lane_work_test_kura(
        NonZeroUsize::new(1).expect("retain one carrier body"),
    ))
}
fn native_body_recovery_adapter_with_kura(
    kura: Arc<Kura>,
) -> (V2LaneWorkAdapter, Vec<KeyPair>, LaneId, DataSpaceId) {
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
        std::time::Duration::from_millis(10),
        std::time::Duration::from_secs(1),
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
struct GroupedNativeCandidateFixture {
    service: crate::sumeragi::v2_apply::V2ApplyService,
    context: wire::HeightContext,
    body: SignedBlock,
    state: Arc<State>,
    kura: Arc<Kura>,
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
    participant_incarnation: Hash,
    participant_height: u64,
}
#[allow(clippy::too_many_lines)]
fn grouped_native_candidate_fixture(
    pending_control_validation_bytes: Option<NonZeroUsize>,
) -> GroupedNativeCandidateFixture {
    let mut kura =
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY);
    if let Some(aggregate_bytes) = pending_control_validation_bytes {
        Arc::get_mut(&mut kura)
            .expect("fresh grouped Native fixture Kura has one owner")
            .set_pending_control_sidecar_validation_bytes_for_testing(aggregate_bytes);
    }
    let (mut adapter, keys, participant_lane, participant_dataspace) =
        native_body_recovery_adapter_with_kura(kura);
    let mut finality_manifest_root = [0_u8; Hash::LENGTH];
    finality_manifest_root
        .copy_from_slice(Hash::new(b"grouped Native coordinator finality manifest").as_ref());
    Arc::get_mut(&mut adapter.state)
        .expect("fresh grouped Native fixture owns its State")
        .set_axt_policy(
            DataSpaceId::UNIVERSAL,
            iroha_data_model::nexus::AxtPolicyEntry {
                manifest_root: finality_manifest_root,
                target_lane: LaneId::SINGLE,
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            },
        );
    let transaction_key =
        KeyPair::try_from_seed(vec![0xD8; 32], Algorithm::Ed25519).expect("transaction key");
    let authority = AccountId::new(transaction_key.public_key().clone());
    let authority_domain =
        DomainId::try_new("budgetauthority", "universal").expect("authority domain id");
    let mut world = adapter.state.world.block();
    world.domains.insert(
        authority_domain.clone(),
        Domain::new(authority_domain).build(&authority),
    );
    world.accounts.insert(
        authority.clone(),
        AccountValue::new(AccountDetails::default()),
    );
    world.commit();
    let transaction_time = TimeSource::new_fixed(Duration::from_secs(4));
    let mut transactions = [
        ("budgetuniversalone", "budgetindependentone"),
        ("budgetuniversaltwo", "budgetindependenttwo"),
    ]
    .into_iter()
    .map(|(universal_name, participant_name)| {
        TransactionBuilder::new_with_time_source(
            adapter.context.network_id,
            authority.clone(),
            &transaction_time,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new(universal_name, "universal")
                    .expect("universal fixture domain id"),
            ))),
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new(participant_name, "independent-dataspace")
                    .expect("participant fixture domain id"),
            ))),
        ])
        .sign(transaction_key.private_key())
    })
    .collect::<Vec<_>>();
    transactions.sort_by_key(|transaction| transaction.hash());
    let source_ids = transactions
        .iter()
        .map(|transaction| {
            let mut source_id = [0_u8; Hash::LENGTH];
            source_id.copy_from_slice(transaction.hash().as_ref());
            source_id
        })
        .collect::<Vec<_>>();
    assert!(
        source_ids.windows(2).all(|pair| pair[0] < pair[1]),
        "grouped Native sources must be in canonical transaction order"
    );
    let entrypoint_hashes = transactions
        .iter()
        .map(|transaction| transaction.hash_as_entrypoint())
        .collect::<Vec<_>>();
    let parent_height = NonZeroUsize::new(
        usize::try_from(
            adapter
                .context
                .height
                .checked_sub(1)
                .expect("grouped Native candidate is non-genesis"),
        )
        .expect("parent height fits usize"),
    )
    .expect("parent height is non-zero");
    let parent = adapter
        .kura
        .get_block(parent_height)
        .expect("durable grouped Native candidate parent");
    let block_cadence = Duration::from_secs(1);
    let mut creation_time = parent
        .header()
        .creation_time()
        .checked_add(block_cadence)
        .expect("grouped Native block time fits Duration");
    for transaction in &transactions {
        creation_time = creation_time.max(
            transaction
                .creation_time()
                .checked_add(Duration::from_millis(1))
                .expect("grouped Native transaction time fits Duration"),
        );
    }
    let creation_time_ms =
        u64::try_from(creation_time.as_millis()).expect("grouped Native block time fits u64");
    let routing_plans = {
        let state_view = adapter.state.view();
        transactions
            .iter()
            .map(|transaction| {
                let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
                crate::queue::evaluate_policy_plan_with_nexus_and_world_at_block_height(
                    &state_view.nexus,
                    &accepted,
                    state_view.world(),
                    creation_time_ms,
                    adapter.context.height,
                )
                .expect("cross-dataspace fixture transaction derives a Native plan")
            })
            .collect::<Vec<_>>()
    };
    assert!(
        routing_plans
            .iter()
            .all(|plan| matches!(plan, RoutingPlan::NativeAmx(_))),
        "both grouped transactions must use Native AMX"
    );
    assert_eq!(
        routing_plans[0], routing_plans[1],
        "grouped transactions must derive one exact routing plan"
    );
    let routing_plan = routing_plans[0].clone();
    let coordinator = routing_plan.coordinator_route();
    assert_eq!(coordinator, RoutingDecision::default());
    assert_eq!(
        routing_plan.legs()[1].route,
        RoutingDecision::new(participant_lane, participant_dataspace)
    );
    let coordinator_routes = routing_plans
        .iter()
        .map(RoutingPlan::coordinator_route)
        .collect::<Vec<_>>();
    let candidate_hashes = entrypoint_hashes
        .iter()
        .copied()
        .map(Hash::from)
        .collect::<Vec<_>>();
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("global leader index fits usize");
    let lane_plan = prepare_v2_lane_payload_plan(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        0,
        &adapter.context.roster[leader_index].validator,
        &coordinator_routes,
        &candidate_hashes,
    )
    .expect("derive exact grouped coordinator lane plan");
    assert!(lane_plan.unavailable_indices.is_empty());
    assert_eq!(lane_plan.ownerships.len(), 1);
    assert_eq!(lane_plan.proposals.len(), 1);
    assert_eq!(
        lane_plan.ownerships[0].accepted_candidate_indices,
        vec![0, 1]
    );
    assert_eq!(
        lane_plan.ownerships[0].accepted_transaction_hashes,
        candidate_hashes
    );
    let coordinator_proposal = lane_plan.proposals[0].clone();
    let participant_incarnation = adapter
        .state
        .lane_incarnation_at_height(participant_lane, adapter.context.height)
        .expect("active grouped participant incarnation");
    let participant_base = proposal_for_route(
        &adapter,
        &keys,
        participant_lane,
        participant_dataspace,
        participant_incarnation,
        adapter.context.height,
        1,
    );
    let mut participant_ownership = ownership_from_proposal(&participant_base);
    participant_ownership.accepted_candidate_indices = vec![0, 1];
    participant_ownership.accepted_transaction_hashes = candidate_hashes.clone();
    let participant_replay = participant_ownership
        .compute_replay_hashes()
        .expect("grouped participant ownership replay material");
    participant_ownership.subject_hash = participant_replay.subject_hash;
    participant_ownership.payload_ownership_hash = participant_replay.payload_ownership_hash;
    participant_ownership.rbc_instance_hash = participant_replay.rbc_instance_hash;
    participant_ownership.lane_block_descriptor_hash =
        Some(participant_replay.lane_block_descriptor_hash);
    let mut participant_proposal = proposal_from_ownership(
        &participant_ownership,
        HashOf::from_untyped_unchecked(Hash::new(b"grouped Native participant proposal hint")),
    )
    .expect("reconstruct exact grouped participant proposal");
    participant_proposal.payload_block_hint = None;
    crate::lane_consensus::validate_lane_block_proposal(&participant_proposal)
        .expect("grouped participant proposal is structurally valid");
    let bind_request = |source_id: [u8; Hash::LENGTH],
                        entrypoint_hash: HashOf<TransactionEntrypoint>| {
        let coordinator_descriptor = &coordinator_proposal.descriptor;
        let participant_descriptor = &participant_proposal.descriptor;
        let mut request = native_request_with_distinct_participant(
            &adapter,
            &keys,
            participant_lane,
            participant_dataspace,
            coordinator_descriptor.lane_block_height,
            coordinator_descriptor.previous_lane_block_descriptor_hash,
        );
        request.plan_legs = routing_plan.legs();
        request.coordinator_proposal = coordinator_proposal.clone();
        request.participant_proposal = participant_proposal.clone();
        let body = &mut request.body;
        body.source_id = source_id;
        body.tx_entrypoint_hash = entrypoint_hash;
        body.plan_digest = routing_plan.digest();
        body.coordinator_lane_id = coordinator_descriptor.lane_id;
        body.coordinator_dataspace_id = coordinator_descriptor.dataspace_id;
        body.coordinator_lane_incarnation = coordinator_descriptor.lane_incarnation;
        body.planned_coordinator_block_height = coordinator_descriptor.lane_block_height;
        body.coordinator_lane_block_view = coordinator_descriptor.lane_block_view;
        body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
        body.participant_lane_id = participant_descriptor.lane_id;
        body.participant_dataspace_id = participant_descriptor.dataspace_id;
        body.participant_lane_incarnation = participant_descriptor.lane_incarnation;
        body.participant_previous_block_height = participant_descriptor.previous_lane_block_height;
        body.participant_previous_block_descriptor_hash =
            participant_descriptor.previous_lane_block_descriptor_hash;
        body.participant_lane_block_height = participant_descriptor.lane_block_height;
        body.participant_lane_block_view = participant_descriptor.lane_block_view;
        body.participant_proposal_hash = participant_proposal.proposal_hash;
        body.participant_validator_set_hash = participant_descriptor.validator_set_hash;
        body.participant_validator_count = participant_descriptor.validator_count;
        body.participant_min_quorum = participant_descriptor.min_quorum;
        request
    };
    let template = bind_request(source_ids[0], entrypoint_hashes[0]);
    let participant_settlement = template
        .body
        .computed_grouped_participant_settlement(&source_ids)
        .expect("derive exact grouped participant settlement");
    let participant_settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
            .expect("hash exact grouped participant settlement");
    let receipts = source_ids
        .iter()
        .copied()
        .zip(entrypoint_hashes.iter().copied())
        .map(|(source_id, entrypoint_hash)| {
            let mut request = bind_request(source_id, entrypoint_hash);
            request.participant_settlement = participant_settlement.clone();
            request.body.participant_settlement_commitment =
                Hash::from(participant_settlement_hash);
            request
                .validate_plan_binding()
                .expect("exact grouped Native request binding");
            let prepare_qc = native_qc_for_body(request.body, &keys);
            let mut commit_body = request.body;
            commit_body.phase = NativeAmxPhase::Commit;
            let leg = NativeAmxLegRecordV2 {
                lane_id: participant_lane,
                dataspace_id: participant_dataspace,
                participant_proposal: request.participant_proposal,
                participant_settlement: request.participant_settlement,
                participant_settlement_hash,
                prepare_qc,
                commit_qc: native_qc_for_body(commit_body, &keys),
            };
            adapter
                .assemble_native_receipt(
                    source_id,
                    coordinator,
                    routing_plan.digest(),
                    &coordinator_proposal,
                    vec![leg],
                )
                .expect("assemble exact grouped Native receipt")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        receipts[0].legs[0].participant_proposal,
        receipts[1].legs[0].participant_proposal
    );
    assert_eq!(
        receipts[0].legs[0].participant_settlement,
        receipts[1].legs[0].participant_settlement
    );
    let external = entrypoint_hashes
        .iter()
        .copied()
        .zip(receipts)
        .map(|(entrypoint_hash, receipt)| {
            crate::queue::execution_context_for_routing_plan(entrypoint_hash, &routing_plan)
                .with_native_amx_receipt(receipt)
        })
        .collect::<Vec<_>>();
    let execution_context = BlockExecutionContextBundle::new(external)
        .with_lane_payload_ownerships(lane_plan.ownerships);
    let mut header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero grouped candidate height"),
        Some(parent.hash()),
        None,
        None,
        creation_time_ms,
        0,
    );
    let confidential_features = {
        let state_view = adapter.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            state_view.world(),
            &state_view.zk,
            state_view.sccp_registry.as_ref(),
            adapter.context.height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    header.set_confidential_features(confidential_features);
    let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
        &adapter.state.nexus_snapshot(),
        adapter.context.height,
    );
    let mut builder = BlockBuilder::new(header);
    for transaction in transactions {
        builder.push_transaction(transaction);
    }
    builder.set_da_proof_policies(Some(proof_policy_bundle));
    builder.set_execution_context(Some(execution_context));
    let body = builder
        .try_build_with_signature(
            u64::try_from(leader_index).expect("global leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .expect("sign grouped Native candidate")
        .canonical_resultless_proposal();
    assert!(body.is_resultless_proposal());
    assert_eq!(body.external_entrypoint_count(), 2);
    let state = Arc::clone(&adapter.state);
    let kura = Arc::clone(&adapter.kura);
    let context = adapter.context.clone();
    let validator_set_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("grouped Native validator PoP")
        })
        .collect::<Vec<_>>();
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(32);
    let queue = Arc::new(Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender.clone(),
    ));
    let service = crate::sumeragi::v2_apply::V2ApplyService::new(
        Arc::clone(&state),
        queue,
        Arc::clone(&kura),
        None,
        None,
        block_cadence,
        authority,
        events_sender,
        validator_set_pops,
    );
    GroupedNativeCandidateFixture {
        service,
        context,
        body,
        state,
        kura,
        participant_lane,
        participant_dataspace,
        participant_incarnation,
        participant_height: participant_proposal.descriptor.lane_block_height,
    }
}
#[test]
#[allow(clippy::too_many_lines)]
fn grouped_native_amx_prevote_rejects_undersized_evidence_budget_without_kura_or_wsv_mutation() {
    let positive = grouped_native_candidate_fixture(None);
    let positive_state_hash =
        crate::snapshot::canonical_state_snapshot_hash(positive.state.as_ref());
    let commitment = positive
        .service
        .validate_candidate(&positive.context, &positive.body)
        .expect("default evidence budget admits the exact grouped Native candidate");
    assert_eq!(commitment.native_amx_application_manifest_count, 1);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(positive.state.as_ref()),
        positive_state_hash,
        "positive pre-vote validation must discard its WSV overlay"
    );
    let negative = grouped_native_candidate_fixture(Some(
        NonZeroUsize::new(1).expect("non-zero undersized evidence budget"),
    ));
    assert_eq!(positive.context, negative.context);
    assert_eq!(positive.body, negative.body);
    assert_eq!(negative.state.committed_height(), 3);
    assert_eq!(
        negative
            .kura
            .exact_durable_blocks_count()
            .expect("read exact pre-vote durable height"),
        3
    );
    let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(negative.state.as_ref());
    let candidate_height = NonZeroUsize::new(
        usize::try_from(negative.context.height).expect("candidate height fits usize"),
    )
    .expect("candidate height is non-zero");
    assert!(negative.kura.get_block_hash(candidate_height).is_none());
    assert!(
        negative
            .kura
            .wsv_checkpoint(negative.context.height)
            .expect("read pre-vote WSV checkpoint")
            .is_none()
    );
    assert!(
        negative
            .kura
            .commit_manifest(negative.context.height)
            .expect("read pre-vote commit manifest")
            .is_none()
    );
    assert!(
        negative
            .kura
            .v2_finality_artifact(negative.context.height)
            .expect("read pre-vote finality")
            .is_none()
    );
    assert!(
        negative
            .kura
            .read_native_amx_participant_application_receipt(
                negative.participant_lane,
                negative.participant_dataspace,
                negative.participant_incarnation,
                negative.participant_height,
            )
            .is_none()
    );
    let error = negative
        .service
        .validate_candidate(&negative.context, &negative.body)
        .expect_err("one-byte evidence budget must reject before voting");
    match &error {
        crate::sumeragi::v2_apply::V2ApplyError::Validation(message) => {
            assert!(
                message.contains("configured shared stable aggregate byte bound")
                    && message.contains("of 1 bytes"),
                "unexpected grouped Native byte-budget rejection: {message}"
            );
        }
        other => panic!("unexpected grouped Native pre-vote error: {other}"),
    }
    assert!(!error.requires_restart_recovery());
    assert_eq!(negative.state.committed_height(), 3);
    assert_eq!(
        negative
            .kura
            .exact_durable_blocks_count()
            .expect("read exact post-rejection durable height"),
        3
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(negative.state.as_ref()),
        state_hash_before,
        "undersized pre-vote rejection must discard its WSV overlay"
    );
    assert!(negative.kura.get_block_hash(candidate_height).is_none());
    assert!(
        negative
            .kura
            .wsv_checkpoint(negative.context.height)
            .expect("read post-rejection WSV checkpoint")
            .is_none()
    );
    assert!(
        negative
            .kura
            .commit_manifest(negative.context.height)
            .expect("read post-rejection commit manifest")
            .is_none()
    );
    assert!(
        negative
            .kura
            .v2_finality_artifact(negative.context.height)
            .expect("read post-rejection finality")
            .is_none()
    );
    assert!(
        negative
            .kura
            .read_native_amx_participant_application_receipt(
                negative.participant_lane,
                negative.participant_dataspace,
                negative.participant_incarnation,
                negative.participant_height,
            )
            .is_none()
    );
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
        adapter.context.network_id,
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
    let mut finality = verified_finality_artifact_for_block_with_execution_commitment(
        adapter, keys, carrier, commitment,
    );
    let local_signer = adapter.context.leader(0);
    finality
        .commit_qc
        .signers
        .retain(|signer| *signer != local_signer);
    assert_eq!(
        u32::try_from(finality.commit_qc.signers.len()).expect("signer count fits u32"),
        finality.height_context.quorum.min_signers,
        "the non-local validators form the exact commit quorum"
    );
    let first_signer = *finality
        .commit_qc
        .signers
        .first()
        .expect("non-local finality quorum has one signer");
    let preimage = finality
        .commit_qc
        .signer_preimage(&adapter.context, first_signer)
        .expect("derive non-local finality signer preimage");
    let signatures = finality
        .commit_qc
        .signers
        .iter()
        .map(|signer| {
            Signature::try_new(
                keys[usize::try_from(*signer).expect("signer index fits usize")].private_key(),
                &preimage,
            )
            .expect("sign non-local finality vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    finality.commit_qc.aggregate_signature =
        iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate non-local finality votes");
    finality
        .verify()
        .expect("cryptographically valid non-local finality quorum");
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
        .expect("inspect durable carrier payload")
        .expect("durable carrier exists");
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
    assert_eq!(
        kura.repair_native_amx_participant_application_evidence_for_markers(
            &fixture.carrier,
            &[fixture.marker.clone()],
        )
        .expect("retry exact Native startup evidence repair"),
        1,
        "an exact post-recovery retry must remain idempotent"
    );
    let mut drifted_marker = fixture.marker.clone();
    drifted_marker.participant_proposal_hash =
        Hash::new(b"drifted Native body-recovery participant proposal");
    let error = kura
        .preflight_native_amx_participant_application_evidence_repair(
            &fixture.carrier,
            &[drifted_marker],
            None,
        )
        .expect_err("a drifted State marker must not select carrier evidence");
    assert!(
        error
            .to_string()
            .contains("absent from its authenticated carrier manifest"),
        "unexpected drifted Native marker error: {error}"
    );
    assert_eq!(
        kura.read_native_amx_participant_application_receipt(
            fixture.marker.lane_id,
            fixture.marker.dataspace_id,
            fixture.marker.lane_incarnation,
            fixture.marker.lane_block_height,
        ),
        Some(receipt),
        "failed marker preflight must leave the repaired receipt unchanged"
    );
}
struct MergeNativeProjectionFixture {
    block: SignedBlock,
    entry: iroha_data_model::merge::MergeLedgerEntry,
    source_ids: Vec<[u8; Hash::LENGTH]>,
    routes: Vec<(LaneId, DataSpaceId)>,
}
fn merge_native_projection_lane_qc(
    proposal: &LaneBlockProposalV1,
    phase: CertPhase,
) -> LaneBlockQcV1 {
    let descriptor = &proposal.descriptor;
    LaneBlockQcV1 {
        body: proposal.vote_body(phase),
        validator_set_hash_version: descriptor.validator_set_hash_version,
        validator_set_hash: descriptor.validator_set_hash,
        validator_set: descriptor.validator_set.clone(),
        signers_bitmap: vec![1],
        bls_aggregate_signature: vec![0xA5; 96],
        payload_availability_qc: None,
    }
}
fn merge_native_projection_execution(
    entrypoints: Vec<TransactionEntrypoint>,
    results: Vec<iroha_data_model::transaction::signed::TransactionResult>,
    receipts: Vec<NativeAmxReceipt>,
) -> iroha_data_model::merge::MergeLaneExecution {
    let coordinator_proposal = receipts[0]
        .legs
        .iter()
        .find_map(|leg| {
            matches!(
                crate::native_amx::native_amx_participant_application_role(&receipts[0], leg),
                Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator)
            )
            .then(|| leg.participant_proposal.clone())
        })
        .unwrap_or_else(|| {
            receipts[0]
                .legs
                .last()
                .expect("merge projection fixture coordinator-shaped leg")
                .participant_proposal
                .clone()
        });
    let source_bundle = b"Native AMX merge projection source".to_vec();
    let mut settlement = receipts[0]
        .legs
        .last()
        .expect("merge projection fixture coordinator settlement")
        .participant_settlement
        .clone();
    settlement.native_amx_receipts = receipts.clone();
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
        .expect("hash merge projection fixture settlement");
    iroha_data_model::merge::MergeLaneExecution {
        source_bundle_hash: Hash::new(&source_bundle),
        source_bundle,
        proposal: coordinator_proposal.clone(),
        origin_proposal: coordinator_proposal.clone(),
        prepare_qc: merge_native_projection_lane_qc(&coordinator_proposal, CertPhase::Prepare),
        commit_qc: merge_native_projection_lane_qc(&coordinator_proposal, CertPhase::Commit),
        signer_proofs: Vec::new(),
        autonomous_network_id: receipts[0].network_id,
        autonomous_epoch: 3,
        autonomous_payload_hash: Hash::new(b"Native AMX merge projection payload"),
        entrypoint_hashes: entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect(),
        authenticated_signed_replay_aliases: vec![None; entrypoints.len()],
        entrypoints,
        reservation_keys: vec![Vec::new(); receipts.len()],
        routing_plans: vec![Vec::new(); receipts.len()],
        native_amx_receipts: receipts.into_iter().map(Some).collect(),
        result_hashes: results
            .iter()
            .map(|result| Hash::from(result.hash()))
            .collect(),
        results,
        settlement_commitment: settlement,
        settlement_hash,
        fastpq_transcripts: Vec::new().into(),
    }
}
fn merge_native_projection_batch(
    execution: iroha_data_model::merge::MergeLaneExecution,
    application_block_header: &BlockHeader,
    parent_hash: HashOf<BlockHeader>,
) -> iroha_data_model::merge::MergeExecutionBatch {
    let lanes = vec![execution];
    let base_state_height = application_block_header.height().get() - 1;
    let base_state_hash = parent_hash;
    let execution_root = crate::merge::merge_execution_root(&lanes);
    let entrypoint_merkle_root = crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
        .expect("merge projection fixture entrypoint root");
    let result_merkle_root = crate::merge::merge_execution_result_merkle_root(&lanes)
        .expect("merge projection fixture result root");
    let write_set_root = Hash::new(b"Native AMX merge projection write set");
    let mut batch = iroha_data_model::merge::MergeExecutionBatch {
        version: 1,
        base_state_height,
        base_state_hash,
        application_block_header: application_block_header.clone(),
        entrypoint_count: u64::try_from(lanes[0].entrypoints.len())
            .expect("merge projection fixture entrypoint count fits u64"),
        lanes,
        entrypoint_merkle_root,
        result_merkle_root,
        execution_root,
        application_write_set_root: Hash::new(b"Native AMX merge projection application write set"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            base_state_height,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    batch
}
fn merge_native_projection_entry_and_carrier(
    batch: iroha_data_model::merge::MergeExecutionBatch,
    application_block_header: BlockHeader,
    parent_hash: HashOf<BlockHeader>,
) -> (SignedBlock, iroha_data_model::merge::MergeLedgerEntry) {
    let application_height = application_block_header.height().get();
    let carrier_key = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::BlsNormal)
        .expect("merge projection carrier key");
    let validator_set = vec![PeerId::new(carrier_key.public_key().clone())];
    let entry = iroha_data_model::merge::MergeLedgerEntry {
        version: iroha_data_model::merge::MergeLedgerEntry::VERSION,
        epoch_id: 3,
        lane_catalog_hash: Hash::new(b"Native AMX merge projection lane catalog"),
        active_lanes: Vec::new(),
        lane_authority_catalog: iroha_data_model::merge::MergeLaneAuthorityCatalogV1::default(),
        incarnation_root: Hash::new(b"Native AMX merge projection incarnations"),
        activation_root: Hash::new(b"Native AMX merge projection activations"),
        lane_snapshots: Vec::new(),
        global_state_root: Hash::new(b"Native AMX merge projection global state"),
        merge_qc: iroha_data_model::merge::MergeQuorumCertificate::new(
            application_block_header.view_change_index(),
            3,
            application_height,
            parent_hash,
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"Native AMX merge projection chain"),
            )),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&validator_set),
            validator_set,
            vec![1],
            Vec::new(),
            vec![0xA5; 96],
            Hash::new(b"Native AMX merge projection QC"),
        ),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
    };
    let signature =
        SignatureOf::try_from_hash(carrier_key.private_key(), application_block_header.hash())
            .expect("sign merge projection carrier");
    let mut block = SignedBlock::presigned(
        BlockSignature::new(0, signature),
        application_block_header,
        Vec::new(),
    );
    block.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_merge_entry(
            iroha_data_model::block::CertifiedMergeLedgerReference::new(&entry),
        ),
    ));
    (block, entry)
}
fn merge_native_projection_fixture(
    mutate_receipts: impl FnOnce(&mut [NativeAmxReceipt]),
) -> MergeNativeProjectionFixture {
    let ordinary_block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let ordinary_manifest =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &ordinary_block,
        )
        .expect("ordinary Native projection fixture manifest");
    let routes = ordinary_manifest
        .entries()
        .iter()
        .map(|entry| (entry.leaf.lane_id, entry.leaf.dataspace_id))
        .collect::<Vec<_>>();
    let entrypoints = ordinary_block
        .external_entrypoints_cloned()
        .collect::<Vec<_>>();
    let results = ordinary_block.results().cloned().collect::<Vec<_>>();
    let mut receipts = ordinary_block
        .execution_context()
        .expect("ordinary Native projection execution context")
        .external
        .iter()
        .map(|context| {
            context
                .native_amx_receipt
                .clone()
                .expect("ordinary Native projection receipt")
        })
        .collect::<Vec<_>>();
    let source_ids = receipts
        .iter()
        .map(|receipt| receipt.source_id)
        .collect::<Vec<_>>();
    mutate_receipts(&mut receipts);
    let execution = merge_native_projection_execution(entrypoints, results, receipts);
    let application_height = ordinary_block.header().height().get();
    let parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"Native AMX merge projection carrier parent"));
    let application_block_header = BlockHeader::new(
        NonZeroU64::new(application_height).expect("non-zero projection fixture height"),
        Some(parent_hash),
        None,
        None,
        application_height,
        ordinary_block.header().view_change_index(),
    );
    let batch = merge_native_projection_batch(execution, &application_block_header, parent_hash);
    let (block, entry) =
        merge_native_projection_entry_and_carrier(batch, application_block_header, parent_hash);
    MergeNativeProjectionFixture {
        block,
        entry,
        source_ids,
        routes,
    }
}
fn merge_native_projection_rebind_single_source_participant(
    leg: &mut NativeAmxLegRecordV2,
    entrypoint_index: u64,
    source_id: [u8; Hash::LENGTH],
    participant_height: u64,
    predecessor_descriptor_hash: Option<Hash>,
) {
    let entrypoint_hash = leg.prepare_qc.body.tx_entrypoint_hash;
    let predecessor_height = participant_height
        .checked_sub(1)
        .expect("merge projection participant height is non-zero");
    let descriptor = &mut leg.participant_proposal.descriptor;
    descriptor.previous_lane_block_height = predecessor_height;
    descriptor.previous_lane_block_descriptor_hash = predecessor_descriptor_hash;
    descriptor.lane_block_height = participant_height;
    descriptor.accepted_candidate_indices = vec![entrypoint_index];
    descriptor.accepted_transaction_hashes = vec![Hash::from(entrypoint_hash)];
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    leg.participant_settlement.block_height = participant_height;
    leg.participant_settlement
        .receipts
        .retain(|receipt| receipt.source_id == source_id);
    assert_eq!(leg.participant_settlement.receipts.len(), 1);
    leg.participant_settlement.tx_count = 1;
    leg.participant_settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&leg.participant_settlement)
            .expect("hash single-source merge projection settlement");
    let descriptor = &leg.participant_proposal.descriptor;
    let participant_lane_id = descriptor.lane_id;
    let participant_dataspace_id = descriptor.dataspace_id;
    let participant_incarnation = descriptor.lane_incarnation;
    let participant_view = descriptor.lane_block_view;
    let proposal_hash = leg.participant_proposal.proposal_hash;
    let settlement_commitment = Hash::from(leg.participant_settlement_hash);
    for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
        body.source_id = source_id;
        body.tx_entrypoint_hash = entrypoint_hash;
        body.participant_lane_id = participant_lane_id;
        body.participant_dataspace_id = participant_dataspace_id;
        body.participant_lane_incarnation = participant_incarnation;
        body.participant_previous_block_height = predecessor_height;
        body.participant_previous_block_descriptor_hash = predecessor_descriptor_hash;
        body.participant_lane_block_height = participant_height;
        body.participant_lane_block_view = participant_view;
        body.participant_proposal_hash = proposal_hash;
        body.participant_settlement_commitment = settlement_commitment;
    }
}
fn merge_native_projection_split_participant_heights(
    receipts: &mut [NativeAmxReceipt],
    second_height_delta: u64,
) {
    assert_eq!(receipts.len(), 2);
    let coordinator_route = (receipts[0].lane_id, receipts[0].dataspace_id);
    let participant = receipts[0]
        .legs
        .iter()
        .find(|leg| (leg.lane_id, leg.dataspace_id) != coordinator_route)
        .expect("merge projection separate participant leg");
    let route = (participant.lane_id, participant.dataspace_id);
    let first_height = participant
        .participant_proposal
        .descriptor
        .lane_block_height;
    let first_predecessor_hash = participant
        .participant_proposal
        .descriptor
        .previous_lane_block_descriptor_hash;
    let second_height = first_height
        .checked_add(second_height_delta)
        .expect("participant height fits u64");
    for receipt in receipts.iter_mut() {
        let coordinator_route = (receipt.lane_id, receipt.dataspace_id);
        receipt.legs.retain(|leg| {
            let leg_route = (leg.lane_id, leg.dataspace_id);
            leg_route == route || leg_route == coordinator_route
        });
    }
    let (first, second) = receipts.split_at_mut(1);
    let first_receipt = &mut first[0];
    let first_source_id = first_receipt.source_id;
    let first_leg = first_receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == route)
        .expect("first-height participant leg");
    merge_native_projection_rebind_single_source_participant(
        first_leg,
        0,
        first_source_id,
        first_height,
        first_predecessor_hash,
    );
    let first_descriptor_hash = first_leg.participant_proposal.descriptor.descriptor_hash;
    let second_receipt = &mut second[0];
    let second_source_id = second_receipt.source_id;
    let second_leg = second_receipt
        .legs
        .iter_mut()
        .find(|leg| (leg.lane_id, leg.dataspace_id) == route)
        .expect("second-height participant leg");
    merge_native_projection_rebind_single_source_participant(
        second_leg,
        1,
        second_source_id,
        second_height,
        Some(first_descriptor_hash),
    );
}
#[test]
fn native_amx_manifest_projects_finality_bound_merge_batch_in_canonical_order() {
    let fixture = merge_native_projection_fixture(|_| {});
    assert!(
        fixture
            .block
            .execution_context()
            .expect("merge carrier execution context")
            .external
            .is_empty(),
        "the merge carrier must not duplicate certified external contexts"
    );
    assert_eq!(
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &fixture.block,
        )
        .expect("ordinary-only projection of autonomous carrier")
        .count(),
        0,
        "autonomous receipts must come only from the exact certified merge entry"
    );
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("project finality-bound merge receipts");
    assert_eq!(manifest.count(), 2);
    assert_eq!(
        manifest
            .entries()
            .iter()
            .map(|entry| (entry.leaf.lane_id, entry.leaf.dataspace_id))
            .collect::<Vec<_>>(),
        fixture.routes
    );
    for entry in manifest.entries() {
        assert_eq!(
            entry
                .leaf
                .members
                .iter()
                .map(|member| (member.entrypoint_index, member.source_id))
                .collect::<Vec<_>>(),
            vec![(0, fixture.source_ids[0]), (1, fixture.source_ids[1])],
            "lane/entrypoint order must be retained while identical routes are grouped"
        );
    }
    let markers = crate::state::State::native_amx_participant_frontier_markers_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("derive State frontiers from the canonical merge manifest");
    assert_eq!(markers.len(), manifest.entries().len());
    for (marker, entry) in markers.iter().zip(manifest.entries()) {
        let leaf = &entry.leaf;
        assert_eq!(marker.lane_id, leaf.lane_id);
        assert_eq!(marker.dataspace_id, leaf.dataspace_id);
        assert_eq!(marker.lane_incarnation, leaf.lane_incarnation);
        assert_eq!(marker.lane_block_height, leaf.participant_height);
        assert_eq!(marker.participant_proposal_hash, leaf.proposal_hash);
        assert_eq!(marker.participant_settlement_hash, leaf.settlement_hash);
        assert_eq!(
            marker.source_count,
            u64::try_from(leaf.members.len()).expect("fixture member count fits u64")
        );
    }
}
#[test]
fn native_amx_merge_projection_rejects_multiple_participant_heights_in_one_carrier() {
    for second_height_delta in [1_u64, 2_u64] {
        let fixture = merge_native_projection_fixture(|receipts| {
            merge_native_projection_split_participant_heights(receipts, second_height_delta);
        });
        let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
            &fixture.block,
            Some(&fixture.entry),
        )
        .expect_err("one carrier must not publish two heights for one participant route");
        assert_eq!(
            error,
            "Native AMX participant route carries more than one height in one application block"
        );
    }
}
#[test]
fn native_amx_merge_projection_rejects_same_height_participant_identity_conflict() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let coordinator_lane_id = receipts[0].lane_id;
        let coordinator_dataspace_id = receipts[0].dataspace_id;
        let participant = receipts[0]
            .legs
            .iter()
            .find(|leg| {
                leg.lane_id != coordinator_lane_id || leg.dataspace_id != coordinator_dataspace_id
            })
            .expect("merge projection separate participant leg");
        let participant_lane_id = participant.lane_id;
        let participant_dataspace_id = participant.dataspace_id;
        let participant_incarnation = participant.participant_proposal.descriptor.lane_incarnation;
        let participant_height = participant
            .participant_proposal
            .descriptor
            .lane_block_height;
        {
            let leg = receipts[1]
                .legs
                .iter_mut()
                .find(|leg| {
                    leg.lane_id == participant_lane_id
                        && leg.dataspace_id == participant_dataspace_id
                })
                .expect("conflicting same-height participant leg");
            assert_eq!(
                leg.participant_proposal.descriptor.lane_incarnation,
                participant_incarnation
            );
            assert_eq!(
                leg.participant_proposal.descriptor.lane_block_height,
                participant_height
            );
            let descriptor = &mut leg.participant_proposal.descriptor;
            descriptor.lane_block_view = descriptor
                .lane_block_view
                .checked_add(1)
                .expect("participant view fits u64");
            descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
            leg.participant_proposal.proposal_hash =
                leg.participant_proposal.computed_proposal_hash();
            let participant_view = leg.participant_proposal.descriptor.lane_block_view;
            let proposal_hash = leg.participant_proposal.proposal_hash;
            for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
                body.participant_lane_block_view = participant_view;
                body.participant_proposal_hash = proposal_hash;
            }
        }
        let leg = receipts[1]
            .legs
            .iter()
            .find(|leg| {
                leg.lane_id == participant_lane_id && leg.dataspace_id == participant_dataspace_id
            })
            .expect("drifted separate participant leg");
        assert_eq!(
            crate::native_amx::native_amx_participant_application_role(&receipts[1], leg),
            Ok(crate::native_amx::NativeAmxParticipantApplicationRole::SeparateParticipant)
        );
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("same-height participant identity drift must fail closed");
    assert!(
        error.contains("participant route carries conflicting proposal/control claims"),
        "{error}"
    );
}
#[test]
fn native_amx_merge_projection_excludes_coordinator_only_receipts() {
    let fixture = merge_native_projection_fixture(|receipts| {
        for receipt in receipts {
            let lane_id = receipt.lane_id;
            let dataspace_id = receipt.dataspace_id;
            receipt
                .legs
                .retain(|leg| leg.lane_id == lane_id && leg.dataspace_id == dataspace_id);
        }
    });
    let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("coordinator-only merge projection");
    assert_eq!(manifest.count(), 0);
    assert!(
        crate::state::State::native_amx_participant_frontier_markers_and_merge_entry(
            &fixture.block,
            Some(&fixture.entry),
        )
        .expect("coordinator-only State projection")
        .is_empty()
    );
}
#[test]
fn native_amx_merge_projection_rejects_same_route_identity_conflict() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let receipt = &mut receipts[0];
        let lane_id = receipt.lane_id;
        let dataspace_id = receipt.dataspace_id;
        let leg = receipt
            .legs
            .iter_mut()
            .find(|leg| leg.lane_id == lane_id && leg.dataspace_id == dataspace_id)
            .expect("merge projection coordinator leg");
        leg.participant_proposal.descriptor.lane_incarnation =
            Hash::new(b"conflicting merge coordinator incarnation");
        leg.participant_proposal.descriptor.descriptor_hash = leg
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
        leg.participant_settlement.lane_incarnation =
            leg.participant_proposal.descriptor.lane_incarnation;
        leg.participant_settlement_hash =
            iroha_data_model::nexus::compute_settlement_hash(&leg.participant_settlement)
                .expect("hash conflicting merge coordinator settlement");
        for body in [&mut leg.prepare_qc.body, &mut leg.commit_qc.body] {
            body.participant_lane_incarnation =
                leg.participant_proposal.descriptor.lane_incarnation;
            body.participant_proposal_hash = leg.participant_proposal.proposal_hash;
            body.participant_settlement_commitment = Hash::from(leg.participant_settlement_hash);
        }
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("same-route identity drift must fail closed");
    assert!(
        error.contains("same-route leg differs from the coordinator identity"),
        "{error}"
    );
}
#[test]
fn native_amx_merge_projection_rejects_duplicate_group_source() {
    let fixture = merge_native_projection_fixture(|receipts| {
        let duplicate_source_id = receipts[0].source_id;
        receipts[1].source_id = duplicate_source_id;
        for leg in &mut receipts[1].legs {
            leg.prepare_qc.body.source_id = duplicate_source_id;
            leg.commit_qc.body.source_id = duplicate_source_id;
        }
    });
    let error = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect_err("duplicate participant source must fail closed");
    assert!(error.contains("repeats a source transaction"), "{error}");
}
#[test]
fn native_amx_merge_projection_matches_decoded_replay_entry() {
    let fixture = merge_native_projection_fixture(|_| {});
    let encoded = norito::to_bytes(&fixture.entry).expect("encode durable merge entry");
    let recovered =
        norito::decode_from_bytes::<iroha_data_model::merge::MergeLedgerEntry>(&encoded)
            .expect("decode durable merge entry");
    let live = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&fixture.entry),
    )
    .expect("live merge projection");
    let restarted = crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &fixture.block,
        Some(&recovered),
    )
    .expect("recovered merge projection");
    let ordinary_only =
        crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block(
            &fixture.block,
        )
        .expect("ordinary-only replay projection");
    let witness = iroha_data_model::block::consensus::ExecWitness {
        reads: Vec::new(),
        writes: Vec::new(),
        fastpq_transcripts: Vec::new(),
        fastpq_batches: Vec::new(),
    };
    let lane_finality_manifest =
        crate::sumeragi::exec::LaneFinalityManifestV1::from_result_bearing_block(&fixture.block)
            .expect("merge replay lane-finality manifest");
    let live_commitment = crate::sumeragi::exec::execution_commitment_from_validated_block(
        &witness,
        &live,
        &lane_finality_manifest,
        &fixture.block,
    )
    .expect("live merge replay commitment");
    let restarted_commitment = crate::sumeragi::exec::execution_commitment_from_validated_block(
        &witness,
        &restarted,
        &lane_finality_manifest,
        &fixture.block,
    )
    .expect("decoded merge replay commitment");
    let ordinary_only_commitment =
        crate::sumeragi::exec::execution_commitment_from_validated_block(
            &witness,
            &ordinary_only,
            &lane_finality_manifest,
            &fixture.block,
        )
        .expect("ordinary-only merge replay commitment");
    assert_eq!(ordinary_only.count(), 0);
    assert_ne!(restarted.root(), ordinary_only.root());
    assert_eq!(restarted_commitment, live_commitment);
    assert_ne!(ordinary_only_commitment, live_commitment);
    assert_eq!(restarted.root(), live.root());
    assert_eq!(restarted.count(), live.count());
    assert_eq!(
        restarted
            .entries()
            .iter()
            .map(|entry| entry.leaf.clone())
            .collect::<Vec<_>>(),
        live.entries()
            .iter()
            .map(|entry| entry.leaf.clone())
            .collect::<Vec<_>>()
    );
}
