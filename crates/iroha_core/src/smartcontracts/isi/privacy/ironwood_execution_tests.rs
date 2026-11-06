// Real native Ironwood proofs exercise the activated Core handler and its effects.
// The runtime owner separately verifies the governed selection transition; this component fixture
// installs that exact selection and enters the ordinary signed-source binding/admission boundary.
use super::*;
use crate::privacy_engines::orchard::{
    IronwoodOutputProverInputV1, OrchardSpendProverInputV1, authorize_ironwood_bundle_v1,
    orchard_authorization_wire_size_v1, orchard_empty_root_v1, prepare_ironwood_bundle_v1_with_rng,
};
use incrementalmerkletree::{Hashable as _, Level};
use iroha_data_model::{
    asset::{AssetBalancePolicy, AssetBalanceScope, AssetId},
    isi::{Mint, privacy::BootstrapPrivacyIronwoodPoolV1},
    privacy::{
        OrchardHalo2ActionsStatementV1, PrivacyIronwoodPoolBootstrapV1,
        PrivacyNativeConsensusBindingV1, PrivacyOrchardActionV1, PrivacyValueBalanceV1,
    },
};
use iroha_test_samples::BOB_ID;
use orchard::{
    Action, Anchor, Bundle,
    bundle::{BundleVersion, EffectsOnly, Flags},
    keys::{FullViewingKey, Scope, SpendingKey},
    note::{ExtractedNoteCommitment, Note, Nullifier, TransmittedNoteCiphertext},
    primitives::redpallas::{SpendAuth, VerificationKey},
    tree::{MerkleHashOrchard, MerklePath},
    value::ValueCommitment,
};

fn bootstrap() -> PrivacyIronwoodPoolBootstrapV1 {
    PrivacyIronwoodPoolBootstrapV1::new(
        PrivacyPoolIdV1::new([0xD2; 32]),
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("privacy", "universal").unwrap(),
            "ironwood_core".parse().unwrap(),
        ),
        AssetBalanceScope::Global,
        BOB_ID.clone(),
    )
    .unwrap()
}

fn state(activated: bool) -> State {
    let bootstrap = bootstrap();
    let domain = Domain::new(DomainId::try_new("privacy", "universal").unwrap()).build(&ALICE_ID);
    let definition = AssetDefinition::numeric(
        bootstrap.asset_definition_id,
        "Ironwood Core fixture",
        AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let mut world = World::with(
        [domain],
        [
            Account::new(ALICE_ID.clone()).build(&ALICE_ID),
            Account::new(BOB_ID.clone()).build(&ALICE_ID),
        ],
        [definition],
    );
    if activated {
        crate::runtime::ironwood::install_for_test(&mut world);
    }
    let mut state = State::new_with_chain_and_network_id_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
        TEST_CHAIN_ID.into(),
        test_network_id(),
    );
    // World::with supplies a genesis asset definition. Finalize that original empty-parent
    // boundary before installing the synthetic privacy genesis hash, so later fixture commits
    // retain the canonical incarnation instead of presenting an unregistered post-genesis asset.
    state
        .block(BlockHeader::new(
            NonZeroU64::new(1).unwrap(),
            None,
            None,
            0,
            0,
        ))
        .commit_world_overlay_for_testing()
        .expect("finalize the fixture genesis asset incarnation");
    install_synthetic_privacy_genesis_hash(&mut state, TEST_GENESIS_HASH);
    state
}

fn spending_key(seed: u8) -> SpendingKey {
    Option::from(SpendingKey::from_bytes([seed; 32])).unwrap()
}

