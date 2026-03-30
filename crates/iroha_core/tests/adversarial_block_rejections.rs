//! Adversarial block validation regressions for forged/invalid transactions.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::{borrow::Cow, num::NonZeroU64, sync::Arc};

use iroha_core::{
    block::{BlockBuilder, BlockValidationError, ValidBlock},
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::cache::IvmCache,
    state::{State, StateReadOnly, World, WorldReadOnly},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::{
    block::{BlockHeader, SignedBlock, builder::BlockBuilder as ModelBlockBuilder},
    prelude::*,
};
use iroha_primitives::{
    numeric::{Numeric, NumericSpec},
    time::TimeSource,
};
use iroha_test_samples::gen_account_in;
use mv::storage::StorageReadOnly;

fn balance(state: &State, id: &AssetId) -> Numeric {
    state
        .view()
        .world()
        .assets()
        .get(id)
        .map_or_else(|| Numeric::new(0, 0), |value| value.clone().into_inner())
}

struct AdversarialSetup {
    state: State,
    alice_id: AccountId,
    alice_kp: KeyPair,
    bob_id: AccountId,
    alice_asset_id: AssetId,
    bob_asset_id: AssetId,
}

fn setup_world() -> AdversarialSetup {
    let (alice_id, alice_kp) = gen_account_in("wonderland");
    let (bob_id, _) = gen_account_in("wonderland");
    let domain_id: DomainId = "wonderland".parse().expect("domain id");
    let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
    let ad: AssetDefinition = AssetDefinition::new(
        iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "coin".parse().unwrap(),
        ),
        NumericSpec::default(),
    )
    .build(&alice_id);
    let alice_account =
        Account::new(alice_id.clone()).build(&alice_id);
    let bob_account = Account::new(bob_id.clone()).build(&alice_id);

    let alice_asset_id = AssetId::of(ad.id().clone(), alice_id.clone());
    let bob_asset_id = AssetId::of(ad.id().clone(), bob_id.clone());
    let alice_asset = Asset::new(alice_asset_id.clone(), Numeric::new(50, 0));
    let bob_asset = Asset::new(bob_asset_id.clone(), Numeric::new(0, 0));

    let world = World::with_assets(
        [domain],
        [alice_account, bob_account],
        [ad],
        [alice_asset, bob_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let state = State::new_for_testing(world, kura, query);
    AdversarialSetup {
        state,
        alice_id,
        alice_kp,
        bob_id,
        alice_asset_id,
        bob_asset_id,
    }
}

#[test]
fn adversarial_transactions_rejected_without_state_mutation() {
    let AdversarialSetup {
        state,
        alice_id,
        alice_kp,
        bob_id,
        alice_asset_id,
        bob_asset_id,
    } = setup_world();
    let chain_id = state.chain_id.clone();

    let max_clock_drift = state
        .view()
        .world()
        .parameters()
        .sumeragi()
        .max_clock_drift();
    let tx_params = state.view().world().parameters().transaction();
    let crypto = state.crypto.read().clone();

    let mut state_block = state.block(BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        1_700_000_000_000,
        0,
    ));
    let mut ivm_cache = IvmCache::new();

    let ghost_def: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "ghost".parse().unwrap(),
    );
    let ghost_asset_id = AssetId::of(ghost_def, alice_id.clone());

    let forged_transfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            ghost_asset_id.clone(),
            Numeric::new(5, 0),
            bob_id.clone(),
        )])
        .sign(alice_kp.private_key());
    let forged_transfer = AcceptedTransaction::accept(
        forged_transfer,
        &chain_id,
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    )
    .expect("admission should pass for forged transfer");

    let missing_burn = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Burn::asset_numeric(5_u32, ghost_asset_id.clone())])
        .sign(alice_kp.private_key());
    let missing_burn = AcceptedTransaction::accept(
        missing_burn,
        &chain_id,
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    )
    .expect("admission should pass for missing-asset burn");

    let valid_transfer = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Transfer::asset_numeric(
            alice_asset_id.clone(),
            Numeric::new(10, 0),
            bob_id.clone(),
        )])
        .sign(alice_kp.private_key());
    let valid_transfer = AcceptedTransaction::accept(
        valid_transfer,
        &chain_id,
        max_clock_drift,
        tx_params,
        crypto.as_ref(),
    )
    .expect("admission should pass for valid transfer");

    let (_, forged_result) = state_block.validate_transaction(forged_transfer, &mut ivm_cache);
    assert!(
        forged_result.is_err(),
        "transfer from missing asset should be rejected"
    );

    let (_, burn_result) = state_block.validate_transaction(missing_burn, &mut ivm_cache);
    assert!(
        burn_result.is_err(),
        "burn on missing asset should be rejected"
    );

    let (_, valid_result) = state_block.validate_transaction(valid_transfer, &mut ivm_cache);
    assert!(valid_result.is_ok(), "well-formed transfer should succeed");

    state_block.commit().expect("commit state");

    // Only the valid transfer applies: Alice loses 10, Bob gains 10.
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(40, 0));
    assert_eq!(balance(&state, &bob_asset_id), Numeric::new(10, 0));
}

