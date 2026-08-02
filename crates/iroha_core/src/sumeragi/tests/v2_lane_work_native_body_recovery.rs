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
    native_body_recovery_adapter_with_kura(Kura::blank_kura_for_testing_with_blocks_in_memory(
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
    let mut kura = Kura::blank_kura_for_testing();
    if let Some(aggregate_bytes) = pending_control_validation_bytes {
        Arc::get_mut(&mut kura)
            .expect("fresh grouped Native fixture Kura has one owner")
            .set_pending_control_sidecar_validation_bytes_for_testing(aggregate_bytes);
    }
    let (adapter, keys, participant_lane, participant_dataspace) =
        native_body_recovery_adapter_with_kura(kura);

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
            adapter.context.chain_id.clone(),
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
        context.chain_id.clone(),
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
