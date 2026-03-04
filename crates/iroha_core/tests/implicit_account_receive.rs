//! End-to-end regressions for domain-scoped implicit account receive.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{collections::BTreeMap, num::NonZeroU64};

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    account::{
        ACCOUNT_ADMISSION_POLICY_METADATA_KEY, AccountAdmissionMode, AccountAdmissionPolicy,
    },
    executor::ValidationFail,
    isi::error::{
        AccountAdmissionDefaultRoleError, AccountAdmissionError, AccountAdmissionQuotaExceeded,
        AccountAdmissionQuotaScope, InstructionExecutionError,
    },
    prelude::*,
    transaction::error::TransactionRejectionReason,
};
use iroha_primitives::json::Json;
use mv::storage::StorageReadOnly;

fn balance(state: &State, id: &AssetId) -> Numeric {
    state
        .view()
        .world()
        .assets()
        .get(id)
        .map_or_else(|| Numeric::new(0, 0), |value| value.clone().into_inner())
}

fn test_state(world: World) -> State {
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query)
}

fn prepare_state(
    policy: Option<AccountAdmissionPolicy>,
    alice_balance: Numeric,
) -> (
    State,
    DomainId,
    AccountId,
    KeyPair,
    AssetDefinitionId,
    AssetId,
) {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let alice_id = AccountId::new(domain_id.clone(), alice_kp.public_key().clone());

    let domain = policy.map_or_else(
        || Domain::new(domain_id.clone()).build(&alice_id),
        |policy| {
            let mut metadata = Metadata::default();
            let policy_key: Name = ACCOUNT_ADMISSION_POLICY_METADATA_KEY
                .parse()
                .expect("policy metadata key");
            metadata.insert(policy_key, Json::new(policy));
            Domain::new(domain_id.clone())
                .with_metadata(metadata)
                .build(&alice_id)
        },
    );

    let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let alice_asset_id = AssetId::new(asset_def_id.clone(), alice_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), alice_balance);
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);

    let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);

    (
        test_state(world),
        domain_id,
        alice_id,
        alice_kp,
        asset_def_id,
        alice_asset_id,
    )
}

#[test]
fn transfer_to_missing_account_creates_account_by_default() {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let alice_id = AccountId::new(domain_id.clone(), alice_kp.public_key().clone());

    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let alice_asset_id = AssetId::new(asset_def_id.clone(), alice_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(50, 0));
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);

    let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
    let state = test_state(world);
    let chain_id = state.chain_id.clone();

    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest = AccountId::new(domain_id.clone(), dest_kp.public_key().clone());
    assert!(
        state.view().world().accounts().get(&dest).is_none(),
        "destination must not exist before receipt"
    );

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(10, 0),
            dest.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();

    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_ok(), "transfer must succeed: {result:?}");
    state_block.commit().expect("commit state");

    let created_via_key: Name = "iroha:created_via".parse().expect("metadata key");
    let details = state
        .view()
        .world()
        .accounts()
        .get(&dest)
        .expect("destination account must exist after receipt")
        .clone()
        .into_inner();
    assert_eq!(
        details
            .metadata()
            .get(&created_via_key)
            .expect("implicit account must be tagged"),
        &Json::new("implicit")
    );

    let dest_asset_id = AssetId::new(asset_def_id, dest.clone());
    assert_eq!(balance(&state, &dest_asset_id), Numeric::new(10, 0));
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(40, 0));
}