fn decrypt_notes(instruction: &SubmitPrivacyIronwoodProofV1, seed: u8) -> Vec<Note> {
    let statement = &instruction.envelope.statement;
    let mut actions = statement
        .actions
        .iter()
        .map(|action| {
            Action::from_parts(
                Option::<Nullifier>::from(Nullifier::from_bytes(&action.nullifier)).unwrap(),
                VerificationKey::<SpendAuth>::try_from(action.randomized_key).unwrap(),
                Option::<ExtractedNoteCommitment>::from(ExtractedNoteCommitment::from_bytes(
                    &action.note_commitment,
                ))
                .unwrap(),
                TransmittedNoteCiphertext {
                    epk_bytes: action.ephemeral_key,
                    enc_ciphertext: orchard::note_encryption::NoteBytesData(
                        action.encrypted_note.clone().try_into().unwrap(),
                    ),
                    out_ciphertext: action.outgoing_ciphertext.clone().try_into().unwrap(),
                },
                Option::<ValueCommitment>::from(ValueCommitment::from_bytes(
                    &action.value_commitment,
                ))
                .unwrap(),
                (),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let first = actions.remove(0);
    let value_balance = match statement.value_balance.direction {
        PrivacyValueBalanceDirectionV1::Balanced => 0_i64,
        PrivacyValueBalanceDirectionV1::IntoPool => -(statement.value_balance.amount as i64),
        PrivacyValueBalanceDirectionV1::OutOfPool => statement.value_balance.amount as i64,
    };
    let bundle = Bundle::from_parts(
        (first, actions).into(),
        Flags::ENABLED,
        value_balance,
        Option::<Anchor>::from(Anchor::from_bytes(*statement.anchor.as_bytes())).unwrap(),
        EffectsOnly,
        BundleVersion::ironwood_v3(),
    )
    .unwrap();
    let viewer = FullViewingKey::from(&spending_key(seed));
    bundle
        .decrypt_outputs_with_keys(&[viewer.to_ivk(Scope::External)])
        .into_iter()
        .map(|(_, _, note, address, _)| {
            assert_eq!(address, viewer.address_at(0_u32, Scope::External));
            note
        })
        .collect()
}

fn funded_spend() -> (OrchardSpendProverInputV1, [u8; 32]) {
    let funding = cached_proof(Operation::PayerFunding);
    let mut notes = decrypt_notes(&funding, 0x61);
    assert_eq!(notes.len(), 1);
    let note = notes.remove(0);
    assert_eq!(note.value().inner(), 46);
    let commitment = ExtractedNoteCommitment::from(note.commitment());
    let path = MerklePath::from_parts(
        0,
        core::array::from_fn(|level| MerkleHashOrchard::empty_root(Level::from(level as u8))),
    );
    let anchor = path.root(commitment).to_bytes();
    (
        OrchardSpendProverInputV1::new(spending_key(0x61), note, path),
        anchor,
    )
}

#[derive(Clone, Copy)]
enum Operation {
    RecipientDeposit,
    PayerFunding,
    MerchantPayment,
}

fn proof(balanced: bool) -> SubmitPrivacyIronwoodProofV1 {
    cached_proof(if balanced {
        Operation::MerchantPayment
    } else {
        Operation::RecipientDeposit
    })
}

fn cached_proof(operation: Operation) -> SubmitPrivacyIronwoodProofV1 {
    static DEPOSIT: OnceLock<SubmitPrivacyIronwoodProofV1> = OnceLock::new();
    static FUNDING: OnceLock<SubmitPrivacyIronwoodProofV1> = OnceLock::new();
    static PAYMENT: OnceLock<SubmitPrivacyIronwoodProofV1> = OnceLock::new();
    let balanced = matches!(operation, Operation::MerchantPayment);
    let funding = matches!(operation, Operation::PayerFunding);
    (match operation {
        Operation::RecipientDeposit => &DEPOSIT,
        Operation::PayerFunding => &FUNDING,
        Operation::MerchantPayment => &PAYMENT,
    })
    .get_or_init(|| {
        let bootstrap = bootstrap();
        let merchant = FullViewingKey::from(&spending_key(0x62)).address_at(0_u32, Scope::External);
        let payer = FullViewingKey::from(&spending_key(0x61)).address_at(0_u32, Scope::External);
        assert_ne!(merchant, payer);
        let mut outputs = vec![IronwoodOutputProverInputV1::new(
            None,
            if funding { payer } else { merchant },
            if funding { 46 } else { 17 },
            &[if funding { 0x61 } else { 0x62 }; 512],
        )];
        let (spends, anchor, anchor_epoch, value_balance) = if balanced {
            let (spend, anchor) = funded_spend();
            outputs.push(IronwoodOutputProverInputV1::new(
                None,
                payer,
                29,
                &[0x61; 512],
            ));
            (vec![spend], anchor, 2, PrivacyValueBalanceV1::balanced())
        } else {
            (
                vec![],
                orchard_empty_root_v1(),
                1,
                PrivacyValueBalanceV1 {
                    direction: PrivacyValueBalanceDirectionV1::IntoPool,
                    amount: if funding { 46 } else { 17 },
                },
            )
        };
        let prepared = prepare_ironwood_bundle_v1_with_rng(
            anchor,
            spends,
            outputs,
            1,
            &mut KatRng::new(
                [if balanced {
                    0xD4
                } else if funding {
                    0xD7
                } else {
                    0xD3
                }; 32],
            ),
        )
        .unwrap();
        let profile =
            compiled_privacy_profile_v1(PrivacyRuntimeProtocolIdV1::IronwoodHalo2ActionsV1)
                .unwrap();
        let context = PrivacyStatementContextV1 {
            network_id: test_network_id(),
            action_index: 0,
            transaction_intent_digest: PrivacyTransactionIntentDigestV1::new([0xD5; 32]),
            parameter_id: profile.parameter_id,
            parameter_digest: profile.parameter_digest,
            verifier_digest: profile.verifier_digest,
            statement_schema_digest: profile.statement_schema_digest,
            engine_manifest_digest: profile.engine_manifest_digest,
        };
        let mut statement =
            PrivacyRuntimeStatementV1::IronwoodHalo2ActionsV1(OrchardHalo2ActionsStatementV1 {
                context,
                asset_definition_id: bootstrap.asset_definition_id,
                public_balance_scope: bootstrap.public_balance_scope,
                pool_id: bootstrap.pool_id,
                anchor: PrivacyRootV1::new(anchor),
                anchor_epoch,
                actions: prepared
                    .public_draft()
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
                value_balance,
                expiry_height: 100,
            });
        // Match the exact transaction built by bind_submit_privacy_instruction, with only
        // the normalized intent and authorization slots omitted by the canonical projection.
        let draft = PrivacyRuntimeProofEnvelopeV1::bind_v1(
            statement.clone(),
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(PrivacyProofBytesV1::new(vec![
                1;
                orchard_authorization_wire_size_v1(prepared.public_draft().actions.len()).unwrap()
            ])),
        )
        .unwrap();
        let intent = canonical_intent(draft);
        statement.context_mut().transaction_intent_digest = intent;
        let limits = PrivacyConsensusLimitsV1::taira_default();
        let binding =
            PrivacyNativeConsensusBindingV1::new(statement.context(), TEST_GENESIS_HASH, &limits)
                .unwrap();
        let proved = authorize_ironwood_bundle_v1(prepared, &binding, &limits).unwrap();
        let envelope = PrivacyRuntimeProofEnvelopeV1::bind_v1(
            statement,
            PrivacyRuntimeProofV1::IronwoodHalo2ActionsV1(PrivacyProofBytesV1::new(
                proved.authorization,
            )),
        )
        .unwrap();
        assert_eq!(canonical_intent(envelope.clone()), intent);
        SubmitPrivacyIronwoodProofV1::new(envelope.try_into().expect("dedicated Ironwood envelope"))
    })
    .clone()
}

fn canonical_intent(envelope: PrivacyRuntimeProofEnvelopeV1) -> PrivacyTransactionIntentDigestV1 {
    let mut builder = iroha_data_model::transaction::TransactionBuilder::new(
        test_network_id(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([SubmitPrivacyIronwoodProofV1::new(
        envelope.try_into().expect("dedicated Ironwood envelope"),
    )]);
    builder.set_creation_time(core::time::Duration::from_millis(1));
    builder
        .into_payload()
        .unwrap()
        .privacy_transaction_intent_digest_v1()
        .unwrap()
}

fn signed_submission_hash(instruction: &SubmitPrivacyIronwoodProofV1) -> iroha_crypto::Hash {
    // Match the exact source retained by bind_submit_privacy_instruction. The finite component
    // producer owns this signed invocation before the disposable transaction is opened; setting
    // tx_call_hash alone intentionally cannot authorize a FASTPQ-visible reserve transfer.
    let mut builder = iroha_data_model::transaction::TransactionBuilder::new(
        test_network_id(),
        ALICE_ID.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction.clone()]);
    builder.set_creation_time(core::time::Duration::from_millis(1));
    let signed = builder.sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
    iroha_crypto::Hash::from(signed.hash_as_entrypoint())
}

fn initialize(block: &mut crate::state::StateBlock<'_>, funded: bool) {
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        BootstrapPrivacyIronwoodPoolV1::new(bootstrap())
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        Mint::asset_quantity(
            100_u32,
            AssetId::new(bootstrap().asset_definition_id, ALICE_ID.clone()),
        )
        .execute(&ALICE_ID, &mut transaction)
        .unwrap();
        transaction.apply();
    }
    if funded {
        let instruction = cached_proof(Operation::PayerFunding);
        let source = signed_submission_hash(&instruction);
        let mut transaction = block.transaction_for_fastpq_testing(source);
        bind_submit_privacy_instruction(&mut transaction, &instruction);
        assert_eq!(transaction.tx_call_hash, Some(source));
        instruction
            .execute(&ALICE_ID, &mut transaction)
            .expect("real public deposit funds the spent note");
        transaction.apply();
    }
}

fn funded_state() -> State {
    let state = state(true);
    let mut block = state.block(test_header());
    initialize(&mut block, true);
    // Bootstrap and funding consume the predecessor-sized block budget. Publish only this
    // component fixture's World overlay, then exercise spending in its own ordinary block view.
    block
        .commit_world_overlay_for_testing()
        .expect("commit the funded Ironwood fixture World");
    state
}

fn balance(transaction: &StateTransaction<'_, '_>, account: &AccountId) -> Quantity {
    let id = AssetId::new(bootstrap().asset_definition_id, account.clone());
    transaction
        .world
        .assets
        .get(&id)
        .map(|value| (**value).clone())
        .unwrap_or_default()
}

#[test]
fn ironwood_baseline_refuses_bootstrap_and_real_proof_without_privacy_effects() {
    let state = state(false);
    let mut block = state.block(test_header());
    let mut transaction = block.transaction();
    grant_governance(&mut transaction);
    let error = BootstrapPrivacyIronwoodPoolV1::new(bootstrap())
        .execute(&ALICE_ID, &mut transaction)
        .unwrap_err();
    let message = smart_contract_parameter_message(&error);
    assert!(message.contains("Ironwood protocol storage is inactive"));
    assert!(message.contains(BootstrapPrivacyIronwoodPoolV1::WIRE_ID));
    let instruction = proof(false);
    bind_submit_privacy_instruction(&mut transaction, &instruction);
    assert!(instruction.execute(&ALICE_ID, &mut transaction).is_err());
    assert_empty_and_unbudgeted(&transaction);
}

#[test]
fn ironwood_real_recipient_deposit_applies_and_rolls_back_all_effects() {
    let state = state(true);
    let mut block = state.block(test_header());
    initialize(&mut block, false);
    let key = PrivacyCommitmentKeyV1::orchard_pool_state(bootstrap().namespace()).unwrap();
    let instruction = proof(false);
    let source = signed_submission_hash(&instruction);
    let mut transaction = block.transaction_for_fastpq_testing(source);
    let before = proof_managed_state_snapshot(&transaction, key);
    bind_submit_privacy_instruction(&mut transaction, &instruction);
    assert_eq!(transaction.tx_call_hash, Some(source));
    instruction
        .execute(&ALICE_ID, &mut transaction)
        .expect("real Ironwood recipient deposit applies");
    assert_eq!(balance(&transaction, &ALICE_ID), Quantity::from(83_u32));
    assert_eq!(balance(&transaction, &BOB_ID), Quantity::from(17_u32));
    assert_eq!(transaction.world.privacy_nullifiers.iter().count(), 1);
    assert_eq!(transaction.world.privacy_roots.iter().count(), 2);
    let after = proof_managed_state_snapshot(&transaction, key);
    assert_ne!(before, after);
    drop(transaction);
    let transaction = block.transaction();
    assert_eq!(proof_managed_state_snapshot(&transaction, key), before);
    assert_eq!(balance(&transaction, &ALICE_ID), Quantity::from(100_u32));
    assert_eq!(balance(&transaction, &BOB_ID), Quantity::zero());
}

#[test]
fn ironwood_real_foreign_recipient_payment_advances_pool_and_rejects_replay() {
    let state = funded_state();
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 1));
    let instruction = proof(true);
    let merchant_notes = decrypt_notes(&instruction, 0x62);
    let payer_notes = decrypt_notes(&instruction, 0x61);
    assert_eq!(merchant_notes.len(), 1);
    assert_eq!(merchant_notes[0].value().inner(), 17);
    assert_eq!(payer_notes.len(), 1);
    assert_eq!(payer_notes[0].value().inner(), 29);
    let namespace = bootstrap().namespace();
    let key = PrivacyCommitmentKeyV1::orchard_pool_state(namespace).unwrap();
    {
        let source = signed_submission_hash(&instruction);
        let mut transaction = block.transaction_for_fastpq_testing(source);
        bind_submit_privacy_instruction(&mut transaction, &instruction);
        assert_eq!(transaction.tx_call_hash, Some(source));
        instruction
            .clone()
            .execute(&ALICE_ID, &mut transaction)
            .expect("real cross-address payment applies");
        assert_eq!(balance(&transaction, &ALICE_ID), Quantity::from(54_u32));
        assert_eq!(balance(&transaction, &BOB_ID), Quantity::from(46_u32));
        assert_eq!(transaction.world.privacy_nullifiers.iter().count(), 3);
        let head = transaction
            .world
            .privacy_root_heads
            .get(
                &PrivacyRootHeadKeyV1::new(namespace, PrivacyRootRoleV1::NoteCommitmentAnchor)
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(head.epoch(), 3);
        assert_eq!(head.retained_root_count(), 3);
        transaction.apply();
    }
    // The two-action payment fills this block's limit. Replay must reach the persisted
    // nullifier check in a fresh block, without a prior budget refusal masking it.
    block
        .commit_world_overlay_for_testing()
        .expect("commit the Ironwood payment fixture World");
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 2));
    let mut transaction = block.transaction();
    let before = proof_managed_state_snapshot(&transaction, key);
    bind_submit_privacy_instruction(&mut transaction, &instruction);
    let error = instruction
        .execute(&ALICE_ID, &mut transaction)
        .unwrap_err();
    assert!(smart_contract_parameter_message(&error).contains("nullifier was already consumed"));
    assert_eq!(proof_managed_state_snapshot(&transaction, key), before);
}

