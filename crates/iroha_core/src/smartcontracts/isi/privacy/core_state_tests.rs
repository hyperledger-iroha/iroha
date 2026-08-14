// Same-scope regression coverage extracted to keep the parent source budget bounded.
#[test]
fn fcmp_submit_rejections_and_transaction_drop_preserve_exact_proof_managed_state() {
    let fixture = fcmp_runtime_fixture_for_test();
    let state = state_with_fcmp_runtime_fixture(&fixture);
    let namespace = fixture.snapshot.namespace();
    let config_key =
        PrivacyCommitmentKeyV1::proof_managed_pool_config(namespace).expect("FCMP++ config key");
    let PrivacyStatementV1::MoneroFcmpPlusPlusV1(valid_statement) = &fixture.envelope.statement
    else {
        unreachable!("FCMP++ runtime fixture")
    };
    let key_image = valid_statement.inputs[0].key_image;
    let nullifier_count =
        u32::try_from(valid_statement.inputs.len()).expect("FCMP++ key-image count");
    let output_count = u32::try_from(valid_statement.outputs.len()).expect("FCMP++ output count");
    {
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        transaction.world.privacy_nullifiers.insert(
            PrivacyNullifierKeyV1::fcmp_key_image(namespace, key_image).expect("FCMP++ replay key"),
            PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
                fixture.snapshot.bootstrap_digest(),
                PrivacyStatementDigestV1::new([0xE1; 32]),
                nullifier_count,
                output_count,
                fixture
                    .current_height
                    .checked_sub(1)
                    .expect("FCMP++ fixture height follows genesis"),
                0,
            )
            .expect("FCMP++ replay record"),
        );
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            SubmitPrivacyProofV1::new(fixture.envelope.clone()),
            config_key,
            "FCMP++ key image was already consumed",
        );
    }
    {
        let mut foreign_bootstrap_digest = fixture.snapshot.bootstrap_digest();
        foreign_bootstrap_digest.0[0] ^= 1;
        assert_ne!(
            foreign_bootstrap_digest,
            fixture.snapshot.bootstrap_digest()
        );
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        transaction.world.privacy_nullifiers.insert(
            PrivacyNullifierKeyV1::fcmp_key_image(namespace, key_image)
                .expect("FCMP++ cross-bootstrap replay key"),
            PrivacyStateItemRecordV1::proof_managed_pool_verified_nullifier(
                foreign_bootstrap_digest,
                PrivacyStatementDigestV1::new([0xE2; 32]),
                nullifier_count,
                output_count,
                fixture
                    .current_height
                    .checked_sub(1)
                    .expect("FCMP++ fixture height follows genesis"),
                0,
            )
            .expect("FCMP++ cross-bootstrap replay record"),
        );
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            SubmitPrivacyProofV1::new(fixture.envelope.clone()),
            config_key,
            "persisted FCMP++ key image has cross-bootstrap provenance",
        );
    }
    {
        let mut foreign_bootstrap_digest = fixture.snapshot.bootstrap_digest();
        foreign_bootstrap_digest.0[0] ^= 1;
        assert_ne!(
            foreign_bootstrap_digest,
            fixture.snapshot.bootstrap_digest()
        );
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        transaction.world.privacy_commitments.insert(
            PrivacyCommitmentKeyV1::fcmp_output(namespace, fixture.initial_output.output_id())
                .expect("FCMP++ cross-bootstrap output key"),
            PrivacyStateItemRecordV1::fcmp_bootstrap_output(
                foreign_bootstrap_digest,
                fixture.initial_output,
                0,
                fixture.snapshot.bootstrap_admitted_at_height(),
            )
            .expect("FCMP++ cross-bootstrap output record"),
        );
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            SubmitPrivacyProofV1::new(fixture.envelope.clone()),
            config_key,
            "FCMP++ output key or provenance differs from its complete tuple",
        );
    }
    {
        let mut duplicate_output = SubmitPrivacyProofV1::new(fixture.envelope.clone());
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) =
            &mut duplicate_output.envelope.statement
        else {
            unreachable!("FCMP++ runtime fixture")
        };
        statement.outputs[0] = fixture.initial_output;
        duplicate_output.envelope.statement_digest = duplicate_output
            .envelope
            .statement
            .digest()
            .expect("modified FCMP++ statement digest");
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            duplicate_output,
            config_key,
            "FCMP++ output already exists",
        );
    }
    {
        let wrong_typed_root = fixture
            .snapshot
            .derive_fcmp_successor(&valid_statement.outputs)
            .expect("FCMP++ successor")
            .root();
        let mut wrong_root = SubmitPrivacyProofV1::new(fixture.envelope.clone());
        let PrivacyStatementV1::MoneroFcmpPlusPlusV1(statement) =
            &mut wrong_root.envelope.statement
        else {
            unreachable!("FCMP++ runtime fixture")
        };
        statement.output_set_root = wrong_typed_root;
        wrong_root.envelope.statement_digest = wrong_root
            .envelope
            .statement
            .digest()
            .expect("wrong-root FCMP++ statement digest");
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            wrong_root,
            config_key,
            "anchor is not in the exact retained root window",
        );
    }
    {
        let successor = fixture
            .snapshot
            .derive_fcmp_successor(&valid_statement.outputs)
            .expect("FCMP++ successor");
        let mut block = state.block(fcmp_test_header(&fixture));
        let mut transaction = block.transaction();
        transaction.world.privacy_commitments.insert(
            config_key,
            PrivacyStateItemRecordV1::proof_managed_pool_state(
                fixture.snapshot.bootstrap().clone(),
                fixture.snapshot.bootstrap_digest(),
                fixture.snapshot.initial_root(),
                PrivacyProofManagedPoolAccumulatorStateV1::Fcmp(successor),
                fixture.snapshot.bootstrap_admitted_at_height(),
            )
            .expect("individually valid but uncommitted FCMP++ frontier"),
        );
        assert_proof_managed_submit_rejection_is_atomic(
            &mut transaction,
            SubmitPrivacyProofV1::new(fixture.envelope.clone()),
            config_key,
            "trusted proof-managed pool state failed validation",
        );
    }
    let baseline;
    {
        let mut block = state.block(fcmp_test_header(&fixture));
        {
            let mut transaction = block.transaction();
            baseline = proof_managed_state_snapshot(&transaction, config_key);
            let valid = SubmitPrivacyProofV1::new(fixture.envelope.clone());
            bind_submit_privacy_instruction(&mut transaction, &valid);
            valid
                .execute(&ALICE_ID, &mut transaction)
                .expect("valid native FCMP++ submission");
            let staged = proof_managed_state_snapshot(&transaction, config_key);
            assert_ne!(
                staged, baseline,
                "valid FCMP++ execution must stage its complete successor"
            );
            assert_ne!(
                staged.config, baseline.config,
                "valid FCMP++ execution must stage its native frontier"
            );
            assert_eq!(staged.budget.0, baseline.budget.0 + 1);
            assert_ne!(
                staged.roots, baseline.roots,
                "valid FCMP++ execution must stage its successor root"
            );
            assert_ne!(
                staged.root_heads, baseline.root_heads,
                "valid FCMP++ execution must stage its successor head"
            );
            assert!(
                staged.nullifiers.len() > baseline.nullifiers.len(),
                "valid FCMP++ execution must stage its key image"
            );
            assert!(
                staged.commitments.len() > baseline.commitments.len(),
                "valid FCMP++ execution must stage every output and successor frontier"
            );
            let late_error = SubmitPrivacyProofV1::new(fixture.envelope.clone())
                .execute(&ALICE_ID, &mut transaction)
                .expect_err("consumed direct submission must reject after staged writes");
            assert!(
                format!("{late_error:?}")
                    .contains("the signed privacy submission has already been consumed"),
                "unexpected late FCMP++ rejection: {late_error:?}"
            );
            assert_eq!(
                proof_managed_state_snapshot(&transaction, config_key),
                staged,
                "late one-shot conflict changed the already staged FCMP++ successor"
            );
            // The mutable overlay intentionally exposes no interleaving writer
            // hook. This one-shot conflict is injected after the final production
            // write, then the complete transaction is dropped below.
        }
        let transaction = block.transaction();
        assert_eq!(
            proof_managed_state_snapshot(&transaction, config_key),
            baseline,
            "dropping the successful FCMP++ transaction published staged state into its parent block"
        );
    }
    let mut block = state.block(fcmp_test_header(&fixture));
    let transaction = block.transaction();
    assert_eq!(
        proof_managed_state_snapshot(&transaction, config_key),
        baseline,
        "dropping the parent block changed committed FCMP++ state"
    );
}