#[test]
fn transfer_to_missing_account_rejected_in_explicit_domain() {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let alice_id = AccountId::new(domain_id.clone(), alice_kp.public_key().clone());

    let policy = AccountAdmissionPolicy {
        mode: AccountAdmissionMode::ExplicitOnly,
        max_implicit_creations_per_tx: None,
        max_implicit_creations_per_block: None,
        implicit_creation_fee: None,
        min_initial_amounts: BTreeMap::new(),
        default_role_on_create: None,
    };
    let mut metadata = Metadata::default();
    let policy_key: Name = ACCOUNT_ADMISSION_POLICY_METADATA_KEY
        .parse()
        .expect("policy metadata key");
    metadata.insert(policy_key, Json::new(policy));
    let domain: Domain = Domain::new(domain_id.clone())
        .with_metadata(metadata)
        .build(&alice_id);

    let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let alice_asset_id = AssetId::new(asset_def_id.clone(), alice_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(50, 0));
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);

    let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
    let state = test_state(world);
    let chain_id = state.chain_id.clone();

    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest = AccountId::new(domain_id.clone(), dest_kp.public_key().clone());

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(10, 0),
            dest.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();

    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    let err = result.expect_err("transfer must be rejected in ExplicitOnly domain");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::ImplicitAccountCreationDisabled(_)
                )
            ))
        ),
        "unexpected error: {err:?}"
    );

    state_block.commit().expect("commit state");
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(50, 0));
    assert!(
        state.view().world().accounts().get(&dest).is_none(),
        "destination must not exist after rejected transfer"
    );
}

#[test]
fn multiple_receipts_in_one_tx_create_account_once() {
    let (state, domain_id, alice_id, alice_kp, asset_def_id, alice_asset_id) =
        prepare_state(None, Numeric::new(50, 0));
    let chain_id = state.chain_id.clone();
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest = AccountId::new(domain_id.clone(), dest_kp.public_key().clone());

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(5, 0), dest.clone()),
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(7, 0), dest.clone()),
        ])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_ok(), "transfer must succeed: {result:?}");
    state_block.commit().expect("commit state");

    let dest_asset_id = AssetId::new(asset_def_id, dest.clone());
    assert_eq!(balance(&state, &dest_asset_id), Numeric::new(12, 0));
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(38, 0));

    let account_count = state.view().world().accounts().iter().count();
    assert_eq!(account_count, 2, "account should be created once");
}

#[test]
fn transaction_quota_limits_implicit_accounts() {
    let policy = AccountAdmissionPolicy {
        max_implicit_creations_per_tx: Some(1),
        ..AccountAdmissionPolicy::default()
    };
    let (state, domain_id, alice_id, alice_kp, _asset_def_id, alice_asset_id) =
        prepare_state(Some(policy), Numeric::new(40, 0));
    let chain_id = state.chain_id.clone();
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest1_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
    let dest2_kp = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
    let dest2 = AccountId::new(domain_id.clone(), dest2_kp.public_key().clone());

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(5, 0), dest1.clone()),
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(5, 0), dest2.clone()),
        ])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_100_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    let err = result.expect_err("tx should be rejected by per-tx quota");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::QuotaExceeded(
                    AccountAdmissionQuotaExceeded {
                        scope: AccountAdmissionQuotaScope::Transaction,
                        ..
                    }
                ))
            ))
        ),
        "unexpected error: {err:?}"
    );
    state_block.commit().expect("commit state");

    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(40, 0));
    assert!(
        state.view().world().accounts().get(&dest1).is_none(),
        "dest1 must not be created on rejection"
    );
    assert!(
        state.view().world().accounts().get(&dest2).is_none(),
        "dest2 must not be created on rejection"
    );
}

#[test]
fn block_quota_limits_creations_across_transactions() {
    let policy = AccountAdmissionPolicy {
        max_implicit_creations_per_block: Some(1),
        ..AccountAdmissionPolicy::default()
    };
    let (state, domain_id, alice_id, alice_kp, asset_def_id, alice_asset_id) =
        prepare_state(Some(policy), Numeric::new(60, 0));
    let chain_id = state.chain_id.clone();
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest1_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
    let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(10, 0),
            dest1.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted1 =
        AcceptedTransaction::accept(tx1, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("tx1 admission must succeed");

    let dest2_kp = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
    let dest2 = AccountId::new(domain_id.clone(), dest2_kp.public_key().clone());
    let tx2 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(3, 0),
            dest2.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted2 =
        AcceptedTransaction::accept(tx2, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("tx2 admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_200_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, res1) = state_block.validate_transaction(accepted1, &mut ivm_cache);
    assert!(res1.is_ok(), "first tx should succeed: {res1:?}");
    let (_, res2) = state_block.validate_transaction(accepted2, &mut ivm_cache);
    let err = res2.expect_err("second tx must be rejected by block quota");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::QuotaExceeded(
                    AccountAdmissionQuotaExceeded {
                        scope: AccountAdmissionQuotaScope::Block,
                        ..
                    }
                ))
            ))
        ),
        "unexpected error: {err:?}"
    );
    state_block.commit().expect("commit state");

    let dest1_asset_id = AssetId::new(asset_def_id.clone(), dest1.clone());
    assert_eq!(balance(&state, &dest1_asset_id), Numeric::new(10, 0));
    assert!(
        state.view().world().accounts().get(&dest2).is_none(),
        "second account must not be created"
    );
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(50, 0));
}