#[test]
fn ironwood_scope_validation_precedes_native_proof_verification_and_writes() {
    let state = funded_state();
    let mut block = state.block(test_header_at(TEST_BLOCK_HEIGHT + 1));
    let mut transaction = block.transaction();
    let id = bootstrap().asset_definition_id;
    transaction.world.asset_definitions.insert(
        id.clone(),
        AssetDefinition::numeric(
            id,
            "Restricted scope negative control",
            AssetBalancePolicy::DataspaceRestricted,
            None,
        )
        .build(&ALICE_ID),
    );
    let key = PrivacyCommitmentKeyV1::orchard_pool_state(bootstrap().namespace()).unwrap();
    let before = proof_managed_state_snapshot(&transaction, key);
    let mut instruction = proof(true);
    instruction.envelope.proof = PrivacyProofBytesV1::new(vec![1; 7461]);
    bind_submit_privacy_instruction(&mut transaction, &instruction);
    let error = instruction
        .execute(&ALICE_ID, &mut transaction)
        .unwrap_err();
    let Error::InvariantViolation(message) = error else {
        panic!("unexpected scope rejection: {error:?}")
    };
    assert_eq!(
        message.as_ref(),
        "dataspace-restricted asset definition requires an exact public balance scope"
    );
    assert_eq!(proof_managed_state_snapshot(&transaction, key), before);
}

