use iroha_data_model::transaction::TransactionAdmissionIntent;

const MERGE_QUEUE_PLAN_SYNCED_INTENT_ERROR: &str =
    "autonomous merge entrypoint does not carry QueuePlanSynced admission intent";

fn ordinary_external_entrypoint_for_merge_intent_test(
    state: &State,
    tag: u8,
) -> TransactionEntrypoint {
    let transaction_keypair =
        KeyPair::try_from_seed(vec![tag.wrapping_add(0x31); 32], Algorithm::Ed25519)
            .expect("deterministic Ordinary merge transaction key");
    let authority = AccountId::new(transaction_keypair.public_key().clone());
    let mut transaction = TransactionBuilder::new(
        *state.network_id_ref(),
        authority,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(Duration::from_millis(u64::from(tag).saturating_add(1)));
    TransactionEntrypoint::External(
        transaction
            .with_instructions([Log::new(
                Level::INFO,
                format!("ordinary-merge-admission-{tag}"),
            )])
            .with_admission_intent(TransactionAdmissionIntent::Ordinary)
            .sign(transaction_keypair.private_key()),
    )
}

fn assert_merge_queue_plan_synced_intent_error(error: MergeLedgerCommitError) {
    match error {
        MergeLedgerCommitError::ExecutionBatchInvalid(reason) => {
            assert_eq!(reason, MERGE_QUEUE_PLAN_SYNCED_INTENT_ERROR);
        }
        other => panic!("Ordinary merge entrypoint returned an unexpected error: {other:?}"),
    }
}

#[test]
fn autonomous_merge_admission_intent_producer_rejects_ordinary_external_before_effects() {
    let (state, validator_keypairs, _, parent) = configured_single_lane_queue_plan_state();
    let authority_height = parent.header().height().get();
    let application_height = authority_height
        .checked_add(1)
        .expect("fixture application height");
    let tag = 0x79;
    let entrypoint = ordinary_external_entrypoint_for_merge_intent_test(&state, tag);
    assert_eq!(
        entrypoint.admission_intent(),
        TransactionAdmissionIntent::Ordinary
    );
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_entrypoint_state_test(
        &state,
        routing_plan.clone(),
        &validator_keypairs,
        authority_height,
        tag,
        &entrypoint,
    );
    {
        let mut world = state.world.block();
        world.accounts.insert(
            entrypoint.authority().clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    seed_exact_queue_plan_admission_state_for_test(&state, &certificate);
    let source = autonomous_merge_source_for_queue_plan_admission_test(
        &state,
        &binding,
        entrypoint,
        routing_plan,
        &validator_keypairs,
    );
    let application_header = BlockHeader::new(
        NonZeroU64::new(application_height).expect("fixture application height is non-zero"),
        Some(parent.hash()),
        None,
        None,
        u64::try_from(parent.header().creation_time().as_millis())
            .expect("fixture parent time fits u64")
            .saturating_add(1),
        0,
    );
    let mut state_block = state.lane_application_block(application_header);
    let error = State::preexecute_merge_execution_sources_into(&mut state_block, vec![source])
        .expect_err("producer must reject Ordinary External autonomous content");
    assert_merge_queue_plan_synced_intent_error(error);
    assert!(state_block.direct_committed_entrypoints.is_empty());
    assert!(state_block.world.external_event_buf.is_empty());
}

#[test]
fn autonomous_merge_admission_intent_follower_and_historical_reject_ordinary_external() {
    let (state, entry, _, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let mut batch = entry
        .execution_batch
        .as_ref()
        .expect("fixture carries QueuePlanSynced execution")
        .clone();
    assert!(batch.lanes.iter().all(|execution| {
        execution.entrypoints.iter().all(|entrypoint| {
            entrypoint.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced
        })
    }));
    for validate_live_authority in [true, false] {
        state
            .validate_merge_execution_batch(
                &entry.active_lanes,
                &batch,
                &BTreeMap::new(),
                validate_live_authority,
                Some(ConsensusMode::Permissioned),
            )
            .expect("QueuePlanSynced merge content remains valid");
    }
    let ordinary = ordinary_external_entrypoint_for_merge_intent_test(&state, 0x7A);
    let ordinary_hash = Hash::from(ordinary.hash());
    batch.lanes[0].entrypoints[0] = ordinary;
    batch.lanes[0].entrypoint_hashes[0] = ordinary_hash;
    batch.entrypoint_merkle_root =
        crate::merge::merge_execution_entrypoint_merkle_root(&batch.lanes)
            .expect("mutated batch retains an entrypoint root");
    batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    assert!(crate::merge::merge_execution_batch_commitments_match(
        &batch
    ));
    for validate_live_authority in [true, false] {
        let error = state
            .validate_merge_execution_batch(
                &entry.active_lanes,
                &batch,
                &BTreeMap::new(),
                validate_live_authority,
                Some(ConsensusMode::Permissioned),
            )
            .expect_err("follower and historical validation must reject Ordinary content");
        assert_merge_queue_plan_synced_intent_error(error);
    }
}
