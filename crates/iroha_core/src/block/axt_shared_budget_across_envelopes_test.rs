#[test]
fn axt_validation_enforces_shared_budget_across_envelopes() {
    let dsid = DataSpaceId::new(122);
    let lane = LaneId::new(2);
    let (state, issuer, issuer_uaid, manifest_root) =
        authenticated_axt_validation_state(dsid, lane, 0x7A);
    let descriptor = AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = binding_for_descriptor(&descriptor);
    let make_envelope = |sub_nonce: u64, amount: u64, proof_seed: &[u8]| {
        let amount = Quantity::from(amount);
        let mut handle = sample_handle(binding, lane, dsid, 20, manifest_root);
        handle.handle.sub_nonce = sub_nonce;
        handle.intent.op.amount = Some(amount.clone());
        handle.amount = Some(amount.clone());
        let handle =
            sign_axt_validation_handle(handle, &state, &issuer, issuer_uaid, manifest_root);
        let proof = proof_blob_for_with_amount(
            dsid,
            manifest_root,
            proof_seed,
            20,
            None,
            None,
            vec![remote_spend_claim(&handle, &amount)],
        );
        AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: descriptor.clone(),
            touches: Vec::new(),
            proofs: vec![AxtProofFragment { dsid, proof }],
            handles: vec![handle],
            commit_height: 1,
        }
    };
    let mut snapshot = axt_policy_snapshot_for_validation_test(&state);
    snapshot.entries[0].policy.next_handle_counter = 3;
    snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);

    let attack = build_block_with_envelope_records(
        vec![
            make_envelope(1, 7, b"split-envelope-budget-a"),
            make_envelope(2, 7, b"split-envelope-budget-b"),
        ],
        snapshot.clone(),
    );
    let attack_state_block = state.block(attack.header());
    let error = validate_axt_envelopes(&attack, &attack_state_block).unwrap_err();
    expect_axt_error(
        error,
        AxtRejectReason::Budget,
        "shared handle budget exceeded across blocks or AXT envelopes",
    );
    drop(attack_state_block);

    let control_envelopes = vec![
        make_envelope(1, 5, b"split-envelope-control-a"),
        make_envelope(2, 5, b"split-envelope-control-b"),
    ];
    let control = build_block_with_envelope_records(control_envelopes.clone(), snapshot);
    let mut control_state_block = state.block(control.header());
    for envelope in control_envelopes {
        let mut executed = control_state_block.transaction();
        executed
            .record_axt_envelope(envelope)
            .expect("each below-cap envelope must execute");
        executed.apply();
    }
    validate_axt_envelopes(&control, &control_state_block)
        .expect("two completed envelopes may consume exactly the shared signed budget");
}

