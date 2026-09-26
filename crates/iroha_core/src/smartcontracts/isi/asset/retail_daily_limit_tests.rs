use super::super::isi::{NumericAssetTransferSourcePolicy, prepare_retail_daily_usage_update};
use crate::state::retail_daily_limit_state as retail_state;
use iroha_data_model::asset::{
    RETAIL_IDENTITY_ATTESTATION_DOMAIN_V1, RetailDailyLimitPolicyV1, RetailDailyUsageKeyV1,
    RetailIdentityAttestationBodyV1, RetailIdentityAttestationV1, RetailIdentityCommitmentV1,
    RetailInstitutionalExceptionV1,
};
use iroha_data_model::isi::error::{AssetTransferAdmissionError, InstructionExecutionError};

fn retail_test_reserve_account() -> AccountId {
    let key = KeyPair::try_from_seed(vec![0xC2; 32], Algorithm::Ed25519)
        .expect("test-only reserve account key");
    AccountId::new(key.public_key().clone())
}

fn retail_cap_test_state() -> (State, AssetDefinitionId, AssetId, AssetId, AccountId) {
    let domain_id = wonderland_domain_id();
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let carol_key = KeyPair::try_from_seed(vec![0xC1; 32], Algorithm::Ed25519)
        .expect("test-only second retail account key");
    let carol = AccountId::new(carol_key.public_key().clone());
    let definition_id = wonderland_asset_definition_id("rose");
    let definition = AssetDefinition::new(
        definition_id.clone(),
        "rose".to_owned(),
        NumericSpec::fractional(2),
        AssetBalancePolicy::DataspaceRestricted,
        Some(domain_id.clone()),
    )
    .build(&ALICE_ID);
    let source = AssetId::with_scope(
        definition_id.clone(),
        ALICE_ID.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let alternate_source = AssetId::with_scope(
        definition_id.clone(),
        carol.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let world = World::with_assets(
        [domain],
        [
            build_account_in_domain(&ALICE_ID, &domain_id),
            build_account_in_domain(&BOB_ID, &domain_id),
            build_account_in_domain(&carol, &domain_id),
            build_account_in_domain(&retail_test_reserve_account(), &domain_id),
        ],
        [definition],
        [
            Asset::new(source.clone(), Quantity::from(10_u32)),
            Asset::new(alternate_source.clone(), Quantity::from(10_u32)),
        ],
        [],
    );
    (
        asset_route_test_state(world),
        definition_id,
        source,
        alternate_source,
        carol,
    )
}

fn seed_test_retail_policy(
    stx: &mut StateTransaction<'_, '_>,
    definition_id: &AssetDefinitionId,
    cap: u32,
    bindings: &[(AccountId, [u8; 32])],
) {
    seed_test_retail_policy_revision(stx, definition_id, cap, 1, bindings);
}

fn seed_test_retail_policy_revision(
    stx: &mut StateTransaction<'_, '_>,
    definition_id: &AssetDefinitionId,
    cap: u32,
    revision: u64,
    bindings: &[(AccountId, [u8; 32])],
) {
    let issuer = KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519)
        .expect("test-only identity issuer");
    let policy = RetailDailyLimitPolicyV1 {
        asset_definition_id: definition_id.clone(),
        physical_dataspace: DataSpaceId::new(7),
        revision,
        daily_cap: Quantity::from(cap),
        identity_issuer: ALICE_ID.clone(),
        identity_issuer_public_key: issuer.public_key().clone(),
        monetary_issuer_account: ALICE_ID.clone(),
        reserve_account: retail_test_reserve_account(),
        institutional_exceptions: BTreeSet::<RetailInstitutionalExceptionV1>::new(),
    };
    stx.world.smart_contract_state.insert(
        retail_state::policy_key(definition_id, DataSpaceId::new(7)),
        norito::encode_canonical(&policy).expect("test-only encoded retail policy"),
    );
    let activation =
        retail_state::activation_for_policy(&policy, 0).expect("test-only next UTC DAY marker");
    stx.world.smart_contract_state.insert(
        retail_state::activation_key(definition_id),
        norito::encode_canonical(&activation).expect("test-only encoded activation"),
    );
    for (account, digest) in bindings {
        let body = RetailIdentityAttestationBodyV1 {
            domain: RETAIL_IDENTITY_ATTESTATION_DOMAIN_V1.to_owned(),
            asset_definition_id: definition_id.clone(),
            physical_dataspace: policy.physical_dataspace,
            policy_revision: policy.revision,
            account_id: account.clone(),
            identity: RetailIdentityCommitmentV1 { digest: *digest },
            uniqueness_evidence_digest: [0xE1; 32],
        };
        let attestation = RetailIdentityAttestationV1 {
            signature: iroha_crypto::SignatureOf::try_new(issuer.private_key(), &body)
                .expect("test-only identity signature"),
            body,
        };
        stx.world.smart_contract_state.insert(
            retail_state::identity_key(definition_id, DataSpaceId::new(7), account),
            norito::encode_canonical(&attestation).expect("test-only encoded retail identity"),
        );
    }
}

fn assert_retail_cap_rejected(error: InstructionExecutionError) {
    match error {
        InstructionExecutionError::AssetTransferAdmission(
            AssetTransferAdmissionError::PolicyRejected(reason),
        ) => assert!(
            reason.contains("identity-wide retail DAY cap exceeded"),
            "unexpected retail policy reason: {reason}"
        ),
        other => panic!("unexpected retail cap error: {other:?}"),
    }
}

#[test]
fn retail_governed_debits_stay_closed_through_activation_day() {
    let (state, definition, source, _, _) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 1_000, 0));
    let mut stx = block.transaction();
    let destination = AssetId::with_scope(
        definition.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    seed_test_retail_policy(
        &mut stx,
        &definition,
        5,
        &[(ALICE_ID.clone(), [0xA1; 32]), (BOB_ID.clone(), [0xB1; 32])],
    );
    let error = prepare_retail_daily_usage_update(
        &stx,
        &source,
        &destination,
        &Quantity::one(),
        NumericAssetTransferSourcePolicy::User,
    )
    .expect_err("activation day cannot contain an uncounted debit");
    assert!(error.to_string().contains("until the next UTC day"));
    assert!(
        stx.world
            .smart_contract_state
            .iter()
            .all(|(path, _)| { !path.as_ref().starts_with(retail_state::USAGE_ROOT) })
    );
}

