//! Composed producer/verifier controls on the actual certified activation and bootstrap.

use super::*;
use iroha_allocation::AllocationBudget;
use iroha_core_privacy::ironwood_replay::verify_world_replay_context_v1;
use iroha_data_model::account::AccountId;
use iroha_data_model::sumeragi_finality::{VerifiedSumeragiBlock, ironwood_world_schema_v1};
use std::cell::Cell;

use crate::state::WorldStateSnapshotError;
use crate::sumeragi::certified_chain::CurrentCertifiedCut;

fn selected_cut(fixture: &Fixture, verified: &VerifiedSumeragiBlock) -> CurrentCertifiedCut {
    let state = fixture.chain.state();
    let limits = crate::sumeragi::certified_chain::CurrentCutSourceLimits::new(
        iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_BLOCK_BYTES,
        iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_JOURNAL_BYTES,
        3,
    )
    .unwrap();
    let frames = AllocationBudget::new(
        iroha_data_model::sumeragi::finality::NATIVE_FINALITY_MAX_JOURNAL_BYTES,
    );
    let graphs = AllocationBudget::new(256 * 1024 * 1024);
    let tip = crate::sumeragi::certified_chain::read_current_certified_cut(
        &state.view(),
        verified.height(),
        fixture.chain.root_scope(),
        limits,
        &frames,
        &graphs,
    )
    .unwrap();
    assert_eq!(frames.reserved_bytes(), 0);
    assert_eq!(graphs.reserved_bytes(), 0);
    tip
}

fn reader<'a>(
    fixture: &'a Fixture,
    verified: &'a VerifiedSumeragiBlock,
    tip: &'a CurrentCertifiedCut,
    called: &'a Cell<usize>,
) -> impl Fn(
    PrivacyPoolIdV1,
    &AssetDefinitionId,
    &AccountId,
    &AccountId,
    &AllocationBudget,
) -> Result<(), WorldStateSnapshotError>
+ 'a {
    let state = fixture.chain.state();
    let (predecessor, published) = repaired(crate::runtime::release_catalog().unwrap());
    let chosen = change_set(&predecessor, &published).successor;
    move |pool,
          asset: &AssetDefinitionId,
          payer: &AccountId,
          reserve: &AccountId,
          budget: &AllocationBudget| {
        state.with_replayed_ironwood_context_v1(
            tip,
            pool,
            asset,
            payer,
            reserve,
            budget,
            |world, originals| {
                called.set(called.get() + 1);
                assert!(
                    budget.reserved_bytes() > 0,
                    "physical owners remain charged in the consumer"
                );
                assert_eq!(
                    world.root().unwrap().as_ref(),
                    tip.world_state_root().as_ref()
                );
                assert_eq!(world.schema_hash, ironwood_world_schema_v1());
                let world_frame = norito::encode_canonical(world).unwrap();
                let originals_frame = norito::encode_canonical(originals).unwrap();
                let accepted = verify_world_replay_context_v1(
                    verified,
                    &world_frame,
                    &originals_frame,
                    fixture.chain.network_id(),
                    chosen,
                    ironwood_world_schema_v1(),
                    pool,
                    asset,
                    payer,
                )
                .unwrap();
                assert_eq!(
                    accepted.state(),
                    originals.state.orchard_pool_state_ref().unwrap()
                );
                assert_eq!(accepted.payer_balance(), &originals.payer_balance);
                assert_eq!(accepted.reserve_balance(), &originals.reserve_balance);
                assert!(
                    accepted.payer_balance().is_none(),
                    "bootstrap preserves certified absence"
                );
                assert!(accepted.reserve_balance().is_none());
                Ok(())
            },
        )
    }
}

fn assert_wrong_selectors(
    read: &impl Fn(
        PrivacyPoolIdV1,
        &AssetDefinitionId,
        &AccountId,
        &AccountId,
        &AllocationBudget,
    ) -> Result<(), WorldStateSnapshotError>,
    request: &PrivacyIronwoodPoolBootstrapV1,
    payer: &AccountId,
    budget: &AllocationBudget,
    called: &Cell<usize>,
) {
    let foreign_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("privacy", "universal").unwrap(),
        "foreign".parse().unwrap(),
    );
    for (pool, asset, payer, reserve) in [
        (
            PrivacyPoolIdV1::new([0; 32]),
            &request.asset_definition_id,
            payer,
            &request.reserve_account,
        ),
        (
            PrivacyPoolIdV1::new([0xe1; 32]),
            &request.asset_definition_id,
            payer,
            &request.reserve_account,
        ),
        (
            request.pool_id,
            &foreign_asset,
            payer,
            &request.reserve_account,
        ),
        (request.pool_id, &request.asset_definition_id, payer, payer),
        (
            request.pool_id,
            &request.asset_definition_id,
            payer,
            &account(2),
        ),
    ] {
        assert!(read(pool, asset, payer, reserve, budget).is_err());
        assert_eq!(
            called.get(),
            1,
            "rejected original never reaches the consumer"
        );
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

pub(super) fn assert_native_callback(fixture: &Fixture, verified: &VerifiedSumeragiBlock) {
    let state = fixture.chain.state();
    let tip = selected_cut(fixture, verified);
    let request = pool();
    let payer = account(6);
    let budget = AllocationBudget::new(384 * 1024 * 1024);
    let called = Cell::new(0usize);
    let read = reader(fixture, verified, &tip, &called);
    read(
        request.pool_id,
        &request.asset_definition_id,
        &payer,
        &request.reserve_account,
        &budget,
    )
    .unwrap();
    assert_eq!(called.get(), 1);
    assert_eq!(budget.reserved_bytes(), 0);
    assert!(
        read(
            request.pool_id,
            &request.asset_definition_id,
            &payer,
            &request.reserve_account,
            &AllocationBudget::new(0)
        )
        .is_err()
    );
    assert_wrong_selectors(&read, &request, &payer, &budget, &called);
    let failure = state
        .with_replayed_ironwood_context_v1(
            &tip,
            request.pool_id,
            &request.asset_definition_id,
            &payer,
            &request.reserve_account,
            &budget,
            |_, _| Err::<(), _>("consumer publication refused".into()),
        )
        .unwrap_err();
    assert!(
        matches!(failure, crate::state::WorldStateSnapshotError::Invalid(message)
        if message == "consumer publication refused")
    );
    assert_eq!(budget.reserved_bytes(), 0);
    read(
        request.pool_id,
        &request.asset_definition_id,
        &payer,
        &request.reserve_account,
        &budget,
    )
    .unwrap();
    assert_eq!(called.get(), 2);
    assert_eq!(budget.reserved_bytes(), 0);
}