#[test]
fn ironwood_real_proof_cannot_use_an_orchard_pool_with_the_same_pool_id() {
    let state = state(true);
    let mut block = state.block(test_header());
    let new = bootstrap();
    let old = iroha_data_model::privacy::PrivacyOrchardPoolBootstrapV1::new(
        new.pool_id,
        new.asset_definition_id,
        new.public_balance_scope,
        new.reserve_account,
    )
    .unwrap();
    let key = PrivacyCommitmentKeyV1::orchard_pool_state(old.namespace()).unwrap();
    {
        let mut transaction = block.transaction();
        grant_governance(&mut transaction);
        BootstrapPrivacyOrchardPoolV1::new(old)
            .execute(&ALICE_ID, &mut transaction)
            .unwrap();
        transaction.apply();
    }
    let mut transaction = block.transaction();
    let before = proof_managed_state_snapshot(&transaction, key);
    let instruction = proof(false);
    bind_submit_privacy_instruction(&mut transaction, &instruction);
    let error = instruction
        .execute(&ALICE_ID, &mut transaction)
        .unwrap_err();
    let Error::InvariantViolation(message) = error else {
        panic!("unexpected cross-profile pool rejection: {error:?}")
    };
    assert!(message.starts_with("trusted Orchard pool state failed validation"));
    assert_eq!(proof_managed_state_snapshot(&transaction, key), before);
}
