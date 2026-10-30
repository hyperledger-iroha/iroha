/// Real signed mixed-batch execution with native Orchard custody, effects and rollback.
mod privacy_occurrence_executor {
    use super::*;
    use crate::{
        privacy_engines::orchard::{
            OrchardChangeProverInputV1, Scope, SpendingKey, authorize_orchard_bundle_v1,
            orchard_empty_root_v1, prepare_orchard_bundle_v1_with_rng,
        },
        privacy_profiles::compiled_privacy_profile_v1,
        state::seed_committed_transaction_context,
    };
    use iroha_data_model::{
        isi::{
            error::InvalidParameterError,
            privacy::{BootstrapPrivacyOrchardPoolV1, SubmitPrivacyProofV1},
        },
        privacy::{
            OrchardHalo2ActionsStatementV1, PrivacyConsensusLimitsV1,
            PrivacyNativeConsensusBindingV1, PrivacyOrchardActionV1, PrivacyOrchardPoolBootstrapV1,
            PrivacyPoolIdV1, PrivacyProofBytesV1, PrivacyProofEnvelopeV1, PrivacyProofV1,
            PrivacyProtocolIdV1, PrivacyRootV1, PrivacyStatementContextV1, PrivacyStatementV1,
            PrivacyTransactionIntentDigestV1, PrivacyValueBalanceDirectionV1,
            PrivacyValueBalanceV1,
        },
        transaction::{TransactionEntrypoint, TransactionPayload},
    };
    use rand::{SeedableRng as _, rngs::StdRng};
    use std::time::Duration;

    const GAS: u64 = 50_000_000;
    const BEFORE: &str = "privacy_occurrence_before";
    const AFTER: &str = "privacy_occurrence_after";

