//! Governance slashing and restitution flows for plain ballots and manual appeals.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::Domain,
    events::data::governance::GovernanceSlashReason,
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::{
    CanRestituteGovernanceLock, CanSubmitGovernanceBallot,
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, gen_account_in};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn governance_state_with_accounts(
    voting_asset_id: AssetDefinitionId,
    escrow_account: &iroha_data_model::account::AccountId,
    slash_account: &iroha_data_model::account::AccountId,
) -> State {
    let domain = Domain::new("wonderland".parse().expect("domain")).build(escrow_account);
    let alice_account =
        iroha_data_model::account::Account::new(ALICE_ID.clone()).build(escrow_account);
    let escrow =
        iroha_data_model::account::Account::new(escrow_account.clone()).build(escrow_account);
    let slash =
        iroha_data_model::account::Account::new(slash_account.clone()).build(escrow_account);
    let asset_def = AssetDefinition::numeric(voting_asset_id.clone()).build(escrow_account);
    // Seed balances: Alice 1_000, escrow 0, slash 0.
    let alice_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), ALICE_ID.clone()),
        Numeric::new(1_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(voting_asset_id.clone(), escrow_account.clone()),
        Numeric::new(0, 0),
    );
    let slash_asset = Asset::new(
        AssetId::new(voting_asset_id, slash_account.clone()),
        Numeric::new(0, 0),
    );

    let world = World::with_assets(
        [domain],
        [alice_account, escrow, slash],
        [asset_def],
        [alice_asset, escrow_asset, slash_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    State::new_for_testing(world, kura, query_handle)
}

fn seed_slash_snapshot(
    state: &mut State,
    rid: &str,
    escrow_asset_id: &AssetId,
    slash_asset_id: &AssetId,
) {
    let mut seed_block = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut seed_tx = seed_block.transaction();
    let mut locks = iroha_core::state::GovernanceLocksForReferendum::default();
    locks.locks.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceLockRecord {
            owner: ALICE_ID.clone(),
            amount: 60,
            slashed: 40,
            expiry_height: 100,
            direction: 0,
            duration_blocks: 100,
        },
    );
    seed_tx
        .world
        .governance_locks_mut()
        .insert(rid.to_string(), locks);
    let mut ledger = iroha_core::state::GovernanceSlashLedger::default();
    ledger.slashes.insert(
        ALICE_ID.clone(),
        iroha_core::state::GovernanceSlashEntry {
            total_slashed: 40,
            total_restituted: 0,
            last_reason: GovernanceSlashReason::DoubleVote,
            last_height: 1,
        },
    );
    seed_tx
        .world
        .governance_slashes_mut()
        .insert(rid.to_string(), ledger);
    **seed_tx
        .world
        .asset_mut(escrow_asset_id)
        .expect("escrow asset") = Numeric::new(60, 0);
    **seed_tx
        .world
        .asset_mut(slash_asset_id)
        .expect("slash asset") = Numeric::new(40, 0);
    seed_tx.apply();
    let _ = seed_block.commit();
}

#[test]
#[allow(clippy::too_many_lines)]
fn double_vote_slashes_plain_lock() {
    let def_id: AssetDefinitionId = "xor#wonderland".parse().expect("asset def id");
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.min_bond_amount = 10;
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    gov_cfg.slash_double_vote_bps = 2_000; // 20%
    state.set_gov(gov_cfg);

    // Block 1: seed referendum and cast initial ballot.
    let rid = "rid-slash-plain".to_string();
    {
        let header1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut sblock1 = state.block(header1);
        let mut stx1 = sblock1.transaction();
        stx1.world.governance_referenda_mut().insert(
            rid.clone(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 1,
                h_end: 50,
                status: iroha_core::state::GovernanceReferendumStatus::Proposed,
                mode: iroha_core::state::GovernanceReferendumMode::Plain,
            },
        );
        let perm: Permission = CanSubmitGovernanceBallot {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx1)
            .expect("grant ballot permission");

        let ballot_ok = iroha_data_model::isi::governance::CastPlainBallot {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 20,
            duration_blocks: 200,
            direction: 0,
        };
        ballot_ok
            .execute(&ALICE_ID, &mut stx1)
            .expect("first ballot should succeed");
        stx1.apply();
        let _ = sblock1.commit();
    }

    // Block 2: conflicting direction triggers slash + rejection.
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut sblock2 = state.block(header2);
    let mut stx2 = sblock2.transaction();
    let ballot_conflict = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: rid.clone(),
        owner: ALICE_ID.clone(),
        amount: 30,
        duration_blocks: 200,
        direction: 1, // switch direction to force double-vote slash
    };
    let err = ballot_conflict.execute(&ALICE_ID, &mut stx2).unwrap_err();
    assert!(
        err.to_string().contains("re-vote cannot change direction"),
        "expected direction change rejection"
    );
    let events = stx2.world.take_external_events();
    assert!(events.iter().any(|ev| {
        matches!(
            ev.as_data_event(),
            Some(iroha_data_model::events::data::DataEvent::Governance(
                iroha_data_model::events::data::governance::GovernanceEvent::LockSlashed(payload)
            )) if payload.referendum_id == rid
                && payload.reason == GovernanceSlashReason::DoubleVote
                && payload.amount == 4
                && payload.destination == slash_id
        )
    }));
    // Commit side effects so the slash is reflected in state for inspection.
    stx2.apply();
    let _ = sblock2.commit();

    // Escrow should now hold 16 (20 - 20% slash), slash receiver 4.
    let view = state.view();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id);
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id);
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after slash");
    assert_eq!(lock.amount, 16);
    assert_eq!(lock.slashed, 4);
    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Numeric::new(16, 0));
    assert_eq!(slash_balance.clone(), Numeric::new(4, 0));
}

