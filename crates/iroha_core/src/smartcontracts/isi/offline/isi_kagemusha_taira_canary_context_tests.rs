/// Exact signed-wire seeding and affine-consumption boundary regressions.
mod taira_canary_context_tests {
    use super::*;
    use crate::tx::AcceptedTransaction;
    use iroha_data_model::{
        events::execute_trigger::ExecuteTriggerEventFilter,
        transaction::{
            TransactionAdmissionIntent, TransactionEntrypoint,
            signed::{
                SealedTransactionCommitmentPayload, SealedTransactionReveal,
                SignedSealedTransactionCommitment, compute_sealed_transaction_commitment,
            },
        },
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    use std::borrow::Cow;

    #[test]
    fn taira_canary_committed_replay_seeds_only_one_direct_wire() {
        offline_test_transaction!(transaction);
        let canary_key = KeyPair::from_seed(vec![0xD5; 32], Algorithm::MlDsa);
        let authority = AccountId::new(canary_key.public_key().clone());
        let first = canary_consensus_fixture_for_key(&transaction, 17, &canary_key);
        let second = canary_consensus_fixture_for_key(&transaction, 17, &canary_key);
        let identity = |signed: &SignedTransaction| {
            crate::smartcontracts::isi::offline::signed_kagemusha_taira_canary_wire_identity_v1(
                signed,
            )
            .expect("derive exact canary wire identity")
        };
        let first_wire = identity(&first.canary_transaction).expect("direct canary wire");
        let second_wire =
            identity(&second.canary_transaction).expect("alternate direct canary wire");
        assert_eq!(
            first.canary_transaction.hash(),
            second.canary_transaction.hash()
        );
        assert_ne!(first_wire, second_wire);

        crate::state::seed_committed_transaction_context(
            &mut transaction,
            &TransactionEntrypoint::External(first.canary_transaction.clone()),
            3,
        );
        assert_eq!(
            transaction.kagemusha_taira_canary_wire_identity,
            Some(first_wire)
        );
        crate::state::seed_committed_transaction_context(
            &mut transaction,
            &TransactionEntrypoint::External(second.canary_transaction.clone()),
            4,
        );
        assert_eq!(
            transaction.kagemusha_taira_canary_wire_identity,
            Some(second_wire),
            "replay must derive the exact committed proof-bearing wire, not only payload intent",
        );

        let multi = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
        ])
        .sign(canary_key.private_key());
        crate::state::seed_committed_transaction_context(
            &mut transaction,
            &TransactionEntrypoint::External(multi),
            5,
        );
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);

        let queue_plan = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([RecordKagemushaTairaCanaryV4::new(first.permit.clone())])
        .with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced)
        .sign(canary_key.private_key());
        assert_eq!(identity(&queue_plan), None);
        crate::state::seed_committed_transaction_context(
            &mut transaction,
            &TransactionEntrypoint::External(queue_plan),
            6,
        );
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);

        let batch = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(
            vec![ExecutableBatchItem::Instruction(
                RecordKagemushaTairaCanaryV4::new(first.permit).into(),
            )]
            .into(),
        ))
        .sign(canary_key.private_key());
        crate::state::seed_committed_transaction_context(
            &mut transaction,
            &TransactionEntrypoint::External(batch),
            7,
        );
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);

        let sealed = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            Hash::new(b"sealed Kagemusha canary replay boundary"),
            first.canary_transaction,
            [0xD8; 32],
        ));
        crate::state::seed_committed_transaction_context(&mut transaction, &sealed, 8);
        assert!(!transaction.kagemusha_taira_canary_external_entrypoint);
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);
    }

    #[test]
    fn taira_canary_sealed_reveal_validation_cannot_gain_external_provenance() {
        let state = offline_test_state();
        let mut commitment_block = state.block(offline_test_header());
        let fixture = {
            let mut setup = commitment_block.transaction();
            let fixture = canary_consensus_fixture(&setup, 31);
            commit_canary_activation_binding(&fixture.permit.body.binding, &mut setup);
            AuthorizeKagemushaTairaCanaryV4::new(fixture.reservation.clone())
                .execute(&ALICE_ID, &mut setup)
                .expect("authorize the exact external canary before the sealed attempt");
            setup.apply();
            fixture
        };
        let salt = [0xD9; 32];
        let reveal_deadline_height = 3;
        let commitment = compute_sealed_transaction_commitment(
            state.network_id_ref(),
            &fixture.canary_transaction,
            salt,
            reveal_deadline_height,
        );
        let commitment_entrypoint =
            TransactionEntrypoint::SealedCommitment(SignedSealedTransactionCommitment::sign(
                SealedTransactionCommitmentPayload::new(
                    *state.network_id_ref(),
                    ALICE_ID.clone(),
                    commitment,
                    2,
                    reveal_deadline_height,
                    None,
                ),
                ALICE_KEYPAIR.private_key(),
            ));
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        commitment_block
            .validate_transaction(
                AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint)),
                &mut ivm_cache,
            )
            .1
            .expect("the matching sealed commitment is admitted at height one");
        commitment_block
            .commit()
            .expect("commit the pending sealed canary envelope");

        let reveal_header = BlockHeader::new(
            NonZeroU64::new(2).expect("non-zero reveal height"),
            None,
            None,
            None,
            POLICY_TEST_TIME_MS + 1,
            0,
        );
        let mut reveal_block = state.block(reveal_header);
        let replay_keys_before = reveal_block.world.kagemusha_replay_keys.iter().count();
        let reveal_entrypoint = TransactionEntrypoint::SealedReveal(SealedTransactionReveal::new(
            commitment,
            fixture.canary_transaction,
            salt,
        ));
        let error = reveal_block
            .validate_transaction(
                AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal_entrypoint)),
                &mut ivm_cache,
            )
            .1
            .expect_err("a sealed inner Record cannot consume the external canary reservation");
        assert!(
            error
                .to_string()
                .contains("canary_external_entrypoint_required")
        );
        assert_eq!(
            reveal_block.world.kagemusha_replay_keys.iter().count(),
            replay_keys_before,
            "the rejected reveal must not consume or add any canary marker",
        );
    }

    #[test]
    fn taira_canary_executor_enforces_exact_wire_shape_and_proof() {
        offline_test_transaction!(transaction);
        let canary_key = KeyPair::from_seed(vec![0xD6; 32], Algorithm::MlDsa);
        let authority = AccountId::new(canary_key.public_key().clone());
        let first = canary_consensus_fixture_for_key(&transaction, 19, &canary_key);
        let second = canary_consensus_fixture_for_key(&transaction, 19, &canary_key);
        for signed in [&first.canary_transaction, &second.canary_transaction] {
            signed
                .verify_signature()
                .expect("independent ML-DSA canary proof verifies");
        }
        assert_eq!(
            first.canary_transaction.hash(),
            second.canary_transaction.hash()
        );
        assert_ne!(
            first.canary_transaction_wire,
            second.canary_transaction_wire
        );
        commit_canary_activation_binding(&first.permit.body.binding, &mut transaction);
        AuthorizeKagemushaTairaCanaryV4::new(first.reservation.clone())
            .execute(&authority, &mut transaction)
            .expect("authorize the first exact signed wire");
        let markers_after_authorization = transaction.world.kagemusha_replay_keys.iter().count();
        let executor = transaction.world.executor.clone();
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();

        let sealed_or_contextless = executor
            .execute_transaction(
                &mut transaction,
                &authority,
                first.canary_transaction.clone(),
                &mut ivm_cache,
            )
            .expect_err("a signed canary without External entrypoint provenance must fail");
        assert!(
            sealed_or_contextless
                .to_string()
                .contains("canary_external_entrypoint_required")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_after_authorization,
        );
        transaction.kagemusha_taira_canary_external_entrypoint = true;

        let alternate = executor
            .execute_transaction(
                &mut transaction,
                &authority,
                second.canary_transaction,
                &mut ivm_cache,
            )
            .expect_err("an independently valid signature over the same payload must not pass");
        assert!(
            alternate
                .to_string()
                .contains("canary_authorization_missing")
        );
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_after_authorization,
        );

        let multi = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
            RecordKagemushaTairaCanaryV4::new(first.permit.clone()),
        ])
        .sign(canary_key.private_key());
        let multi_error = executor
            .execute_transaction(&mut transaction, &authority, multi, &mut ivm_cache)
            .expect_err("a multi-instruction transaction must receive no canary wire capability");
        assert!(
            multi_error
                .to_string()
                .contains("canary_authorization_missing")
        );

        let batch = TransactionBuilder::new(
            transaction.network_id().clone(),
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(Executable::Batch(
            vec![ExecutableBatchItem::Instruction(
                RecordKagemushaTairaCanaryV4::new(first.permit.clone()).into(),
            )]
            .into(),
        ))
        .sign(canary_key.private_key());
        let batch_error = executor
            .execute_transaction(&mut transaction, &authority, batch, &mut ivm_cache)
            .expect_err("a top-level Batch must receive no canary wire capability");
        assert!(
            batch_error
                .to_string()
                .contains("canary_authorization_missing")
        );

        executor
            .execute_transaction(
                &mut transaction,
                &authority,
                first.canary_transaction,
                &mut ivm_cache,
            )
            .expect("the automatically seeded exact signed wire must consume the canary");
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);
    }

    #[test]
    fn taira_canary_nested_trigger_cannot_inherit_outer_wire() {
        offline_test_transaction!(transaction);
        let outer = canary_consensus_fixture(&transaction, 23);
        let controller = canary_consensus_controller();
        let mut nested_permit = outer.permit.clone();
        nested_permit.body.binding.promotion_id = [0xD7; 32];
        nested_permit.signature =
            SignatureOf::try_from_hash(controller.private_key(), nested_permit.body.signing_hash())
                .expect("controller signs the nested promotion permit");

        commit_canary_activation_binding(&outer.permit.body.binding, &mut transaction);
        commit_canary_activation_binding(&nested_permit.body.binding, &mut transaction);
        AuthorizeKagemushaTairaCanaryV4::new(outer.reservation.clone())
            .execute(&ALICE_ID, &mut transaction)
            .expect("authorize the outer exact canary");
        let nested_body = KagemushaV4TairaCanaryReservationBodyV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_BODY_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            permit: nested_permit.clone(),
            canary_transaction_intent: HashOf::from_untyped_unchecked(outer.exact_call_hash),
            canary_transaction_wire: outer.reservation.body.canary_transaction_wire,
            canary_entrypoint_hash: outer.exact_call_hash,
        };
        let nested_reservation = KagemushaV4TairaCanaryReservationV1 {
            schema: KAGEMUSHA_V4_TAIRA_CANARY_RESERVATION_SCHEMA.to_owned(),
            version: KAGEMUSHA_V4_PROMOTION_RECEIPT_VERSION,
            signature: SignatureOf::try_from_hash(
                controller.private_key(),
                nested_body.signing_hash(),
            )
            .expect("controller signs the hostile inherited-wire reservation"),
            body: nested_body,
        };
        AuthorizeKagemushaTairaCanaryV4::new(nested_reservation)
            .execute(&ALICE_ID, &mut transaction)
            .expect("the hostile tuple is structurally controller-authorized");
        let trigger_id: TriggerId = "kagemusha_nested_canary".parse().expect("valid trigger id");
        let action = Action::new(
            vec![InstructionBox::from(RecordKagemushaTairaCanaryV4::new(
                nested_permit,
            ))],
            Repeats::Exactly(1),
            ALICE_ID.clone(),
            ExecuteTriggerEventFilter::new().for_trigger(trigger_id.clone()),
        )
        .expect("nested canary trigger action is valid");
        Register::trigger(Trigger::new(trigger_id.clone(), action))
            .execute(&ALICE_ID, &mut transaction)
            .expect("register the hostile nested canary trigger");

        transaction.tx_call_hash = Some(outer.exact_call_hash);
        bind_canary_consensus_wire(&outer, &mut transaction);
        RecordKagemushaTairaCanaryV4::new(outer.permit)
            .execute(&ALICE_ID, &mut transaction)
            .expect("the outer direct canary consumes its affine wire capability");
        assert_eq!(transaction.kagemusha_taira_canary_wire_identity, None);
        let markers_before_nested = transaction.world.kagemusha_replay_keys.iter().count();

        let error = ExecuteTrigger::new(trigger_id)
            .execute(&ALICE_ID, &mut transaction)
            .expect_err("a dynamically emitted canary must remain signed-wire unbound");
        assert!(error.to_string().contains("canary_authorization_missing"));
        assert_eq!(
            transaction.world.kagemusha_replay_keys.iter().count(),
            markers_before_nested,
            "the nested rejection must not consume the second promotion",
        );
    }
}
