//! Governance lock slashing and restitution flows.

//!
//! Verifies manual slashing/restitution of governance bonds updates locks, ledgers, and balances.
use crate::native_genesis::{certified_state, certified_state_with};
use iroha_config::parameters::actual::Governance;
use iroha_core::{
    smartcontracts::Execute,
    state::{State, World, WorldReadOnly},
    sumeragi::test_chain::TestChainConfig,
};
use iroha_crypto::Hash;
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
use iroha_model_base::domain::DomainId;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID, gen_account_in};
use mv::storage::StorageReadOnly;
/// Direct governance movements execute under one explicit retained invocation, as a signed
/// call does.
fn governance_call() -> Hash {
    Hash::new(b"gov-slash-restitution-component")
}
/// An ordinary fixture block after the authenticated genesis at height one.
fn ordinary_header(state: &State, height: u64) -> BlockHeader {
    BlockHeader::new(
        std::num::NonZeroU64::new(height).expect("non-zero height"),
        state.view().latest_block_hash(),
        None,
        height,
        0,
    )
}
fn setup_state(def_id: &AssetDefinitionId, receiver_id: &AccountId) -> State {
    setup_state_with(def_id, receiver_id, |_| {})
}
/// Apply the authenticated genesis with the plain-voting policy fixed before signing.
///
/// The genesis also authors the ordinary signing policy used by the funding helper's account
/// admission.
fn setup_state_with(
    def_id: &AssetDefinitionId,
    receiver_id: &AccountId,
    configure: impl FnOnce(&mut Governance),
) -> State {
    let alice_id = ALICE_ID.clone();
    let escrow_id = BOB_ID.clone();
    let wonderland: iroha_model_base::domain::DomainId =
        iroha_model_base::domain::DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(wonderland.clone()).build(&alice_id);
    let alice_account = iroha_data_model::account::Account::new(ALICE_ID.clone()).build(&alice_id);
    let escrow_account = iroha_data_model::account::Account::new(BOB_ID.clone()).build(&alice_id);
    let receiver_account =
        iroha_data_model::account::Account::new(receiver_id.clone()).build(&alice_id);
    let asset_def = AssetDefinition::numeric(
        def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&alice_id);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Quantity::from(0_u64),
    );
    let receiver_asset = Asset::new(
        AssetId::new(def_id.clone(), receiver_id.clone()),
        Quantity::from(0_u64),
    );
    let world = World::with_assets(
        [domain],
        [alice_account, escrow_account, receiver_account],
        [asset_def],
        [alice_asset, escrow_asset, receiver_asset],
        [],
    );
    let mut gov_cfg = certified_state(World::default()).gov.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.min_bond_amount = 10_u64.into();
    gov_cfg.bond_escrow_account = escrow_id.clone();
    gov_cfg.slash_receiver_account = receiver_id.clone();
    configure(&mut gov_cfg);
    let mut config = TestChainConfig::new(world, 0);
    config.governance = Some(gov_cfg);
    certified_state_with(config)
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
    iroha_core::query::standalone_plain_test_fixture::fund_voter(
        tx,
        &iroha_test_samples::ALICE_ID,
        1_000_000_u64.into(),
        0,
    );
    let plain_context = iroha_core::query::standalone_plain_test_fixture::context(&tx.gov, 0);
    tx.world.governance_referenda_mut().insert(
        referendum_id.to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 2,
            h_end: 6,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
            plain_context,
            plain_result: iroha_data_model::governance::conviction::PlainVotingResultV1::Pending,
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
        direction: 0,
        owner: owner.clone(),
        amount: 10_u64.into(),
        duration_blocks: 200,
    };
    ballot
        .clone()
        .execute(owner, tx)
        .expect("ballot should lock funds");
    let slash = iroha_data_model::isi::governance::SlashGovernanceLock {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount: 4_u64.into(),
        reason: "policy_violation".to_string(),
    };
    slash
        .clone()
        .execute(owner, tx)
        .expect("slash should succeed");
    let restitute = iroha_data_model::isi::governance::RestituteGovernanceLock {
        referendum_id: referendum_id.to_string(),
        owner: owner.clone(),
        amount: 2_u64.into(),
        reason: "appeal".to_string(),
    };
    restitute
        .execute(owner, tx)
        .expect("restitution should succeed");
}
#[test]
fn manual_slash_and_restitution_move_bonds_and_record_ledger() {
    let (receiver_id, _) = gen_account_in("wonderland");
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let state = setup_state(&def_id, &receiver_id);
    // Direct retained-custody movements share the execution witness recorder;
    // the State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    let alice_id = ALICE_ID.clone();
    let referendum_id = "rid-slash";
    // The first ordinary block after the authenticated genesis.
    let mut sblock = state.block(ordinary_header(&state, 2));
    let mut stx = sblock.transaction_for_fastpq_testing(governance_call());
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
    assert_eq!(escrow_balance.into_inner(), Quantity::from(8_u64));
    assert_eq!(receiver_balance.into_inner(), Quantity::from(2_u64));
    let locks = sblock
        .world
        .governance_locks()
        .get(referendum_id)
        .expect("locks after slash");
    let rec = locks.locks.get(&alice_id).expect("alice lock after slash");
    assert_eq!(rec.amount, Quantity::from(8_u64));
    assert_eq!(rec.slashed, Quantity::from(2_u64));
    let ledger = sblock
        .world
        .governance_slashes()
        .get(referendum_id)
        .expect("slash ledger");
    let entry = ledger.slashes.get(&alice_id).expect("slash ledger entry");
    assert_eq!(entry.total_slashed, Quantity::from(4_u64));
    assert_eq!(entry.total_restituted, Quantity::from(2_u64));
    assert_eq!(
        entry.last_reason,
        iroha_data_model::events::data::governance::GovernanceSlashReason::Restitution
    );
}