#[test]
fn missing_default_role_rejects_in_pipeline() {
    let role_id: RoleId = "missing".parse().expect("role id");
    let policy = AccountAdmissionPolicy {
        default_role_on_create: Some(role_id.clone()),
        ..AccountAdmissionPolicy::default()
    };
    let (state, domain_id, alice_id, alice_kp, _asset_def_id, alice_asset_id) =
        prepare_state(Some(policy), Numeric::new(25, 0));
    let chain_id = state.chain_id.clone();
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest_kp = KeyPair::from_seed(vec![11; 32], Algorithm::Ed25519);
    let dest = AccountId::new(domain_id.clone(), dest_kp.public_key().clone());
    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(5, 0),
            dest.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_400_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    let err = result.expect_err("tx should fail when default role is missing");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::AccountAdmission(
                    AccountAdmissionError::DefaultRoleError(AccountAdmissionDefaultRoleError {
                        ref role,
                        ..
                    })
                )
            )) if role == &role_id
        ),
        "unexpected error: {err:?}"
    );
    state_block.commit().expect("commit state");

    assert!(
        state.view().world().accounts().get(&dest).is_none(),
        "account must not be created on failure"
    );
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(25, 0));
}

#[test]
fn implicit_account_can_spend_without_roles() {
    let (state, domain_id, alice_id, alice_kp, asset_def_id, alice_asset_id) =
        prepare_state(None, Numeric::new(20, 0));
    let chain_id = state.chain_id.clone();
    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let bob_kp = KeyPair::from_seed(vec![5; 32], Algorithm::Ed25519);
    let bob_id = AccountId::new(domain_id.clone(), bob_kp.public_key().clone());
    let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(7, 0),
            bob_id.clone(),
        )])
        .sign(alice_kp.private_key());
    let accepted1 =
        AcceptedTransaction::accept(tx1, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("tx1 admission must succeed");

    let mut block1 = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_500_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();
    let (_, res1) = block1.validate_transaction(accepted1, &mut ivm_cache);
    assert!(res1.is_ok(), "first transfer should succeed: {res1:?}");
    block1.commit().expect("commit first block");

    let bob_asset_id = AssetId::new(asset_def_id.clone(), bob_id.clone());
    assert_eq!(balance(&state, &bob_asset_id), Numeric::new(7, 0));
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(13, 0));

    let tx2 = TransactionBuilder::new(chain_id.clone(), bob_id.clone())
        .with_instructions([Transfer::asset_numeric(
            bob_asset_id.clone(),
            Numeric::new(5, 0),
            alice_id.clone(),
        )])
        .sign(bob_kp.private_key());
    let accepted2 =
        AcceptedTransaction::accept(tx2, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("tx2 admission must succeed");

    let mut block2 = state.block(BlockHeader::new(
        NonZeroU64::new(2).expect("height"),
        None,
        None,
        None,
        1_700_000_600_000,
        0,
    ));
    let mut ivm_cache2 = IvmCache::new();
    let (_, res2) = block2.validate_transaction(accepted2, &mut ivm_cache2);
    assert!(
        res2.is_ok(),
        "implicit account should be able to spend: {res2:?}"
    );
    block2.commit().expect("commit second block");

    assert_eq!(balance(&state, &bob_asset_id), Numeric::new(2, 0));
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(18, 0));
}

