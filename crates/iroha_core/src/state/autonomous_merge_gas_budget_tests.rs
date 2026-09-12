// Focused autonomous source budgeting regressions. Sources carry real producer signatures,
// QueuePlan bindings, availability certificates and lane CommitQCs from the existing fixture.
fn autonomous_gas_budget_fixture() -> (State, Vec<KeyPair>, SignedBlock) {
    let kura = Kura::blank_kura_for_testing();
    let mut state = State::new_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_catalog = LaneCatalog::new(
        core::num::NonZeroU32::new(2).expect("two lanes"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: LaneId::new(1),
                alias: "gas-budget-lane".to_owned(),
                dataspace_id: DataSpaceId::UNIVERSAL,
                ..LaneConfig::default()
            },
        ],
    )
    .expect("gas-budget lane catalog");
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    state
        .set_nexus(nexus)
        .expect("install two-lane gas fixture");
    let (validator_ids, keys) = bls_accounts_in("validators", 4);
    seed_consensus_keys_with_pops(&state, &keys);
    install_lane_manifest_registry(
        &state,
        &[
            (
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                validator_ids.clone(),
            ),
            (LaneId::new(1), DataSpaceId::UNIVERSAL, validator_ids),
        ],
    );
    set_commit_topology_from_keypairs(&state, &keys);
    let parent = empty_global_block_after(None);
    kura.store_block(Arc::new(parent.clone()))
        .expect("store gas fixture genesis");
    commit_block_metadata_with_genesis_checkpoint_to_state(&state, &parent);
    let parent = advance_queue_plan_fixture_to_beacon_parent(&state, parent);
    (state, keys, parent)
}

fn autonomous_gas_budget_source(
    state: &State,
    keys: &[KeyPair],
    lane_id: LaneId,
    gas: u64,
    tag: u8,
) -> MergeExecutionSource {
    let key = KeyPair::try_from_seed(vec![tag; 32], Algorithm::Ed25519)
        .expect("gas fixture transaction signer");
    let authority = AccountId::new(key.public_key().clone());
    {
        let mut world = state.world.block();
        world.accounts.insert(
            authority.clone(),
            AccountValue::new(AccountDetails::default()),
        );
        world.commit();
    }
    let entrypoint = TransactionEntrypoint::External(
        TransactionBuilder::new(
            *state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(
                Vec::new(),
                NonZeroU64::new(gas),
            ),
        )
        .with_admission_intent(
            iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
        )
        .with_executable(iroha_data_model::transaction::Executable::ContractCall(
            iroha_data_model::transaction::executable::ContractInvocation {
                contract_address: "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
                    .parse()
                    .expect("contract address"),
                expected_code_hash: Hash::new(b"merge-gas-fixture-contract"),
                entrypoint: "configure".to_owned(),
                arguments: None,
            },
        ))
        .sign(key.private_key()),
    );
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        lane_id,
        DataSpaceId::UNIVERSAL,
    ));
    let (binding, certificate) = queue_plan_admission_certificate_for_entrypoint_state_test(
        state,
        routing_plan.clone(),
        keys,
        queue_plan_authority_height_for_state_test(state),
        tag,
        &entrypoint,
    );
    seed_exact_queue_plan_admission_state_for_test(state, &certificate);
    autonomous_merge_source_for_queue_plan_admission_test(
        state,
        &binding,
        entrypoint,
        routing_plan,
        keys,
    )
    .expect("availability-certified gas fixture source")
}

#[test]
fn autonomous_merge_gas_accounting_rejects_missing_limit_and_overflow() {
    let key =
        KeyPair::try_from_seed(vec![0xBF; 32], Algorithm::Ed25519).expect("gas accounting signer");
    let build = |gas| {
        TransactionEntrypoint::External(
            TransactionBuilder::new(
                iroha_data_model::NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                        b"merge-gas-accounting-network",
                    )),
                ),
                AccountId::new(key.public_key().clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), gas),
            )
            .with_executable(iroha_data_model::transaction::Executable::Ivm(
                iroha_data_model::transaction::IvmBytecode::from_compiled(vec![0]),
            ))
            .sign(key.private_key()),
        )
    };
    let missing = build(None);
    assert!(merge_execution_proposal_gas([&missing]).is_err());
    let maximum = build(NonZeroU64::new(u64::MAX));
    assert_eq!(merge_execution_proposal_gas([&maximum]).unwrap(), u64::MAX);
    assert!(matches!(merge_execution_proposal_gas([&maximum, &maximum]),
        Err(MergeLedgerCommitError::ExecutionBatchInvalid(reason))
        if reason == "autonomous source proposal gas overflows u64"));
}