#[test]
fn axt_validation_persists_hidden_family_budget_across_blocks() {
    fn run_case(
        previous_amount: Option<u64>,
        current_amount: u64,
        stage_executed_record: bool,
    ) -> Result<(), BlockValidationError> {
        let dsid = DataSpaceId::new(123);
        let lane = LaneId::new(2);
        let (state, issuer, issuer_uaid, manifest_root) =
            authenticated_axt_validation_state(dsid, lane, 0x7B);
        let descriptor = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![AxtTouchSpec {
                dsid,
                read: Vec::new(),
                write: Vec::new(),
            }],
        };
        let binding = binding_for_descriptor(&descriptor);
        let make_envelope = |sub_nonce: u64, amount: u64, proof_seed: &[u8]| {
            let amount_quantity = Quantity::from(amount);
            let mut handle = sample_handle(binding, lane, dsid, 20, manifest_root);
            handle.handle.sub_nonce = sub_nonce;
            handle.intent.op.amount = None;
            handle.amount = None;
            let mut handle =
                sign_axt_validation_handle(handle, &state, &issuer, issuer_uaid, manifest_root);
            let (proof, amount_commitment) = proof_blob_for_with_authenticated_amount(
                dsid,
                manifest_root,
                proof_seed,
                20,
                u128::from(amount),
                vec![remote_spend_claim(&handle, &amount_quantity)],
            );
            handle.amount_commitment = Some(amount_commitment);
            handle.proof = Some(proof);
            AxtEnvelopeRecord {
                binding,
                lane,
                descriptor: descriptor.clone(),
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: Vec::new(),
                handles: vec![handle],
                commit_height: 1,
            }
        };
        let current_sub_nonce = if previous_amount.is_some() { 2 } else { 1 };
        let current = make_envelope(
            current_sub_nonce,
            current_amount,
            b"cross-block-hidden-current",
        );
        let budget_key =
            iroha_data_model::nexus::AxtHandleBudgetKey::from_handle(&current.handles[0].handle);
        let mut budget_record = iroha_data_model::nexus::AxtHandleBudgetRecord::empty();
        if let Some(previous_amount) = previous_amount {
            budget_record
                .try_consume(
                    &budget_key,
                    &Quantity::from(previous_amount),
                    current.handles[0].handle.expiry_slot,
                )
                .expect("the prior block's hidden amount fits its signed budget");
            let mut ledger = state.world.axt_handle_budget_ledger.block();
            ledger.insert(budget_key.clone(), budget_record.clone());
            ledger.commit();
        }
        let replay_key = AxtHandleReplayKey::from_handle(dsid, &current.handles[0].handle);
        let previous_replay_key = previous_amount.map(|_| {
            let mut previous_handle = current.handles[0].handle.clone();
            previous_handle.sub_nonce = current_sub_nonce - 1;
            AxtHandleReplayKey::from_handle(dsid, &previous_handle)
        });
        let prior_replay_record = previous_amount.map(|_| AxtReplayRecord {
            dataspace: dsid,
            budget_key: budget_key.clone(),
            used_slot: 1,
            retain_until_slot: 1,
        });
        if let Some(prior_replay_record) = prior_replay_record.as_ref() {
            let mut ledger = state.world.axt_replay_ledger.block();
            ledger.insert(
                previous_replay_key.expect("prior amount has a prior replay key"),
                prior_replay_record.clone(),
            );
            ledger.commit();
        }
        let mut policy = state
            .world
            .axt_policies
            .view()
            .get(&dsid)
            .copied()
            .expect("fixture policy");
        policy.next_handle_counter = current_sub_nonce;
        {
            let mut policies = state.world.axt_policies.block();
            policies.insert(dsid, policy);
            policies.commit();
            let mut counters = state.world.axt_handle_counters.block();
            counters.insert(
                dsid,
                iroha_data_model::nexus::AxtHandleCounterRecord::try_from_parts(
                    current_sub_nonce,
                    policy.active_handle_era,
                )
                .expect("cross-block fixture counter"),
            );
            counters.commit();
        }
        let mut snapshot = axt_policy_snapshot_for_validation_test(&state);
        snapshot.entries[0].policy.next_handle_counter = current_sub_nonce + 1;
        snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
        let block = if stage_executed_record {
            build_block_with_envelope_records_at_ms(vec![current.clone()], snapshot, 2)
        } else {
            build_block_with_envelopes(current.clone(), snapshot)
        };
        let mut state_block = state.block(block.header());
        assert_eq!(
            state_block.axt_replay_record_at_block_start(&replay_key),
            None,
            "the next exact handle key must be absent from the pre-block replay state"
        );
        if let (Some(previous_replay_key), Some(prior_replay_record)) =
            (previous_replay_key, prior_replay_record.as_ref())
        {
            assert_eq!(
                state_block.axt_replay_record_at_block_start(&previous_replay_key),
                Some(prior_replay_record),
                "the prior consumed handle must remain present in the pre-block replay state"
            );
        }
        if stage_executed_record {
            let mut executed = state_block.transaction();
            executed
                .record_axt_envelope(current)
                .expect("the below-cap current envelope must execute");
            executed.apply();
            let expected_previous = previous_amount.map(Quantity::from);
            assert_eq!(
                state_block
                    .axt_handle_budget_record_at_block_start(&budget_key)
                    .map(iroha_data_model::nexus::AxtHandleBudgetRecord::consumed),
                expected_previous.as_ref(),
                "the on-demand admission view must expose the exact pre-block family record"
            );
            assert_eq!(
                state_block.axt_replay_record_at_block_start(&replay_key),
                None,
                "the next exact handle key must be absent from the pre-block replay state"
            );
            if let (Some(previous_replay_key), Some(prior_replay_record)) =
                (previous_replay_key, prior_replay_record.as_ref())
            {
                assert_eq!(
                    state_block.axt_replay_record_at_block_start(&previous_replay_key),
                    Some(prior_replay_record),
                    "the on-demand admission view must retain the prior handle's replay state"
                );
            }
        }
        validate_axt_envelopes(&block, &state_block)
    }

    let error = run_case(Some(7), 7, false).expect_err(
        "splitting one hidden-amount family across blocks must not reset its allowance",
    );
    expect_axt_error(
        error,
        AxtRejectReason::Budget,
        "shared handle budget exceeded across blocks or AXT envelopes",
    );
    run_case(Some(5), 5, true).expect(
        "two blocks may consume exactly the signed family allowance while post-execution validation hydrates the prior record",
    );
    run_case(None, 7, true)
        .expect("post-execution validation must preserve a family's pre-block absence");
}