#[test]
fn multi_receipts_within_transaction_succeed_in_open_domain() {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let alice_id = AccountId::new(domain_id.clone(), alice_kp.public_key().clone());

    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let alice_asset_id = AssetId::new(asset_def_id.clone(), alice_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(50, 0));
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);

    let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
    let state = test_state(world);
    let chain_id = state.chain_id.clone();

    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest1_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
    let dest2_kp = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
    let dest2 = AccountId::new(domain_id.clone(), dest2_kp.public_key().clone());

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(5, 0), dest1.clone()),
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(7, 0), dest2.clone()),
        ])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();

    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    assert!(result.is_ok(), "batched transfers must succeed: {result:?}");
    state_block.commit().expect("commit state");

    let created_via_key: Name = "iroha:created_via".parse().expect("metadata key");
    for dest in [&dest1, &dest2] {
        let details = state
            .view()
            .world()
            .accounts()
            .get(dest)
            .expect("destination account must exist after receipt")
            .clone()
            .into_inner();
        assert_eq!(
            details.metadata().get(&created_via_key),
            Some(&Json::new("implicit"))
        );
    }

    let dest1_asset_id = AssetId::new(asset_def_id.clone(), dest1.clone());
    let dest2_asset_id = AssetId::new(asset_def_id, dest2.clone());
    assert_eq!(balance(&state, &dest1_asset_id), Numeric::new(5, 0));
    assert_eq!(balance(&state, &dest2_asset_id), Numeric::new(7, 0));
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(38, 0));
}

#[test]
fn tx_cap_rejects_multiple_implicit_creations() {
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let alice_kp = KeyPair::from_seed(vec![1; 32], Algorithm::Ed25519);
    let alice_id = AccountId::new(domain_id.clone(), alice_kp.public_key().clone());

    let policy = AccountAdmissionPolicy {
        max_implicit_creations_per_tx: Some(1),
        ..AccountAdmissionPolicy::default()
    };
    let mut metadata = Metadata::default();
    let policy_key: Name = ACCOUNT_ADMISSION_POLICY_METADATA_KEY
        .parse()
        .expect("policy metadata key");
    metadata.insert(policy_key, Json::new(policy));
    let domain: Domain = Domain::new(domain_id.clone())
        .with_metadata(metadata)
        .build(&alice_id);

    let asset_def_id: AssetDefinitionId = "coin#wonderland".parse().expect("asset def id");
    let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&alice_id);
    let alice_asset_id = AssetId::new(asset_def_id.clone(), alice_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(50, 0));
    let alice_account = Account::new(alice_id.clone()).build(&alice_id);

    let world = World::with_assets([domain], [alice_account], [asset_def], [alice_asset], []);
    let state = test_state(world);
    let chain_id = state.chain_id.clone();

    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let dest1_kp = KeyPair::from_seed(vec![2; 32], Algorithm::Ed25519);
    let dest1 = AccountId::new(domain_id.clone(), dest1_kp.public_key().clone());
    let dest2_kp = KeyPair::from_seed(vec![3; 32], Algorithm::Ed25519);
    let dest2 = AccountId::new(domain_id, dest2_kp.public_key().clone());

    let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(1, 0), dest1.clone()),
            Transfer::asset_numeric(alice_asset_id.clone(), Numeric::new(1, 0), dest2.clone()),
        ])
        .sign(alice_kp.private_key());
    let accepted =
        AcceptedTransaction::accept(tx, &chain_id, max_clock_drift, tx_params, crypto.as_ref())
            .expect("transaction admission must succeed");

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();

    let (_, result) = state_block.validate_transaction(accepted, &mut ivm_cache);
    let err = result.expect_err("cap should reject second implicit creation");
    assert!(
        matches!(
            err,
            TransactionRejectionReason::Validation(ValidationFail::InstructionFailed(
                InstructionExecutionError::AccountAdmission(AccountAdmissionError::QuotaExceeded(
                    AccountAdmissionQuotaExceeded {
                        scope: AccountAdmissionQuotaScope::Transaction,
                        ..
                    }
                ))
            ))
        ),
        "unexpected error: {err:?}"
    );

    state_block.commit().expect("commit state");
    assert!(
        state.view().world().accounts().get(&dest1).is_none(),
        "dest1 must not exist after rejection"
    );
    assert!(
        state.view().world().accounts().get(&dest2).is_none(),
        "dest2 must not exist after rejection"
    );
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(50, 0));
}
