//! Governance lock slashing and restitution flows.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//!
//! Verifies manual slashing/restitution of governance bonds updates locks,
//! ledgers, and balances.

use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
};
use iroha_data_model::{
    Registrable,
    account::AccountId,
    asset::{Asset, AssetDefinition},
    block::BlockHeader,
    domain::Domain,
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::{
    CanRestituteGovernanceLock, CanSlashGovernanceLock, CanSubmitGovernanceBallot,
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID, gen_account_in};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn setup_state(def_id: &AssetDefinitionId, receiver_id: &AccountId) -> State {
    let alice_id = ALICE_ID.clone();
    let escrow_id = BOB_ID.clone();
    let wonderland: iroha_data_model::domain::DomainId = "wonderland".parse().expect("domain");

    let domain = Domain::new(wonderland.clone()).build(&alice_id);
    let alice_account =
        iroha_data_model::account::Account::new(ALICE_ID.clone().to_account_id(wonderland.clone()))
            .build(&alice_id);
    let escrow_account =
        iroha_data_model::account::Account::new(BOB_ID.clone().to_account_id(wonderland.clone()))
            .build(&alice_id);
    let receiver_account =
        iroha_data_model::account::Account::new(receiver_id.clone().to_account_id(wonderland))
            .build(&alice_id);
    let asset_def = AssetDefinition::numeric(def_id.clone()).build(&alice_id);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Numeric::new(1_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Numeric::new(0, 0),
    );
    let receiver_asset = Asset::new(
        AssetId::new(def_id.clone(), receiver_id.clone()),
        Numeric::new(0, 0),
    );

    let world = World::with_assets(
        [domain],
        [alice_account, escrow_account, receiver_account],
        [asset_def],
        [alice_asset, escrow_asset, receiver_asset],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.min_bond_amount = 10;
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = receiver_id.clone();
    state.set_gov(gov_cfg);
    state
}

fn grant_governance_perms(
    tx: &mut iroha_core::state::StateTransaction<'_, '_>,
    referendum_id: &str,
    alice_id: &AccountId,
) {
    for perm in [
        Permission::from(CanSubmitGovernanceBallot {
            referendum_id: referendum_id.to_string(),
        }),
        Permission::from(CanSlashGovernanceLock {
            referendum_id: referendum_id.to_string(),
        }),
        Permission::from(CanRestituteGovernanceLock {
            referendum_id: referendum_id.to_string(),
        }),
    ] {
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(alice_id, tx)
            .expect("grant governance permission");
    }
}

fn seed_plain_referendum(
    tx: &mut iroha_core::state::StateTransaction<'_, '_>,
    referendum_id: &str,
) {
    tx.world.governance_referenda_mut().insert(
        referendum_id.to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            h_end: 5,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
}

fn lock_slash_restitute(
    tx: &mut iroha_core::state::StateTransaction<'_, '_>,
    referendum_id: &str,
    owner: &AccountId,
) {
    let ballot = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount: 10,
        duration_blocks: 200,
        direction: 0,
    };
    ballot
        .clone()
        .execute(owner, tx)
        .expect("ballot should lock funds");

    let slash = iroha_data_model::isi::governance::SlashGovernanceLock {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount: 4,
        reason: "policy_violation".to_string(),
    };
    slash
        .clone()
        .execute(owner, tx)
        .expect("slash should succeed");

    let restitute = iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount: 2,
        reason: "appeal".to_string(),
    };
    restitute
        .execute(owner, tx)
        .expect("restitution should succeed");
}

#[test]
fn manual_slash_and_restitution_move_bonds_and_record_ledger() {
    let (receiver_id, _) = gen_account_in("wonderland");
    let def_id: AssetDefinitionId = "xor#wonderland".parse().expect("asset def id");
    let state = setup_state(&def_id, &receiver_id);
    let alice_id = ALICE_ID.clone();
    let referendum_id = "rid-slash";

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    grant_governance_perms(&mut stx, referendum_id, &alice_id);
    seed_plain_referendum(&mut stx, referendum_id);
    lock_slash_restitute(&mut stx, referendum_id, &alice_id);
    stx.apply();

    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    let receiver_asset_id = AssetId::new(def_id.clone(), receiver_id.clone());
    let escrow_balance = sblock
        .world
        .assets()
        .get(&escrow_asset_id)
        .expect("escrow asset")
        .clone();
    let receiver_balance = sblock
        .world
        .assets()
        .get(&receiver_asset_id)
        .expect("receiver asset")
        .clone();
    assert_eq!(escrow_balance.into_inner(), Numeric::new(8, 0));
    assert_eq!(receiver_balance.into_inner(), Numeric::new(2, 0));

    let locks = sblock
        .world
        .governance_locks()
        .get(referendum_id)
        .expect("locks after slash");
    let rec = locks.locks.get(&alice_id).expect("alice lock after slash");
    assert_eq!(rec.amount, 8);
    assert_eq!(rec.slashed, 2);

    let ledger = sblock
        .world
        .governance_slashes()
        .get(referendum_id)
        .expect("slash ledger");
    let entry = ledger.slashes.get(&alice_id).expect("slash ledger entry");
    assert_eq!(entry.total_slashed, 4);
    assert_eq!(entry.total_restituted, 2);
    assert_eq!(
        entry.last_reason,
        iroha_data_model::events::data::governance::GovernanceSlashReason::Restitution
    );
}
