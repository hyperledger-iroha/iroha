#[test]
fn payment_rejects_missing_stale_substituted_and_consumed_intent_before_effects() {
    let bootstrap = valid_bootstrap_instruction();
    let payment = valid_payment_instruction();
    let state = state_with_activation(active_lifecycle());
    let mut block = state.block(test_header());
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        bootstrap
            .execute(&ALICE_ID, &mut transaction)
            .expect("complete native bootstrap");
        transaction.apply();
    }
    let assert_unchanged = |transaction: &StateTransaction<'_, '_>,
                            expected_maps: (usize, usize, usize, usize),
                            expected_budget: (u32, u64, u32, u64)| {
        assert_eq!(privacy_map_counts(transaction), expected_maps);
        assert_eq!(
            transaction.privacy_budget_for_testing(),
            expected_budget,
            "intent rejection must not reserve privacy budget"
        );
    };
    {
        let mut transaction = block.transaction();
        let before = privacy_map_counts(&transaction);
        let budget_before = transaction.privacy_budget_for_testing();
        let error = payment
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("contract, trigger, IVM, and ad-hoc paths have no direct binding");
        assert!(
            smart_contract_parameter_message(&error).contains("no bound direct"),
            "{error:?}"
        );
        assert_unchanged(&transaction, before, budget_before);
    }
    {
        let mut transaction = block.transaction();
        let before = privacy_map_counts(&transaction);
        let budget_before = transaction.privacy_budget_for_testing();
        let submission_hash = crate::privacy::privacy_signed_submission_hash_v1(&payment)
            .expect("payment submission hash");
        transaction.bind_privacy_transaction_intent_v1(Some((
            PrivacyTransactionIntentDigestV1::new([0xEE; 32]),
            submission_hash,
        )));
        let error = payment
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("stale signed-payload binding");
        assert!(
            smart_contract_parameter_message(&error).contains("digest differs"),
            "{error:?}"
        );
        assert_unchanged(&transaction, before, budget_before);
    }
    {
        let mut substituted = payment.clone();
        substituted.envelope.proof.bytes_mut().bytes[0] ^= 1;
        let mut transaction = block.transaction();
        let before = privacy_map_counts(&transaction);
        let budget_before = transaction.privacy_budget_for_testing();
        bind_payment_instruction(&mut transaction, &payment);
        let error = substituted
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("child overlay substituted another proof");
        assert!(
            smart_contract_parameter_message(&error).contains("differs from the exact direct"),
            "{error:?}"
        );
        assert_unchanged(&transaction, before, budget_before);
    }
    {
        let mut transaction = block.transaction();
        let before = privacy_map_counts(&transaction);
        let budget_before = transaction.privacy_budget_for_testing();
        bind_payment_instruction(&mut transaction, &payment);
        let digest = payment
            .envelope
            .statement
            .context()
            .transaction_intent_digest;
        let submission_hash = crate::privacy::privacy_signed_submission_hash_v1(&payment)
            .expect("payment submission hash");
        transaction
            .consume_privacy_transaction_intent_v1(digest, submission_hash)
            .expect("simulate prior child consumption");
        let error = payment
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("the exact submission cannot be replayed in a child overlay");
        assert!(
            smart_contract_parameter_message(&error).contains("already been consumed"),
            "{error:?}"
        );
        assert_unchanged(&transaction, before, budget_before);
    }
}
#[test]
fn tampered_pgc_payment_proof_preserves_every_state_map_and_budget() {
    let bootstrap = valid_bootstrap_instruction();
    let mut payment = valid_payment_instruction();
    let PrivacyProofV1::AnonymousPgcKOutOfNV1(proof) = &mut payment.envelope.proof else {
        unreachable!("Anonymous PGC payment fixture")
    };
    let middle = proof.bytes.len() / 2;
    proof.bytes[middle] ^= 1;
    let state = state_with_activation(active_lifecycle());
    let mut block = state.block(test_header());
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        bootstrap
            .execute(&ALICE_ID, &mut transaction)
            .expect("complete native bootstrap");
        transaction.apply();
    }
    let mut transaction = block.transaction();
    let invariants_before = transaction
        .world
        .privacy_pgc_pool_invariants
        .iter()
        .map(|(key, value)| (*key, *value))
        .collect::<Vec<_>>();
    let accounts_before = transaction
        .world
        .privacy_pgc_accounts
        .iter()
        .map(|(key, value)| (*key, *value))
        .collect::<Vec<_>>();
    let roots_before = transaction
        .world
        .privacy_roots
        .iter()
        .map(|(key, value)| (*key, *value))
        .collect::<Vec<_>>();
    let heads_before = transaction
        .world
        .privacy_root_heads
        .iter()
        .map(|(key, value)| (*key, *value))
        .collect::<Vec<_>>();
    let budget_before = transaction.privacy_budget_for_testing();
    bind_payment_instruction(&mut transaction, &payment);
    let transaction_hash = transaction
        .current_tx_hash
        .map(|hash| *hash.as_ref())
        .expect("bound payment transaction hash");
    let receipt_key = PrivacyCommitmentKeyV1::verified_action_execution_receipt(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        transaction_hash,
        0,
    )
    .expect("payment receipt key");
    let error = payment
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("one-bit proof mutation");
    assert_eq!(
        smart_contract_parameter_message(&error),
        "privacy proof admission rejected: native Anonymous-PGC verification failed: \
             Anonymous-PGC payment proof equation failed",
        "unexpected typed proof rejection: {error:?}"
    );
    assert_eq!(
        transaction
            .world
            .privacy_pgc_pool_invariants
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>(),
        invariants_before
    );
    assert_eq!(
        transaction
            .world
            .privacy_pgc_accounts
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>(),
        accounts_before
    );
    assert_eq!(
        transaction
            .world
            .privacy_roots
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>(),
        roots_before
    );
    assert_eq!(
        transaction
            .world
            .privacy_root_heads
            .iter()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>(),
        heads_before
    );
    assert_eq!(
        transaction.privacy_budget_for_testing(),
        budget_before,
        "failed native verification cannot reserve transaction or block budget"
    );
    assert_eq!(
        transaction.world.privacy_commitments.get(&receipt_key),
        None,
        "rejected native verification cannot persist an execution receipt"
    );
}
#[test]
fn duplicate_execution_receipt_rejects_before_pgc_effect_mutation() {
    let bootstrap = valid_bootstrap_instruction();
    let payment = valid_payment_instruction();
    let state = state_with_activation(active_lifecycle());
    let mut block = state.block(test_header());
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        bootstrap
            .execute(&ALICE_ID, &mut transaction)
            .expect("complete native bootstrap");
        transaction.apply();
    }
    let mut transaction = block.transaction();
    bind_payment_instruction(&mut transaction, &payment);
    let transaction_hash = transaction
        .current_tx_hash
        .map(|hash| *hash.as_ref())
        .expect("bound payment transaction hash");
    let capability_manifest = transaction
        .privacy_capability_snapshot_v1()
        .expect("valid test capability snapshot")
        .exact12_capability_manifest_v1()
        .expect("valid test Exact12 capability manifest");
    let receipt_key = PrivacyCommitmentKeyV1::verified_action_execution_receipt(
        PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
        transaction_hash,
        0,
    )
    .expect("payment receipt key");
    let receipt =
        PrivacyActionExecutionReceiptRecordV1::new(PrivacyActionExecutionReceiptFieldsV1 {
            network_id: transaction.network_id,
            protocol_id: PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            operation_schema: PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1,
            ledger_effect_kind: PrivacyLedgerEffectKindV1::AnonymousPgcAccountStateTransition,
            transaction_hash,
            action_index: 0,
            transaction_intent_digest: payment
                .envelope
                .statement
                .context()
                .transaction_intent_digest,
            statement_digest: payment.envelope.statement_digest,
            proof_envelope_hash: *Hash::new(
                &norito::to_bytes(&payment.envelope).expect("canonical payment envelope"),
            )
            .as_ref(),
            capability_manifest_digest: capability_manifest.manifest_digest,
            capability_committed_height: capability_manifest.committed_height,
            admitted_at_height: transaction.block_height(),
        })
        .expect("canonical pre-existing payment receipt");
    transaction.world.privacy_commitments.insert(
        receipt_key,
        PrivacyStateItemRecordV1::VerifiedActionExecutionReceipt { receipt },
    );
    let snapshot = |transaction: &StateTransaction<'_, '_>| {
        (
            transaction
                .world
                .privacy_pgc_accounts
                .iter()
                .map(|(key, value)| norito::to_bytes(&(*key, *value)).expect("PGC account bytes"))
                .collect::<Vec<_>>(),
            transaction
                .world
                .privacy_roots
                .iter()
                .map(|(key, value)| norito::to_bytes(&(*key, *value)).expect("PGC root bytes"))
                .collect::<Vec<_>>(),
            transaction
                .world
                .privacy_root_heads
                .iter()
                .map(|(key, value)| norito::to_bytes(&(*key, *value)).expect("PGC head bytes"))
                .collect::<Vec<_>>(),
            transaction
                .world
                .privacy_commitments
                .iter()
                .map(|(key, value)| {
                    norito::to_bytes(&(*key, value.clone())).expect("privacy commitment bytes")
                })
                .collect::<Vec<_>>(),
            transaction.privacy_budget_for_testing(),
        )
    };
    let before = snapshot(&transaction);
    let error = payment
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("duplicate native execution receipt must reject");
    assert!(
        smart_contract_parameter_message(&error).contains("execution receipt already exists"),
        "{error:?}"
    );
    assert_eq!(
        snapshot(&transaction),
        before,
        "duplicate receipt rejection mutated PGC effects, receipts, or budget"
    );
}
#[test]
fn verified_pgc_payment_replaces_complete_table_atomically_and_replay_rejects() {
    let bootstrap = valid_bootstrap_instruction();
    let payment = valid_payment_instruction();
    let payment_bytes = u64::try_from(
        norito::to_bytes(&payment.envelope)
            .expect("payment encoding")
            .len(),
    )
    .expect("payment length");
    let state = state_with_activation(active_lifecycle());
    let header = test_header();
    let header_hash = header.hash();
    let mut block = state.block(header);
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        bootstrap
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect("complete native bootstrap");
        transaction.apply();
    }
    {
        let mut transaction = block.transaction();
        let invariant_key = PrivacyPgcPoolInvariantKeyV1::new(bootstrap.bootstrap.namespace)
            .expect("invariant key");
        let invariant_before = *transaction
            .world
            .privacy_pgc_pool_invariants
            .get(&invariant_key)
            .expect("bootstrapped invariant");
        let first_key = PrivacyPgcAccountKeyV1::new(
            bootstrap.bootstrap.namespace,
            bootstrap.bootstrap.accounts[0].public_key,
        )
        .expect("first account key");
        let first_balance_before = transaction
            .world
            .privacy_pgc_accounts
            .get(&first_key)
            .expect("first account")
            .encrypted_balance();
        bind_payment_instruction(&mut transaction, &payment);
        let transaction_hash = transaction
            .current_tx_hash
            .map(|hash| *hash.as_ref())
            .expect("bound payment transaction hash");
        payment
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect("complete native payment");
        let receipt_key = PrivacyCommitmentKeyV1::verified_action_execution_receipt(
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1,
            transaction_hash,
            0,
        )
        .expect("payment receipt key");
        let receipt_record = transaction
            .world
            .privacy_commitments
            .get(&receipt_key)
            .expect("successful native payment receipt");
        let PrivacyStateItemRecordV1::VerifiedActionExecutionReceipt { receipt } = receipt_record
        else {
            panic!("payment receipt key contains wrong-role provenance");
        };
        assert_eq!(receipt.network_id, test_network_id());
        assert_eq!(
            receipt.protocol_id,
            PrivacyProtocolIdV1::AnonymousPgcKOutOfNV1
        );
        assert_eq!(
            receipt.operation_schema,
            PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1
        );
        assert_eq!(
            receipt.ledger_effect_kind,
            PrivacyLedgerEffectKindV1::AnonymousPgcAccountStateTransition
        );
        assert_eq!(receipt.transaction_hash, transaction_hash);
        assert_eq!(
            receipt.transaction_intent_digest,
            payment
                .envelope
                .statement
                .context()
                .transaction_intent_digest
        );
        assert_eq!(receipt.statement_digest, payment.envelope.statement_digest);
        assert_eq!(
            receipt.proof_envelope_hash,
            *Hash::new(&norito::to_bytes(&payment.envelope).expect("canonical payment envelope"))
                .as_ref()
        );
        receipt
            .validate()
            .expect("persisted payment receipt validates");
        assert_eq!(privacy_map_counts(&transaction), (1, 16, 2, 1));
        assert_eq!(
            transaction
                .world
                .privacy_pgc_pool_invariants
                .get(&invariant_key),
            Some(&invariant_before),
            "payments cannot replace bootstrap supply provenance"
        );
        assert_ne!(
            transaction
                .world
                .privacy_pgc_accounts
                .get(&first_key)
                .expect("updated first account")
                .encrypted_balance(),
            first_balance_before,
            "the complete successor table must replace current ciphertexts"
        );
        let head_key = PrivacyRootHeadKeyV1::new(
            bootstrap.bootstrap.namespace,
            PrivacyRootRoleV1::PgcAccountState,
        )
        .expect("head key");
        assert_eq!(
            transaction
                .world
                .privacy_root_heads
                .get(&head_key)
                .expect("payment head")
                .epoch(),
            2
        );
        let budget = transaction.privacy_budget_for_testing();
        assert_eq!(budget.0, 1);
        assert_eq!(budget.1, payment_bytes);
        assert_eq!(budget.2, 2);
        transaction.apply();
    }
    {
        let mut transaction = block.transaction();
        let mut next_limits = PrivacyConsensusLimitsV1::taira_default();
        next_limits.retained_root_count = 1;
        SchedulePrivacyConsensusPolicyTighteningV1::new(TEST_BLOCK_HEIGHT + 300, next_limits)
            .execute(&ALICE_ID, &mut transaction)
            .expect("schedule exact delayed PGC retention tightening");
        transaction.apply();
    }
    block.commit().expect("commit bootstrap and payment block");
    let next_header = BlockHeader::new(
        NonZeroU64::new(TEST_BLOCK_HEIGHT + 300).expect("effective height"),
        Some(header_hash),
        None,
        None,
        1_800_000_000_001,
        0,
    );
    let mut next_block = state.block(next_header);
    let mut transaction = next_block.transaction();
    assert_eq!(
        privacy_map_counts(&transaction),
        (1, 16, 1, 1),
        "effective-height hook must prune PGC history to the tightened cap"
    );
    let head_key = PrivacyRootHeadKeyV1::new(
        bootstrap.bootstrap.namespace,
        PrivacyRootRoleV1::PgcAccountState,
    )
    .expect("PGC head key");
    let head = transaction
        .world
        .privacy_root_heads
        .get(&head_key)
        .expect("pruned PGC head");
    let anchor = head
        .retention_anchor()
        .expect("pruning must commit the removed prefix anchor");
    assert_eq!(anchor.epoch(), bootstrap.bootstrap.initial_epoch);
    assert_eq!(anchor.root(), bootstrap.bootstrap.initial_root);
    assert_eq!(
        transaction
            .world
            .privacy_consensus_policy
            .get()
            .current_limits
            .retained_root_count,
        1
    );
    assert_eq!(
        transaction
            .world
            .privacy_consensus_policy
            .get()
            .pending_tightening,
        None
    );
    let counts_before = privacy_map_counts(&transaction);
    bind_payment_instruction(&mut transaction, &payment);
    let error = payment
        .execute(&ALICE_ID, &mut transaction)
        .expect_err("stale payment replay");
    assert!(
        smart_contract_parameter_message(&error).contains("StaleHead"),
        "unexpected replay rejection: {error:?}"
    );
    assert_eq!(privacy_map_counts(&transaction), counts_before);
    assert_eq!(
        transaction.privacy_budget_for_testing(),
        (0, 0, 0, 0),
        "failed replay must not consume the new block budget"
    );
}