#[test]
fn frozen_units_reject_fractional_slash_and_accept_majority_and_full_restitution() {
    use iroha_data_model::isi::governance::{
        CastPlainBallot, RestituteGovernanceLock, SlashGovernanceLock,
    };
    let (receiver, _) = gen_account_in("wonderland");
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "slash".parse().unwrap(),
    );
    let state = setup_state(&definition, &receiver);
    // The State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    let id = "frozen-slash-units";
    // The first ordinary block after the authenticated genesis.
    let ballot_height = 2;
    let mut block = state.block(ordinary_header(&state, ballot_height));
    let mut tx = block.transaction_for_fastpq_testing(governance_call());
    grant_governance_perms(&mut tx, id, &ALICE_ID);
    seed_plain_referendum(&mut tx, id);
    CastPlainBallot {
        referendum_id: id.into(),
        direction: 0,
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
        duration_blocks: 200,
    }
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    let balance = |tx: &iroha_core::state::StateTransaction<'_, '_>, who: &AccountId| -> Quantity {
        tx.world
            .assets()
            .get(&AssetId::new(definition.clone(), who.clone()))
            .map_or_else(Quantity::zero, |v| v.clone().into_inner())
    };
    let before = [
        balance(&tx, &ALICE_ID),
        balance(&tx, &BOB_ID),
        balance(&tx, &receiver),
    ];
    assert!(
        SlashGovernanceLock {
            referendum_id: id.into(),
            owner: ALICE_ID.clone(),
            amount: "0.5".parse().unwrap(),
            reason: "policy_violation".into()
        }
        .execute(&ALICE_ID, &mut tx)
        .is_err(),
        "live unrestricted asset precision cannot override frozen whole units"
    );
    assert_eq!(
        before,
        [
            balance(&tx, &ALICE_ID),
            balance(&tx, &BOB_ID),
            balance(&tx, &receiver)
        ]
    );
    for (slash, remaining, total_slashed) in [(6_u64, 4_u64, 6_u64), (4, 0, 10)] {
        SlashGovernanceLock {
            referendum_id: id.into(),
            owner: ALICE_ID.clone(),
            amount: slash.into(),
            reason: "policy_violation".into(),
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        let record = tx.world.governance_referenda().get(id).unwrap();
        let locks = tx.world.governance_locks().get(id).unwrap();
        let lock = locks.locks.get(&ALICE_ID).unwrap();
        assert_eq!(lock.amount, remaining.into());
        assert_eq!(lock.slashed, total_slashed.into());
        lock.validate_plain_context(record.plain_policy().unwrap())
            .unwrap();
        let restored: iroha_core::state::GovernanceLocksForReferendum =
            norito::decode_canonical(&norito::encode_canonical(locks).unwrap()).unwrap();
        assert!(
            iroha_core::state::plain_governance_tally(record, Some(&restored), ballot_height)
                .is_ok()
        );
        assert_eq!(balance(&tx, &BOB_ID), remaining.into());
        assert_eq!(balance(&tx, &receiver), total_slashed.into());
    }
    assert!(
        RestituteGovernanceLock {
            referendum_id: id.into(),
            owner: ALICE_ID.clone(),
            amount: "0.5".parse().unwrap(),
            reason: "appeal".into()
        }
        .execute(&ALICE_ID, &mut tx)
        .is_err()
    );
    RestituteGovernanceLock {
        referendum_id: id.into(),
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
        reason: "appeal".into(),
    }
    .execute(&ALICE_ID, &mut tx)
    .unwrap();
    assert_eq!(balance(&tx, &ALICE_ID), 990_u64.into());
    assert_eq!(balance(&tx, &BOB_ID), 10_u64.into());
    assert_eq!(balance(&tx, &receiver), 0_u64.into());
    let record = tx.world.governance_referenda().get(id).unwrap();
    assert_eq!(
        iroha_core::state::plain_governance_tally(
            record,
            tx.world.governance_locks().get(id),
            ballot_height
        )
        .unwrap(),
        [9, 0, 0]
    );
}