#[test]
fn restitution_restores_slashed_balance() {
    let def_id: AssetDefinitionId = "xor#wonderland".parse().expect("asset def id");
    let (escrow_id, _) = gen_account_in("wonderland");
    let (slash_id, _) = gen_account_in("wonderland");
    let mut state = governance_state_with_accounts(def_id.clone(), &escrow_id, &slash_id);
    let alice = ALICE_ID.clone();
    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = slash_id.clone();
    state.set_gov(gov_cfg);

    let rid = "rid-restitute".to_string();
    let escrow_asset_id = AssetId::new(def_id.clone(), escrow_id.clone());
    let slash_asset_id = AssetId::new(def_id.clone(), slash_id.clone());
    // Pre-seed a lock with a recorded slash (amount=60 active, 40 slashed) and matching balances.
    seed_slash_snapshot(&mut state, &rid, &escrow_asset_id, &slash_asset_id);

    {
        let header = BlockHeader::new(nonzero!(3_u64), None, None, None, 0, 0);
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        stx.world.governance_referenda_mut().insert(
            rid.clone(),
            iroha_core::state::GovernanceReferendumRecord {
                h_start: 1,
                h_end: 200,
                status: iroha_core::state::GovernanceReferendumStatus::Open,
                mode: iroha_core::state::GovernanceReferendumMode::Plain,
            },
        );
        // Grant restitution permission to ALICE.
        let perm: Permission = CanRestituteGovernanceLock {
            referendum_id: rid.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("grant restitution permission");

        iroha_data_model::isi::governance::RestituteGovernanceLock {
            referendum_id: rid.clone(),
            owner: ALICE_ID.clone(),
            amount: 30,
            reason: "appeal_upheld".to_string(),
        }
        .execute(&ALICE_ID, &mut stx)
        .expect("restitution should succeed");
        let events = stx.world.take_external_events();
        assert!(events.iter().any(|ev| {
            matches!(
                ev.as_data_event(),
                Some(iroha_data_model::events::data::DataEvent::Governance(
                    iroha_data_model::events::data::governance::GovernanceEvent::LockRestituted(payload)
                )) if payload.amount == 30
                    && payload.reason == GovernanceSlashReason::Restitution
                    && payload.note == "appeal_upheld"
            )
        }));
        stx.apply();
        let _ = sblock.commit();
    }

    let view = state.view();
    let lock = view
        .world()
        .governance_locks()
        .get(&rid)
        .and_then(|locks| locks.locks.get(&alice))
        .expect("lock present after restitution");
    assert_eq!(lock.amount, 90);
    assert_eq!(lock.slashed, 10);

    let escrow_balance = view
        .world()
        .asset(&escrow_asset_id)
        .expect("escrow asset exists")
        .as_ref()
        .clone();
    let slash_balance = view
        .world()
        .asset(&slash_asset_id)
        .expect("slash receiver asset exists")
        .as_ref()
        .clone();
    assert_eq!(escrow_balance.clone(), Numeric::new(90, 0));
    assert_eq!(slash_balance.clone(), Numeric::new(10, 0));
}
