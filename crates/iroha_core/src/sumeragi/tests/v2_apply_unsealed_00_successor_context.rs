// Shared canonical successor and FIFO reservation support for apply tests.

/// Build a source carrier against the verified current canonical parent.
fn build_apply_fixture_at_context_with_autonomous_payloads(
    fixture: &ApplyFixture,
    context: wire::HeightContext,
    autonomous_lane_payloads: Vec<iroha_data_model::block::AutonomousLanePayloadEnvelopeV1>,
) -> SuccessorApplyFixture {
    assert_eq!(
        verified_successor_context_at_fixture_tip(fixture).context(),
        &context,
        "source carrier uses the verified live successor context"
    );
    build_apply_fixture_from_current_parent(fixture, context, autonomous_lane_payloads)
}
/// Share canonical parent binding between first-successor and later-cycle fixtures.
fn build_apply_fixture_from_current_parent(
    fixture: &ApplyFixture,
    context: wire::HeightContext,
    autonomous_lane_payloads: Vec<iroha_data_model::block::AutonomousLanePayloadEnvelopeV1>,
) -> SuccessorApplyFixture {
    let parent_height = fixture.state.committed_height();
    assert_eq!(
        context.height,
        u64::try_from(parent_height)
            .expect("fixture parent height fits u64")
            .checked_add(1)
            .expect("fixture successor height does not overflow"),
        "source carrier follows the exact current State tip"
    );
    let parent = fixture
        .kura
        .get_block(NonZeroUsize::new(parent_height).expect("source carrier has a committed parent"))
        .expect("source carrier retains the exact canonical parent");
    assert_eq!(fixture.state.latest_block_hash_fast(), Some(parent.hash()));
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let transaction = TransactionBuilder::new(
        context.network_id,
        fixture.service.genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "reputation retained-capture successor".to_owned(),
    )])
    .sign(fixture.genesis_key.private_key());
    let leader_index = context.leader(round.view);
    let carries_only_autonomous_payloads = !autonomous_lane_payloads.is_empty();
    let execution_context = if carries_only_autonomous_payloads {
        BlockExecutionContextBundle::new(Vec::new())
            .with_autonomous_lane_payloads(autonomous_lane_payloads)
    } else {
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = fixture
            .service
            .queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve successor transaction route");
        let route = routing_plan.coordinator_route();
        let entrypoint_hash = Hash::from(accepted.hash_as_entrypoint());
        let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
            fixture.state.as_ref(),
            fixture.kura.as_ref(),
            &context,
            round.view,
            &context.roster[usize::try_from(leader_index).expect("successor leader index")]
                .validator,
            std::slice::from_ref(&route),
            std::slice::from_ref(&entrypoint_hash),
        )
        .expect("derive canonical successor lane plan");
        assert!(
            lane_plan.unavailable_indices.is_empty(),
            "successor fixture lane must be available"
        );
        BlockExecutionContextBundle::new(vec![execution_context_for_routing_plan(
            transaction.hash_as_entrypoint(),
            &routing_plan,
        )])
        .with_lane_payload_ownerships(lane_plan.ownerships)
    };
    let mut logical_time = parent
        .header()
        .creation_time()
        .checked_add(fixture.service.block_cadence)
        .expect("successor logical time fits Duration");
    if !carries_only_autonomous_payloads {
        logical_time = logical_time.max(
            transaction
                .creation_time()
                .checked_add(Duration::from_millis(1))
                .expect("successor transaction floor fits Duration"),
        );
    }
    let creation_time_ms = logical_time
        .as_millis()
        .try_into()
        .expect("successor creation time fits u64");
    let mut header = BlockHeader::new(
        NonZeroU64::new(context.height).expect("non-zero successor height"),
        Some(parent.hash()),
        None,
        None,
        creation_time_ms,
        0,
    );
    let confidential_features = {
        let state_view = fixture.state.view();
        let digest = crate::state::compute_confidential_feature_digest(
            state_view.world(),
            &state_view.zk,
            state_view.sccp_registry.as_ref(),
            context.height,
        );
        (!digest.is_empty()).then_some(digest)
    };
    header.set_confidential_features(confidential_features);
    let proof_policy_bundle = crate::da::active_proof_policy_bundle_at_height(
        &fixture.state.nexus_snapshot(),
        context.height,
    );
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic successor BLS key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let leader = usize::try_from(leader_index).expect("successor leader index");
    assert_eq!(
        keys[leader].public_key(),
        context.roster[leader].validator.public_key(),
        "successor signer must be the rotating leader"
    );
    let mut builder = iroha_data_model::block::builder::BlockBuilder::new(header);
    if !carries_only_autonomous_payloads {
        builder.push_transaction(transaction);
    }
    builder.set_da_proof_policies(Some(proof_policy_bundle));
    builder.set_execution_context(Some(execution_context));
    let body = builder
        .try_build_with_signature(u64::from(leader_index), keys[leader].private_key())
        .expect("sign successor proposal")
        .canonical_resultless_proposal();
    let canonical_wire = body.encode_wire().expect("encode successor body");
    let subject = wire::BlockSubject {
        parent_block_hash: Some(parent.hash()),
        block_hash: body.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let manifest =
        crate::sumeragi::v2_chunks::encode_payload(&context, round, subject, &canonical_wire)
            .expect("derive successor payload manifest")
            .into_parts()
            .0;
    let execution_commitment = fixture
        .service
        .validate_candidate(&context, &body)
        .expect("derive successor execution commitment");
    let mut certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signatures = certificate
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keys[usize::try_from(*index).expect("successor signer index")].private_key(),
                &preimage,
            )
            .expect("sign successor Commit vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    certificate.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .expect("aggregate successor Commit votes");
    let body_root = tempfile::tempdir().expect("successor body-store directory");
    let mut store = V2BodyStore::open(body_root.path(), context.clone())
        .expect("open successor rotating-leader body store");
    let durable = store
        .store(manifest, canonical_wire)
        .expect("persist exact successor body");
    let validated = store
        .validate(&durable, |candidate| {
            fixture.service.validate_candidate(&context, candidate)
        })
        .expect("persist successor validation marker");
    let task = ApplyTask::for_test(
        context.height,
        EventTag::new(context.height, 0, Generation::new(context.height)),
        subject,
        certificate,
        validated,
    );
    SuccessorApplyFixture {
        context,
        body,
        task,
        _body_root: body_root,
        store,
    }
}