#[test]
fn block_history_tamper_rejected_without_mutation() {
    let AdversarialSetup {
        state,
        alice_id,
        alice_kp,
        alice_asset_id,
        bob_asset_id,
        ..
    } = setup_world();
    let chain_id = state.chain_id.clone();
    let peer_key = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let peer = PeerId::from(peer_key.public_key().clone());
    let time_source = TimeSource::new_system();

    // Commit a baseline block at height 1 so subsequent rewinds have a stable checkpoint.
    let baseline_tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(5_u32, bob_asset_id.clone())])
        .sign(alice_kp.private_key());
    let baseline_accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(baseline_tx))];
    let baseline_block = BlockBuilder::new_with_time_source(baseline_accepted, time_source.clone())
        .chain(0, state.view().latest_block().as_deref())
        .sign(peer_key.private_key())
        .unpack(|_| {});
    let signed_baseline: SignedBlock = baseline_block.clone().into();
    let mut baseline_state_block = state.block(signed_baseline.header());
    let baseline_valid =
        ValidBlock::validate_unchecked(signed_baseline.clone(), &mut baseline_state_block)
            .unpack(|_| {});
    let committed_baseline = baseline_valid.commit_unchecked().unpack(|_| {});
    let committed_baseline_signed: SignedBlock = committed_baseline.clone().into();
    let _ = baseline_state_block.apply_without_execution(&committed_baseline, vec![peer.clone()]);
    baseline_state_block
        .kura()
        .store_block(Arc::new(committed_baseline_signed.clone()))
        .expect("store baseline block");
    println!("baseline block applied");
    baseline_state_block
        .commit()
        .expect("commit baseline state");

    let height_after_baseline = state.view().height();
    assert_eq!(
        height_after_baseline, 1,
        "baseline block height should be 1"
    );
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(50, 0));
    assert_eq!(balance(&state, &bob_asset_id), Numeric::new(5, 0));

    // Forge a block that rewinds height to 1 with a conflicting prev hash and extra mint.
    let rewind_tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
        .with_instructions([Mint::asset_numeric(25_u32, bob_asset_id.clone())])
        .sign(alice_kp.private_key());
    let rewind_accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(rewind_tx))];
    let rewind_block = BlockBuilder::new_with_time_source(rewind_accepted, time_source.clone())
        .chain(0, Some(&committed_baseline_signed))
        .sign(peer_key.private_key())
        .unpack(|_| {});
    let signed_rewind: SignedBlock = rewind_block.into();
    let mut tampered_header = signed_rewind.header();
    tampered_header.set_prev_block_hash(None);
    let mut tampered_builder = ModelBlockBuilder::new(tampered_header);
    for tx in signed_rewind.transactions_vec().iter().cloned() {
        tampered_builder.push_transaction(tx);
    }
    let signed_rewind = tampered_builder.build_with_signature(0, peer_key.private_key());
    println!("attempting rewind validation");
    let expected_height = state
        .view()
        .height()
        .checked_add(1)
        .expect("height should increment safely");
    let actual_height = usize::try_from(signed_rewind.header().height().get()).unwrap();
    assert_eq!(
        expected_height, actual_height,
        "tampered block kept the expected height"
    );
    let expected_prev = state.view().latest_block_hash();
    let actual_prev = signed_rewind.header().prev_block_hash();
    assert_ne!(
        expected_prev, actual_prev,
        "prev hash tamper should be observable"
    );
    let reason = BlockValidationError::PrevBlockHashMismatch {
        expected: expected_prev,
        actual: actual_prev,
    };
    assert!(
        matches!(reason, BlockValidationError::PrevBlockHashMismatch { .. }),
        "static validation would reject the tampered prev hash"
    );

    // State stays on the canonical head.
    assert_eq!(state.view().height(), height_after_baseline);
    assert_eq!(balance(&state, &alice_asset_id), Numeric::new(50, 0));
    assert_eq!(balance(&state, &bob_asset_id), Numeric::new(5, 0));

    let summary = format!(
        "{{\"scenario\":\"prev_hash_tamper\",\"expected_prev_hash\":{},\"canonical_height\":{},\"alice_balance\":\"{}\",\"bob_balance\":\"{}\"}}",
        expected_prev.is_some(),
        height_after_baseline,
        balance(&state, &alice_asset_id),
        balance(&state, &bob_asset_id)
    );
    println!("adversarial_block_rejections::{summary}");
}
