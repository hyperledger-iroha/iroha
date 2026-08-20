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

    let control = build_block_with_envelope_records(
        vec![
            make_envelope(1, 5, b"split-envelope-control-a"),
            make_envelope(2, 5, b"split-envelope-control-b"),
        ],
        snapshot,
    );
    let control_state_block = state.block(control.header());
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
        let (mut state, issuer, issuer_uaid, manifest_root) =
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
        let prior_replay_record =
            (stage_executed_record && previous_amount.is_some()).then_some(AxtReplayRecord {
                dataspace: dsid,
                used_slot: 1,
                retain_until_slot: 1,
            });
        if let Some(prior_replay_record) = prior_replay_record {
            let mut ledger = state.world.axt_replay_ledger.block();
            ledger.insert(replay_key, prior_replay_record);
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
        state.set_axt_policy(dsid, policy);
        let mut snapshot = axt_policy_snapshot_for_validation_test(&state);
        snapshot.entries[0].policy.next_handle_counter = current_sub_nonce + 1;
        snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
        let block = if stage_executed_record {
            build_block_with_envelope_records_at_ms(vec![current], snapshot, 2)
        } else {
            build_block_with_envelopes(current, snapshot)
        };
        let mut state_block = state.block(block.header());
        if stage_executed_record {
            let mut executed_record = budget_record.clone();
            executed_record
                .try_consume(&budget_key, &Quantity::from(current_amount), 20)
                .expect("the executed control fits its cumulative family allowance");
            state_block
                .world
                .axt_handle_budget_ledger
                .insert(budget_key.clone(), executed_record);
            let expected_previous = previous_amount.map(Quantity::from);
            assert_eq!(
                state_block
                    .axt_handle_budget_record_at_block_start(&budget_key)
                    .map(iroha_data_model::nexus::AxtHandleBudgetRecord::consumed),
                expected_previous.as_ref(),
                "the on-demand admission view must expose the exact pre-block family record"
            );
            let executed_replay_record = AxtReplayRecord {
                dataspace: dsid,
                used_slot: 2,
                retain_until_slot: 20,
            };
            state_block
                .world
                .axt_replay_ledger
                .insert(replay_key, executed_replay_record);
            assert_eq!(
                state_block.axt_replay_record_at_block_start(&replay_key),
                prior_replay_record.as_ref(),
                "the on-demand admission view must expose the exact pre-block replay state"
            );
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
    run_case(Some(5), 5, false)
        .expect("two blocks may consume exactly the signed family allowance");
    run_case(Some(5), 5, true).expect(
        "post-execution validation must hydrate the prior record instead of double-charging it",
    );
    run_case(None, 7, true)
        .expect("post-execution validation must preserve a family's pre-block absence");
}