/// Reauthenticate the next context from the actual committed Kura/State tip.
fn verified_successor_context_at_fixture_tip(
    fixture: &ApplyFixture,
) -> super::super::v2::VerifiedHeightContext {
    let parent_height =
        u64::try_from(fixture.state.committed_height()).expect("fixture committed height fits u64");
    assert!(parent_height > 0);
    let parent_artifact = fixture
        .kura
        .v2_finality_artifact(parent_height)
        .expect("read fixture carrier finality")
        .expect("fixture carrier has finality");
    let state_view = fixture.state.view();
    let context = crate::sumeragi::v2_context::build_successor_height_context_from_state(
        &parent_artifact,
        &state_view,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(fixture.state.as_ref()),
    )
    .expect("derive fixture context after the exact canonical carrier");
    drop(state_view);
    assert_eq!(
        context.height,
        parent_height
            .checked_add(1)
            .expect("successor height fits u64")
    );
    verified_context_for_fixture(fixture, &context)
}

/// Reserve exact FIFO inputs for a verified successor without resetting validator authority.
fn reserve_canonical_autonomous_batch_at_context_with_instructions(
    fixture: &ApplyFixture,
    queue: &Arc<Queue>,
    context: &wire::HeightContext,
    count: usize,
    instructions: impl Fn(usize) -> Vec<InstructionBox>,
    sort_by_signed_transaction_hash: bool,
    native_receipt_builder: Option<ApplyNativeReceiptBuilder>,
) -> (LaneExecutablePayloadV1, Vec<HashOf<TransactionEntrypoint>>) {
    assert_eq!(
        verified_successor_context_at_fixture_tip(fixture).context(),
        context,
        "FIFO reservation uses the exact verified successor context"
    );
    reserve_canonical_autonomous_batch_with_installed_authority(
        fixture,
        queue,
        context,
        count,
        instructions,
        sort_by_signed_transaction_hash,
        native_receipt_builder,
    )
}
/// Share the exact admission/reservation path after validator authority is installed.
fn reserve_canonical_autonomous_batch_with_installed_authority(
    fixture: &ApplyFixture,
    queue: &Arc<Queue>,
    context: &wire::HeightContext,
    count: usize,
    instructions: impl Fn(usize) -> Vec<InstructionBox>,
    sort_by_signed_transaction_hash: bool,
    native_receipt_builder: Option<ApplyNativeReceiptBuilder>,
) -> (LaneExecutablePayloadV1, Vec<HashOf<TransactionEntrypoint>>) {
    assert_eq!(
        &context.network_id,
        fixture.state.network_id_ref(),
        "strict canonical successor QueuePlan fixtures must sign and persist the State network identity"
    );
    assert!((1..=16).contains(&count));
    let mut transactions = (0..count)
        .map(|index| {
            let mut builder = TransactionBuilder::new(
                context.network_id,
                fixture.service.genesis_account.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions(instructions(index))
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            );
            let nonce = u32::try_from(index)
                .ok()
                .and_then(|value| value.checked_add(1))
                .and_then(NonZeroU32::new)
                .expect("bounded autonomous fixture index yields a unique nonce");
            builder.set_nonce(nonce);
            builder.sign(fixture.genesis_key.private_key())
        })
        .collect::<Vec<_>>();
    if sort_by_signed_transaction_hash {
        transactions.sort_by_key(|transaction| transaction.hash());
    }
    let signed_transaction_hashes = transactions
        .iter()
        .map(|transaction| transaction.hash())
        .collect::<Vec<_>>();
    assert_eq!(
        signed_transaction_hashes
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        count,
        "each autonomous fixture transaction must have one unique signed identity"
    );
    let entrypoints = transactions
        .iter()
        .cloned()
        .map(TransactionEntrypoint::External)
        .collect::<Vec<_>>();
    let expected_fifo = entrypoints
        .iter()
        .map(TransactionEntrypoint::hash)
        .collect::<Vec<_>>();
    let mut planned_routing = Vec::with_capacity(count);
    for transaction in &transactions {
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction.clone()));
        let routing_plan = queue
            .route_plan_with_state(&accepted, fixture.state.as_ref())
            .expect("resolve canonical autonomous routing plan");
        let admission_context = queue
            .plan_admission_context_with_state(fixture.state.as_ref(), &routing_plan)
            .expect("capture canonical autonomous admission context");
        let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
            fixture.state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build canonical autonomous global admission binding");
        queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                accepted,
                fixture.state.as_ref(),
                routing_plan.clone(),
                &binding,
            )
            .expect("durably enqueue canonical autonomous transaction");
        install_fixture_queue_plan_registry_value(fixture.state.as_ref(), &binding);
        planned_routing.push(routing_plan);
    }
    let coordinator_routes = planned_routing
        .iter()
        .map(crate::queue::RoutingPlan::coordinator_route)
        .collect::<Vec<_>>();
    let coordinator_route = coordinator_routes
        .first()
        .expect("canonical autonomous batch has a coordinator route");
    assert!(
        coordinator_routes
            .iter()
            .all(|route| route == coordinator_route),
        "canonical autonomous fixture must target one reservation slot"
    );
    let reservation_slot = super::super::lane_planner::plan_autonomous_lane_reservation_slot(
        fixture.state.as_ref(),
        fixture.kura.as_ref(),
        context,
        coordinator_route.lane_id,
        coordinator_route.dataspace_id,
    )
    .expect("derive deterministic canonical autonomous reservation slot");
    let producer = reservation_slot.author.clone();
    let entrypoint_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    let lane_plan = super::super::lane_planner::prepare_v2_lane_payload_plan(
        fixture.state.as_ref(),
        fixture.kura.as_ref(),
        context,
        0,
        &producer,
        &coordinator_routes,
        &entrypoint_hashes,
    )
    .expect("derive canonical successor autonomous proposal");
    assert!(lane_plan.unavailable_indices.is_empty());
    assert_eq!(lane_plan.proposals.len(), 1);
    let proposal = lane_plan.proposals[0].clone();
    let network_id = context.network_id;
    let (reservation_owner_hash, proposal_identity_hash) =
        super::super::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            context.id(),
            context.epoch,
            &proposal,
            &producer,
        )
        .expect("derive canonical successor reservation identity");
    assert_eq!(
        (reservation_owner_hash, proposal_identity_hash),
        (
            reservation_slot.reservation_owner_hash,
            reservation_slot.proposal_identity_hash,
        ),
        "proposal and pre-selection slot must bind identical queue ownership",
    );
    let scope = reservation_slot.reservation_scope();
    let reserved = queue
        .reserve_transactions_for_lane(
            fixture.state.as_ref(),
            scope,
            NonZeroUsize::new(count).expect("non-zero canonical reservation count"),
        )
        .expect("reserve canonical successor autonomous batch");
    assert_eq!(reserved.len(), count);
    assert_eq!(
        reserved
            .iter()
            .map(|reservation| reservation.key().entrypoint_hash)
            .collect::<Vec<_>>(),
        expected_fifo,
        "canonical autonomous reservation must preserve FIFO selection order"
    );
    let reservation_keys = reserved
        .iter()
        .map(|reservation| *reservation.key())
        .collect::<Vec<_>>();
    let routing_plans = reserved
        .iter()
        .map(|reservation| reservation.routing_plan().clone())
        .collect::<Vec<_>>();
    assert_eq!(routing_plans, planned_routing);
    let validator_keys = fixture_validator_keys();
    let producer_key = validator_keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("fixture contains canonical autonomous producer key");
    let native_amx_receipts = match native_receipt_builder {
        Some(builder) => builder(
            fixture,
            context,
            network_id,
            &proposal,
            &entrypoints,
            &reservation_keys,
            &routing_plans,
        ),
        None => vec![None; count],
    };
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        context.epoch,
        proposal,
        entrypoints,
        reservation_keys,
        routing_plans,
        native_amx_receipts,
        producer,
        producer_key.private_key(),
    )
    .expect("build canonical successor autonomous payload");
    (payload, expected_fifo)
}