state_test!(consensus_stack autonomous_full_gas_sources_share_one_merge_budget_before_execution
    autonomous_full_gas_sources_share_one_merge_budget_before_execution_on_consensus_stack();
);
fn autonomous_full_gas_sources_share_one_merge_budget_before_execution_on_consensus_stack() {
    let (state, keys, parent) = autonomous_gas_budget_fixture();
    let limit = gas_limit_from_parameters(state.world.view().parameters());
    let left = autonomous_gas_budget_source(&state, &keys, LaneId::SINGLE, limit, 0xC1);
    let right = autonomous_gas_budget_source(&state, &keys, LaneId::new(1), limit, 0xC2);
    assert_eq!(
        merge_execution_proposal_gas(&left.input.entrypoints).unwrap(),
        limit
    );
    assert_eq!(
        merge_execution_proposal_gas(&right.input.entrypoints).unwrap(),
        limit
    );
    let selected = select_merge_execution_source_budget(vec![right.clone(), left.clone()], limit)
        .expect("reserve only one full-cap source");
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].bundle_hash, left.bundle_hash);
    let remaining = select_merge_execution_source_budget(vec![right.clone()], limit)
        .expect("the next carrier can reserve the other complete source");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].bundle_hash, right.bundle_hash);

    let header = empty_global_block_after(Some(&parent)).header();
    let mut block = state.merge_preexecution_block(header);
    let writes_before = block.world.merge_execution_write_set_bytes();
    let error = State::preexecute_merge_execution_sources_into(&mut block, vec![left, right])
        .expect_err("a leader cannot execute two full-cap sources in one block");
    assert!(
        matches!(error, MergeLedgerCommitError::ExecutionBatchInvalid(reason)
        if reason == "autonomous sources exceed the shared block proposal gas budget")
    );
    assert_eq!(block.gas_used_in_block, 0);
    assert_eq!(
        block.world.merge_execution_write_set_bytes(),
        writes_before,
        "aggregate rejection precedes contract execution and any per-transaction rejection"
    );
}

state_test!(consensus_stack autonomous_merge_gas_priority_preserves_old_source_and_canonical_order
    autonomous_merge_gas_priority_preserves_old_source_and_canonical_order_on_consensus_stack();
);
fn autonomous_merge_gas_priority_preserves_old_source_and_canonical_order_on_consensus_stack() {
    let (state, keys, mut parent) = autonomous_gas_budget_fixture();
    let limit = gas_limit_from_parameters(state.world.view().parameters());
    let source_gas = limit / 2;
    let older = autonomous_gas_budget_source(&state, &keys, LaneId::new(1), source_gas, 0xD0);
    for tag in 0xD1..=0xD3 {
        let successor = empty_global_block_after(Some(&parent));
        state
            .kura
            .store_block(Arc::new(successor.clone()))
            .expect("advance busy-lane fixture");
        commit_block_metadata_to_state(&state, &successor);
        parent = successor;
        let newer = autonomous_gas_budget_source(&state, &keys, LaneId::SINGLE, source_gas, tag);
        assert!(
            older.origin_proposal.descriptor.proposal_height
                < newer.origin_proposal.descriptor.proposal_height
        );
        for sources in [
            vec![newer.clone(), older.clone()],
            vec![older.clone(), newer.clone()],
        ] {
            let selected = select_merge_execution_source_budget(sources, source_gas)
                .expect("one-source shared budget");
            assert_eq!(selected.len(), 1);
            assert_eq!(
                selected[0].bundle_hash, older.bundle_hash,
                "new work on a lower-numbered lane cannot overtake the retained source"
            );
        }
        let selected = select_merge_execution_source_budget(vec![newer, older.clone()], limit)
            .expect("both sources fit the complete shared budget");
        assert_eq!(selected[0].bundle_hash, older.bundle_hash);
        let batch = state
            .build_merge_execution_batch_from_source_prefix(
                1,
                empty_global_block_after(Some(&parent)).header(),
                selected,
            )
            .expect("selected sources are restored to canonical execution order");
        assert_eq!(
            batch
                .lanes
                .iter()
                .map(|lane| lane.proposal.descriptor.lane_id)
                .collect::<Vec<_>>(),
            vec![LaneId::SINGLE, LaneId::new(1)]
        );
    }
}