#[test]
fn retail_debit_rejects_unbound_source_and_receiver() {
    let (state, definition, source, _, _) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    let destination = AssetId::with_scope(
        definition.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    seed_test_retail_policy(&mut stx, &definition, 5, &[(BOB_ID.clone(), [0xB1; 32])]);
    let error = prepare_retail_daily_usage_update(
        &stx,
        &source,
        &destination,
        &Quantity::one(),
        NumericAssetTransferSourcePolicy::User,
    )
    .expect_err("unbound retail source must fail closed");
    assert!(
        error
            .to_string()
            .contains("no issuer-signed identity binding")
    );

    seed_test_retail_policy(&mut stx, &definition, 5, &[(ALICE_ID.clone(), [0xA1; 32])]);
    stx.world
        .smart_contract_state
        .remove(retail_state::identity_key(
            &definition,
            DataSpaceId::new(7),
            &BOB_ID,
        ));
    let error = prepare_retail_daily_usage_update(
        &stx,
        &source,
        &destination,
        &Quantity::one(),
        NumericAssetTransferSourcePolicy::User,
    )
    .expect_err("unbound retail receiver must fail closed");
    assert!(
        error
            .to_string()
            .contains("no issuer-signed identity binding")
    );
}

#[test]
fn retail_day_usage_is_shared_across_two_source_accounts() {
    let (state, definition, source, alternate_source, carol) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::new(7));
    stx.world.current_dataspace_id = Some(DataSpaceId::new(7));
    seed_test_call_hash(&mut stx, 0xE2);
    seed_test_retail_policy(
        &mut stx,
        &definition,
        5,
        &[
            (ALICE_ID.clone(), [0xA1; 32]),
            (carol.clone(), [0xA1; 32]),
            (BOB_ID.clone(), [0xB1; 32]),
        ],
    );
    Transfer::asset_quantity(source.clone(), 3_u32, BOB_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("first account may debit three Kina");
    Transfer::asset_quantity(alternate_source.clone(), 2_u32, BOB_ID.clone())
        .execute(&carol, &mut stx)
        .expect("second account may spend the remaining two Kina");
    let overage = Transfer::asset_quantity(alternate_source.clone(), 1_u32, BOB_ID.clone())
        .execute(&carol, &mut stx)
        .expect_err("same identity must not restart the DAY cap at another account");
    assert_retail_cap_rejected(overage);
    let key = RetailDailyUsageKeyV1 {
        asset_definition_id: definition,
        physical_dataspace: DataSpaceId::new(7),
        identity: RetailIdentityCommitmentV1 { digest: [0xA1; 32] },
        utc_day_start_ms: 86_400_000,
    };
    assert_eq!(
        retail_state::usage_for_exact(stx.world(), &key).expect("canonical usage"),
        Some(Quantity::from(5_u32))
    );
    assert_eq!(
        asset_balance_or_zero(&stx, &alternate_source),
        Quantity::from(8_u32),
    );
}

#[test]
fn atomic_batch_rejects_two_accounts_sharing_one_identity() {
    let (state, definition, source, alternate_source, carol) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::new(7));
    stx.world.current_dataspace_id = Some(DataSpaceId::new(7));
    seed_test_call_hash(&mut stx, 0xE3);
    seed_test_retail_policy(
        &mut stx,
        &definition,
        5,
        &[
            (ALICE_ID.clone(), [0xA1; 32]),
            (carol.clone(), [0xA1; 32]),
            (BOB_ID.clone(), [0xB1; 32]),
        ],
    );
    stx.world.add_account_permission(
        &ALICE_ID,
        Permission::from(
            iroha_executor_data_model::permission::asset::CanTransferAsset {
                asset: alternate_source.clone(),
            },
        ),
    );
    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::with_leg_id(
            "first",
            ALICE_ID.clone(),
            BOB_ID.clone(),
            definition.clone(),
            3_u32,
        ),
        TransferAssetBatchEntry::with_leg_id(
            "second",
            carol,
            BOB_ID.clone(),
            definition.clone(),
            3_u32,
        ),
    ]);
    let error = batch
        .execute(&ALICE_ID, &mut stx)
        .expect_err("aggregate same-identity six Kina debit exceeds five Kina");
    assert_retail_cap_rejected(error);
    assert_eq!(asset_balance_or_zero(&stx, &source), Quantity::from(10_u32));
    assert_eq!(
        asset_balance_or_zero(&stx, &alternate_source),
        Quantity::from(10_u32),
    );
    assert!(
        stx.world
            .smart_contract_state
            .iter()
            .all(|(path, _)| { !path.as_ref().starts_with(retail_state::USAGE_ROOT) })
    );
}