#[test]
fn axt_validation_accepts_authenticated_hidden_amount() {
    let (state, envelope) = hidden_amount_fixture(17, 0x31, b"authenticated-hidden-amount");
    let mut snapshot = axt_policy_snapshot_for_validation_test(&state);
    snapshot.entries[0].policy.next_handle_counter = 2;
    snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
    let block = build_block_with_envelopes(envelope.clone(), snapshot);
    let mut state_block = state.block(block.header());
    {
        let mut executed = state_block.transaction();
        executed
            .record_axt_envelope(envelope)
            .expect("authenticated hidden-amount control must execute");
        executed.apply();
    }
    validate_axt_envelopes(&block, &state_block)
        .expect("authenticated hidden amount must pass block admission");
}

#[test]
fn axt_validation_rejects_opaque_authorization_carrier_at_generic_boundary() {
    let dsid = DataSpaceId::new(117);
    let lane = LaneId::new(1);
    let (state, _, _, manifest_root) = authenticated_axt_validation_state(dsid, lane, 0x75);
    let descriptor = AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let envelope = AxtEnvelopeRecord {
        binding: binding_for_descriptor(&descriptor),
        lane,
        descriptor,
        touches: Vec::new(),
        proofs: vec![AxtProofFragment {
            dsid,
            proof: opaque_proof_blob_for(
                dsid,
                manifest_root,
                b"opaque-generic-block-attack",
                12,
            ),
        }],
        handles: Vec::new(),
        commit_height: 1,
    };
    expect_axt_envelope_error(
        &state,
        envelope,
        AxtRejectReason::Proof,
        "requires a witnessed transfer claim",
    );
}

#[test]
fn axt_validation_enforces_registered_asset_balance_policy() {
    let (restricted_state, restricted_envelope) =
        hidden_amount_fixture(118, 0x76, b"restricted-private-dataspace-control");
    let mut restricted_snapshot = axt_policy_snapshot_for_validation_test(&restricted_state);
    restricted_snapshot.entries[0].policy.next_handle_counter = 2;
    restricted_snapshot.version = AxtPolicySnapshot::compute_version(&restricted_snapshot.entries);
    let restricted_block =
        build_block_with_envelopes(restricted_envelope.clone(), restricted_snapshot);
    let mut restricted_state_block = restricted_state.block(restricted_block.header());
    {
        let mut executed = restricted_state_block.transaction();
        executed
            .record_axt_envelope(restricted_envelope.clone())
            .expect("restricted asset control must execute");
        executed.apply();
    }
    validate_axt_envelopes(&restricted_block, &restricted_state_block)
        .expect("a registered restricted asset may use its exact signed intent dataspace");
    drop(restricted_state_block);

    let (global_state, global_envelope) = hidden_amount_fixture_with_asset_policy(
        118,
        0x76,
        b"restricted-private-dataspace-control",
        iroha_data_model::asset::AssetBalancePolicy::Global,
    );
    expect_axt_envelope_error(
        &global_state,
        global_envelope,
        AxtRejectReason::PolicyDenied,
        "does not belong to the intent dataspace",
    );

    let missing_state = restricted_state;
    let mut world = missing_state.world.block();
    assert!(
        world
            .asset_definitions
            .remove(sample_asset_definition_id())
            .is_some(),
        "fixture asset definition must be present before removal"
    );
    world.commit();
    expect_axt_envelope_error(
        &missing_state,
        restricted_envelope,
        AxtRejectReason::PolicyDenied,
        "asset definition is not registered",
    );
}