    fn contract_fixture() -> (State, ContractInvocation, ContractInvocation) {
        let manifest_signing = crate::manifest_signing_test_support::ManifestSigningFixture::new();
        let (program, manifest) = kotodama_lang::compiler::Compiler::new()
            .compile_source_with_manifest(
                r#"
seiyaku PrivacyOccurrenceGuard { permission CanInvokeContractEntrypoint;
  kotoage fn open() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("privacy_occurrence_before"),
      value: Json::parse("true")
    );
  }
  kotoage fn close() authorize(CanInvokeContractEntrypoint) {
    ledger::account::set_metadata(
      account: context::authority(),
      key: Name::parse("privacy_occurrence_after"),
      value: Json::parse("true")
    );
  }
}
"#,
            )
            .expect("compile real privacy occurrence guard");
        let authority = ALICE_ID.clone();
        let domain =
            Domain::new(DomainId::try_new("wonderland", "universal").unwrap()).build(&authority);
        let asset_definition_id = occurrence_asset_definition();
        let mut world = World::with_assets(
            [domain],
            [
                Account::new(authority.clone()).build(&authority),
                Account::new(BOB_ID.clone()).build(&authority),
            ],
            [AssetDefinition::numeric(
                asset_definition_id.clone(),
                "Orchard occurrence asset".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
            .build(&authority)],
            [Asset::new(
                AssetId::new(asset_definition_id.clone(), authority.clone()),
                Quantity::from(100_u32),
            )],
            [],
        );
        seed_test_asset_supply(&mut world, &asset_definition_id);
        let bootstrap = PrivacyOrchardPoolBootstrapV1::new(
            PrivacyPoolIdV1::new([0x64; 32]),
            asset_definition_id,
            iroha_data_model::asset::AssetBalanceScope::Global,
            BOB_ID.clone(),
        )
        .expect("canonical Orchard bootstrap");
        // The original signed genesis invokes the real typed bootstrap. Its current root,
        // reserve custody and compact frontier are retained by the State used below.
        let state = state_after_genesis_with_instructions(world, |_| {
            vec![BootstrapPrivacyOrchardPoolV1::new(bootstrap).into()]
        });
        let code_hash = ivm::contract_code_hash(&program);
        let contract_address =
            ContractAddress::derive(&state.network_id, &authority, 94, DataSpaceId::UNIVERSAL)
                .expect("derive contract address");
        let artifact = iroha_data_model::smart_contract::ContractArtifactId::new(
            DataSpaceId::UNIVERSAL,
            code_hash,
        );
        let mut world = state.world.block();
        world.contract_code.insert(artifact, program);
        world.contract_manifests.insert(
            artifact,
            manifest
                .try_signed(
                    manifest_signing.context(),
                    manifest_signing.max_frame_bytes(),
                    &ALICE_KEYPAIR,
                )
                .expect("sign bounded guard manifest"),
        );
        world.account_permissions.insert(
            contract_address.subject_id(),
            BTreeSet::from([executor_permission::account::CanModifyAccountMetadata {
                account: authority.clone(),
            }
            .into()]),
        );
        world.account_permissions.insert(
            authority.clone(),
            BTreeSet::from([
                iroha_executor_data_model::permission::smart_contract::CanUseContractPermission {
                    contract: contract_address.clone(),
                    permission: "CanInvokeContractEntrypoint".parse().unwrap(),
                }
                .into(),
            ]),
        );
        assert_eq!(world.privacy_protocol_policies.iter().count(), 0);
        assert_eq!(world.privacy_roots.iter().count(), 1);
        assert_eq!(world.privacy_root_heads.iter().count(), 1);
        assert_eq!(world.privacy_commitments.iter().count(), 2);
        world.commit();
        bind_executor_test_contract_after_genesis(&state, &contract_address, &authority, code_hash);
        let call = |entrypoint: &str| ContractInvocation {
            contract_address: contract_address.clone(),
            expected_code_hash: code_hash,
            entrypoint: entrypoint.to_owned(),
            arguments: None,
        };
        (state, call("open"), call("close"))
    }

    fn mixed_payload(
        state: &State,
        open: &ContractInvocation,
        close: &ContractInvocation,
        envelope: PrivacyProofEnvelopeV1,
    ) -> TransactionPayload {
        let mut builder = TransactionBuilder::new(
            state.network_id,
            ALICE_ID.clone(),
            FeePaymentIntent::authority(Vec::new(), core::num::NonZeroU64::new(GAS)),
        )
        .with_executable(Executable::Batch(
            vec![
                ExecutableBatchItem::ContractCall(open.clone()),
                ExecutableBatchItem::Instruction(SubmitPrivacyProofV1::new(envelope).into()),
                ExecutableBatchItem::ContractCall(close.clone()),
            ]
            .into(),
        ));
        // Both phases use exactly this time, fee intent, metadata and complete call sequence.
        builder.set_creation_time(Duration::from_millis(1));
        builder.into_payload().expect("canonical mixed payload")
    }

    fn real_orchard_mixed_transactions(
        state: &State,
        open: &ContractInvocation,
        close: &ContractInvocation,
    ) -> (SignedTransaction, SignedTransaction, u64, usize) {
        let profile = compiled_privacy_profile_v1(PrivacyProtocolIdV1::OrchardHalo2ActionsV1)
            .expect("compiled production Orchard profile");
        // Deterministic native witness/entropy belong only to this test; neither is exported.
        let spending_key = (1_u8..=u8::MAX)
            .find_map(|counter| {
                let mut bytes = [0x61; 32];
                bytes[0] = counter;
                Option::<SpendingKey>::from(SpendingKey::from_bytes(bytes))
            })
            .expect("fixture spending key");
        let mut rng = StdRng::from_seed([0x67; 32]);
        let prepared = prepare_orchard_bundle_v1_with_rng(
            orchard_empty_root_v1(),
            Vec::new(),
            vec![OrchardChangeProverInputV1::new(
                spending_key,
                Scope::External,
                1,
                17,
                &[0x63; 512],
            )],
            1,
            &mut rng,
        )
        .expect("produce a real Post-NU6.3 Halo2 proof");
        let draft = prepared.public_draft();
        assert_eq!(draft.value_balance, -17);
        let mut statement = OrchardHalo2ActionsStatementV1 {
            context: PrivacyStatementContextV1 {
                network_id: state.network_id,
                action_index: 0,
                transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0; 32]),
                parameter_id: profile.parameter_id,
                parameter_digest: profile.parameter_digest,
                verifier_digest: profile.verifier_digest,
                statement_schema_digest: profile.statement_schema_digest,
                engine_manifest_digest: profile.engine_manifest_digest,
            },
            asset_definition_id: occurrence_asset_definition(),
            public_balance_scope: iroha_data_model::asset::AssetBalanceScope::Global,
            pool_id: PrivacyPoolIdV1::new([0x64; 32]),
            anchor: PrivacyRootV1::new(draft.anchor),
            anchor_epoch: 1,
            actions: draft
                .actions
                .iter()
                .map(|action| PrivacyOrchardActionV1 {
                    nullifier: action.nullifier,
                    randomized_key: action.randomized_key,
                    note_commitment: action.note_commitment,
                    ephemeral_key: action.ephemeral_key,
                    encrypted_note: action.encrypted_note.to_vec(),
                    outgoing_ciphertext: action.outgoing_ciphertext.to_vec(),
                    value_commitment: action.value_commitment,
                })
                .collect(),
            value_balance: PrivacyValueBalanceV1 {
                direction: PrivacyValueBalanceDirectionV1::IntoPool,
                amount: 17,
            },
            expiry_height: 100,
        };
        // Empty proof bytes are only the native builder's unsigned draft. They are replaced
        // before signing or execution and are never submitted to any accepting verifier.
        let draft_envelope = PrivacyProofEnvelopeV1::bind_v1(
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement.clone()),
            PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(Vec::new())),
        )
        .expect("bind draft statement");
        let intent = mixed_payload(state, open, close, draft_envelope)
            .privacy_transaction_intent_digest_v1()
            .expect("derive complete signed batch intent");
        assert!(!intent.is_zero());
        statement.context.transaction_intent_digest = intent;
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding = PrivacyNativeConsensusBindingV1::new(
            &statement.context,
            *state.network_id.as_bytes(),
            &limits,
        )
        .expect("bind actual genesis security domain and complete batch");
        // Authorization consumes the prepared witness and independently invokes the production
        // Orchard verifier before returning.
        let proved = authorize_orchard_bundle_v1(prepared, &binding, &limits)
            .expect("authorize and independently verify the real native Orchard bundle");
        assert!(!proved.authorization.is_empty());
        assert_eq!(proved.public.anchor, *statement.anchor.as_bytes());
        assert_eq!(proved.public.actions.len(), statement.actions.len());
        let envelope = PrivacyProofEnvelopeV1::bind_v1(
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement),
            PrivacyProofV1::OrchardHalo2ActionsV1(PrivacyProofBytesV1::new(proved.authorization)),
        )
        .expect("bind final real proof");
        assert!(profile.matches_envelope_v1(&envelope));
        let action_count = match &envelope.statement {
            PrivacyStatementV1::OrchardHalo2ActionsV1(statement) => statement.actions.len(),
            _ => unreachable!("Orchard fixture"),
        };
        let encoded_bytes = u64::try_from(norito::canonical_frame_len(&envelope).unwrap()).unwrap();
        let mut malformed = envelope.clone();
        let proof = &mut malformed.proof.bytes_mut().bytes;
        let last = proof.len() - 1;
        proof[last] ^= 1;
        let sign = |envelope| {
            let payload = mixed_payload(state, open, close, envelope);
            assert_eq!(
                payload
                    .validate_privacy_transaction_intent_binding_v1()
                    .unwrap(),
                intent
            );
            let signed = TransactionBuilder::from_payload(payload)
                .expect("final fixed batch payload")
                .try_sign(ALICE_KEYPAIR.private_key())
                .expect("sign complete batch");
            signed.verify_signature().expect("verify actual signature");
            assert_eq!(
                signed
                    .privacy_transaction_intent_binding_if_present_v1()
                    .unwrap()
                    .unwrap()
                    .0,
                intent
            );
            signed
        };
        // Both payloads carry a real, correct transaction signature and their own immutable
        // original proof occurrence. The malformed one must reach native proof rejection.
        (sign(envelope), sign(malformed), encoded_bytes, action_count)
    }

    fn occurrence_asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "privacy_occurrence".parse().unwrap(),
        )
    }

    #[derive(std::fmt::Debug, PartialEq, Eq)]
    struct PrivacyEffectsSnapshot {
        roots: Vec<Vec<u8>>,
        heads: Vec<Vec<u8>>,
        nullifiers: Vec<Vec<u8>>,
        commitments: Vec<Vec<u8>>,
        source_balance: Quantity,
        reserve_balance: Quantity,
        budget: (u32, u64, u32, u64),
    }

    fn privacy_effects(transaction: &StateTransaction<'_, '_>) -> PrivacyEffectsSnapshot {
        PrivacyEffectsSnapshot {
            roots: transaction
                .world
                .privacy_roots
                .iter()
                .map(|(key, value)| norito::encode_canonical(&(*key, value.clone())).unwrap())
                .collect(),
            heads: transaction
                .world
                .privacy_root_heads
                .iter()
                .map(|(key, value)| norito::encode_canonical(&(*key, value.clone())).unwrap())
                .collect(),
            nullifiers: transaction
                .world
                .privacy_nullifiers
                .iter()
                .map(|(key, value)| norito::encode_canonical(&(*key, value.clone())).unwrap())
                .collect(),
            commitments: transaction
                .world
                .privacy_commitments
                .iter()
                .map(|(key, value)| norito::encode_canonical(&(*key, value.clone())).unwrap())
                .collect(),
            source_balance: initial_batch_balance(
                &transaction.world,
                &AssetId::new(occurrence_asset_definition(), ALICE_ID.clone()),
            ),
            reserve_balance: initial_batch_balance(
                &transaction.world,
                &AssetId::new(occurrence_asset_definition(), BOB_ID.clone()),
            ),
            budget: transaction.privacy_budget_for_testing(),
        }
    }

    fn assert_custody_cleared(
        transaction: &StateTransaction<'_, '_>,
        entrypoint: &TransactionEntrypoint,
    ) {
        assert!(transaction.privacy_transaction_intent_binding.is_none());
        assert_eq!(transaction.current_direct_privacy_instruction_index, None);
        assert_eq!(
            transaction.current_network_entrypoint_hash,
            Some(entrypoint.hash())
        );
    }

    #[test]
    fn signed_privacy_mixed_batch_commits_native_effects_and_malformed_proof_rolls_back() {
        let (state, open, close) = contract_fixture();
        let (signed, malformed, proof_bytes, action_count) =
            real_orchard_mixed_transactions(&state, &open, &close);
        let original_tip = state.view().latest_block_hash();
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), original_tip, None, 1, 0));
        let before: Name = BEFORE.parse().unwrap();
        let after: Name = AFTER.parse().unwrap();
        let baseline = privacy_effects(&block.transaction());
        assert_eq!(baseline.roots.len(), 1);
        assert_eq!(baseline.heads.len(), 1);
        assert_eq!(baseline.commitments.len(), 2);
        assert!(baseline.nullifiers.is_empty());
        assert_eq!(baseline.source_balance, Quantity::from(100_u32));
        assert_eq!(baseline.reserve_balance, Quantity::zero());
        assert_eq!(baseline.budget, (0, 0, 0, 0));
        {
            let entrypoint = TransactionEntrypoint::External(malformed.clone());
            let mut transaction =
                block.transaction_for_fastpq_testing(Hash::from(malformed.hash_as_entrypoint()));
            seed_committed_transaction_context(&mut transaction, &entrypoint, 0);
            transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            let error = crate::executor::Executor::Initial
                .execute_transaction(
                    &mut transaction,
                    &ALICE_ID,
                    &malformed,
                    &mut IvmCache::new(),
                )
                .map_err(crate::execution_attempt::expect_completed_rejection)
                .expect_err(
                    "the correctly signed malformed proof must reach native Orchard verification",
                );
            assert!(
                matches!(error,
                ValidationFail::InstructionFailed(
                    iroha_data_model::isi::error::InstructionExecutionError::InvalidParameter(
                        InvalidParameterError::SmartContract(ref message)
                    )
                ) if message.contains("native Orchard verification failed")),
                "unexpected refusal before native verification: {error:?}"
            );
            let account = transaction.world.account(&ALICE_ID).unwrap();
            assert_eq!(
                account.metadata().get(&before),
                Some(&Json::new(true)),
                "the first real contract call executed before proof rejection"
            );
            assert!(account.metadata().get(&after).is_none());
            assert_custody_cleared(&transaction, &entrypoint);
            assert_eq!(
                privacy_effects(&transaction),
                baseline,
                "native proof failure leaves privacy state, reserve balances and budgets unchanged"
            );
            // Dropping the real owning overlay also rolls back the earlier contract write.
        }
        {
            let verification = block.transaction();
            assert_eq!(privacy_effects(&verification), baseline);
            let account = verification.world.account(&ALICE_ID).unwrap();
            assert!(account.metadata().get(&before).is_none());
            assert!(account.metadata().get(&after).is_none());
        }
        {
            let entrypoint = TransactionEntrypoint::External(signed.clone());
            let mut transaction =
                block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
            seed_committed_transaction_context(&mut transaction, &entrypoint, 0);
            transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            assert_eq!(
                transaction.world.privacy_protocol_policies.iter().count(),
                0
            );
            crate::executor::Executor::Initial
                .execute_transaction(&mut transaction, &ALICE_ID, &signed, &mut IvmCache::new())
                .expect("the original signed occurrence executes between both contract calls");
            assert_custody_cleared(&transaction, &entrypoint);
            let account = transaction.world.account(&ALICE_ID).unwrap();
            assert_eq!(account.metadata().get(&before), Some(&Json::new(true)));
            assert_eq!(account.metadata().get(&after), Some(&Json::new(true)));
            let effects = privacy_effects(&transaction);
            assert_eq!(effects.roots.len(), 2);
            assert_eq!(effects.heads.len(), 1);
            assert_eq!(effects.nullifiers.len(), action_count);
            assert_eq!(effects.commitments.len(), 2);
            assert_ne!(effects.heads, baseline.heads);
            assert_ne!(effects.commitments, baseline.commitments);
            assert_eq!(effects.source_balance, Quantity::from(83_u32));
            assert_eq!(effects.reserve_balance, Quantity::from(17_u32));
            assert_eq!(effects.budget, (1, proof_bytes, 1, proof_bytes));
            let head = transaction
                .world
                .privacy_root_heads
                .iter()
                .next()
                .unwrap()
                .1;
            assert_eq!(head.epoch(), 2);
            assert_eq!(head.retained_root_count(), 2);
            assert_eq!(
                transaction
                    .world
                    .asset_definition(&occurrence_asset_definition())
                    .unwrap()
                    .total_quantity,
                Quantity::from(100_u32)
            );
            transaction.apply();
        }
        let committed = privacy_effects(&block.transaction());
        assert_eq!(committed.source_balance, Quantity::from(83_u32));
        assert_eq!(committed.reserve_balance, Quantity::from(17_u32));
        {
            let hash = Hash::from(signed.hash_as_entrypoint());
            let transaction = block.transaction_for_fastpq_testing(hash);
            assert!(
                transaction
                    .fastpq_rejection_tail_context(hash)
                    .expect_err("an applied source cannot be reused in the same block")
                    .contains("already has source occurrences")
            );
            assert_eq!(privacy_effects(&transaction), committed);
        }
        // Publish only this component's World effects. This does not synthesize
        // finalized transaction membership or advance the authenticated chain.
        // A fresh execution owner then reaches the native nullifier check;
        // reusing this block's already-consumed FASTPQ entry cannot do so.
        block
            .commit_world_overlay_for_testing()
            .expect("publish the isolated component World effects");
        assert_eq!(state.view().latest_block_hash(), original_tip);
        let mut block = state.block(BlockHeader::new(nonzero!(2_u64), original_tip, None, 1, 0));
        let mut expected = committed;
        expected.budget = (0, 0, 0, 0);
        let committed = privacy_effects(&block.transaction());
        assert_eq!(committed, expected, "only the execution budget is fresh");
        {
            let entrypoint = TransactionEntrypoint::External(signed.clone());
            let mut transaction =
                block.transaction_for_fastpq_testing(Hash::from(signed.hash_as_entrypoint()));
            seed_committed_transaction_context(&mut transaction, &entrypoint, 0);
            transaction.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            transaction.world.current_dataspace_id = Some(DataSpaceId::UNIVERSAL);
            let error = crate::executor::Executor::Initial
                .execute_transaction(&mut transaction, &ALICE_ID, &signed, &mut IvmCache::new())
                .map_err(crate::execution_attempt::expect_completed_rejection)
                .expect_err("the committed native nullifiers reject an exact signed replay");
            assert!(
                format!("{error:?}").contains("Orchard nullifier was already consumed"),
                "{error:?}"
            );
            assert_custody_cleared(&transaction, &entrypoint);
            assert_eq!(privacy_effects(&transaction), committed);
        }
        let verification = block.transaction();
        assert_eq!(privacy_effects(&verification), committed);
        let account = verification.world.account(&ALICE_ID).unwrap();
        assert_eq!(account.metadata().get(&before), Some(&Json::new(true)));
        assert_eq!(account.metadata().get(&after), Some(&Json::new(true)));
    }
}