#[test]
fn restitution_rechecks_aggregate_capacity_and_preserves_closed_decision() {
    use iroha_core::state::plain_governance_tally;
    use iroha_data_model::{
        governance::conviction::PlainVotingResultV1,
        isi::{
            Mint,
            governance::{CastPlainBallot, RestituteGovernanceLock, SlashGovernanceLock},
        },
    };
    let (receiver, _) = gen_account_in("wonderland");
    let (second, _) = gen_account_in("wonderland");
    let (third, _) = gen_account_in("wonderland");
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "capacity".parse().unwrap(),
    );
    let state = setup_state_with(&definition, &receiver, |governance| {
        governance.conviction_step_blocks = 1;
        governance.max_conviction = u64::MAX;
    });
    // The State, whose signed genesis records its own witness, is built first.
    let _witness_guard = iroha_core::exec_witness::exec_witness_guard();
    let id = "restitution-capacity";
    let bond = Quantity::from(1_u128 << 126);
    // The first ordinary block after genesis is H=2; its ballots lock through u64::MAX - 1.
    let ballot_height = 2;
    let duration = u64::MAX - 1 - ballot_height;
    let ballot = |owner: &AccountId| CastPlainBallot {
        referendum_id: id.into(),
        direction: 0,
        owner: owner.clone(),
        amount: bond.clone(),
        duration_blocks: duration,
    };
    let slash = |owner: &AccountId| SlashGovernanceLock {
        referendum_id: id.into(),
        owner: owner.clone(),
        amount: bond.clone(),
        reason: "capacity_test".into(),
    };
    let restitution = RestituteGovernanceLock {
        referendum_id: id.into(),
        owner: ALICE_ID.clone(),
        amount: bond.clone(),
        reason: "appeal".into(),
    };
    let balance = |tx: &iroha_core::state::StateTransaction<'_, '_>, owner: &AccountId| {
        tx.world
            .assets()
            .get(&AssetId::new(definition.clone(), owner.clone()))
            .map_or_else(Quantity::zero, |value| value.clone().into_inner())
    };
    let weight;
    {
        let mut block = state.block(ordinary_header(&state, ballot_height));
        let mut tx = block.transaction_for_fastpq_testing(governance_call());
        grant_governance_perms(&mut tx, id, &ALICE_ID);
        seed_plain_referendum(&mut tx, id);
        Mint::asset_quantity(
            bond.clone(),
            AssetId::new(definition.clone(), ALICE_ID.clone()),
        )
        .execute(&ALICE_ID, &mut tx)
        .unwrap();
        for voter in [&second, &third] {
            iroha_core::query::standalone_plain_test_fixture::fund_voter(
                &mut tx,
                voter,
                bond.clone(),
                0,
            );
            Grant::account_permission(
                Permission::from(CanSubmitGovernanceBallot {
                    referendum_id: id.into(),
                }),
                voter.clone(),
            )
            .execute(&ALICE_ID, &mut tx)
            .unwrap();
        }
        let policy = tx
            .world
            .governance_referenda()
            .get(id)
            .unwrap()
            .plain_policy()
            .unwrap();
        weight = policy.weight(&bond, duration).unwrap();
        assert!(weight.checked_mul(2).is_some());
        assert!(weight.checked_mul(3).is_none());
        ballot(&ALICE_ID).execute(&ALICE_ID, &mut tx).unwrap();
        slash(&ALICE_ID).execute(&ALICE_ID, &mut tx).unwrap();
        ballot(&second).execute(&second, &mut tx).unwrap();
        ballot(&third).execute(&third, &mut tx).unwrap();
        let before_balances = [
            balance(&tx, &ALICE_ID),
            balance(&tx, &BOB_ID),
            balance(&tx, &receiver),
        ];
        assert_eq!(
            before_balances,
            [1_000_u64.into(), bond.try_add(&bond).unwrap(), bond.clone()]
        );
        assert!(balance(&tx, &second).is_zero() && balance(&tx, &third).is_zero());
        let before_locks =
            norito::encode_canonical(tx.world.governance_locks().get(id).unwrap()).unwrap();
        let before_ledger =
            norito::encode_canonical(tx.world.governance_slashes().get(id).unwrap()).unwrap();
        let error = restitution.clone().execute(&ALICE_ID, &mut tx).unwrap_err();
        assert!(error.to_string().contains("u128"), "{error}");
        assert_eq!(
            before_balances,
            [
                balance(&tx, &ALICE_ID),
                balance(&tx, &BOB_ID),
                balance(&tx, &receiver)
            ]
        );
        assert_eq!(
            before_locks,
            norito::encode_canonical(tx.world.governance_locks().get(id).unwrap()).unwrap()
        );
        assert_eq!(
            before_ledger,
            norito::encode_canonical(tx.world.governance_slashes().get(id).unwrap()).unwrap()
        );
        assert_eq!(
            plain_governance_tally(
                tx.world.governance_referenda().get(id).unwrap(),
                tx.world.governance_locks().get(id),
                ballot_height
            )
            .unwrap(),
            [weight * 2, 0, 0]
        );
        // Free headroom again, then let the real block-start transition retain its result.
        slash(&second).execute(&ALICE_ID, &mut tx).unwrap();
        tx.apply();
        block.commit_world_overlay_for_testing().unwrap();
    }
    // The referendum window ends at H=6, so H=7 starts with the real closing transition.
    let close_height = 7;
    let mut block = state.block(ordinary_header(&state, close_height));
    let mut tx = block.transaction_for_fastpq_testing(governance_call());
    let closed = tx.world.governance_referenda().get(id).unwrap().clone();
    assert!(matches!(
        closed.plain_result,
        PlainVotingResultV1::Decided(_)
    ));
    assert_eq!(
        plain_governance_tally(&closed, tx.world.governance_locks().get(id), close_height).unwrap(),
        [weight, 0, 0]
    );
    restitution.execute(&ALICE_ID, &mut tx).unwrap();
    assert_eq!(*tx.world.governance_referenda().get(id).unwrap(), closed);
    assert_eq!(
        plain_governance_tally(&closed, tx.world.governance_locks().get(id), close_height).unwrap(),
        [weight, 0, 0]
    );
    assert_eq!(balance(&tx, &BOB_ID), bond.try_add(&bond).unwrap());
    assert_eq!(balance(&tx, &receiver), bond);
    assert_eq!(balance(&tx, &ALICE_ID), 1_000_u64.into());
}