#[test]
fn retail_governed_direct_mint_and_burn_fail_before_mutation() {
    let (state, definition, source, _, _) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    stx.current_dataspace_id = Some(DataSpaceId::new(7));
    stx.world.current_dataspace_id = Some(DataSpaceId::new(7));
    seed_test_retail_policy(&mut stx, &definition, 5, &[(ALICE_ID.clone(), [0xA1; 32])]);
    let mint_error = Mint::asset_quantity(1_u32, source.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("retail mint needs an owner-authorized typed purpose");
    assert!(mint_error.to_string().contains("no admitted typed purpose"));
    let burn_error = Burn::asset_quantity(1_u32, source.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("retail burn needs an owner-authorized typed purpose");
    assert!(burn_error.to_string().contains("no admitted typed purpose"));
    assert_eq!(asset_balance_or_zero(&stx, &source), Quantity::from(10_u32));
    assert!(
        stx.world
            .smart_contract_state
            .iter()
            .all(|(path, _)| { !path.as_ref().starts_with(retail_state::USAGE_ROOT) })
    );
}

#[test]
fn policy_revision_cannot_be_used_to_reset_the_same_identity_day_usage() {
    let (state, definition, source, _, _) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    let destination = AssetId::with_scope(
        definition.clone(),
        BOB_ID.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
    );
    let bindings = [(ALICE_ID.clone(), [0xA1; 32]), (BOB_ID.clone(), [0xB1; 32])];
    seed_test_retail_policy_revision(&mut stx, &definition, 5, 1, &bindings);
    let first = prepare_retail_daily_usage_update(
        &stx,
        &source,
        &destination,
        &Quantity::from(3_u32),
        NumericAssetTransferSourcePolicy::User,
    )
    .expect("initial debit preflight")
    .expect("installed policy");
    retail_state::put_usage(&mut stx.world, first.key.clone(), first.after)
        .expect("test-only canonical usage write");

    // A forged revision, even with a matching rewritten marker and bindings,
    // is rejected because first-release policy installation is immutable.
    // The existing identity-wide usage remains retained.
    seed_test_retail_policy_revision(&mut stx, &definition, 4, 2, &bindings);
    let error = prepare_retail_daily_usage_update(
        &stx,
        &source,
        &destination,
        &Quantity::one(),
        NumericAssetTransferSourcePolicy::User,
    )
    .expect_err("revision cannot reset DAY usage");
    assert!(error.to_string().contains("first-release policy revision"));
    assert_eq!(
        retail_state::usage_for_exact(stx.world(), &first.key).expect("retained usage"),
        Some(Quantity::from(3_u32))
    );
}

#[test]
fn retail_governed_teardown_rejects_account_definition_and_domain() {
    let (state, definition, source, _, _) = retail_cap_test_state();
    let mut block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, 86_400_000, 0));
    let mut stx = block.transaction();
    seed_test_retail_policy(&mut stx, &definition, 5, &[(ALICE_ID.clone(), [0xA1; 32])]);
    let account_error = Unregister::account(ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("bound retail account cannot be unregistered");
    assert!(account_error.to_string().contains("retail DAY"));
    let definition_error = Unregister::asset_definition(definition.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("governed definition cannot be unregistered");
    assert!(definition_error.to_string().contains("retail DAY"));
    let domain_error = Unregister::domain(wonderland_domain_id())
        .execute(&ALICE_ID, &mut stx)
        .expect_err("governed definition domain cannot be unregistered");
    assert!(domain_error.to_string().contains("retail DAY"));
    assert_eq!(asset_balance_or_zero(&stx, &source), Quantity::from(10_u32));
    assert!(stx.world.asset_definitions.get(&definition).is_some());
    assert!(stx.world.accounts.get(&ALICE_ID).is_some());
}
