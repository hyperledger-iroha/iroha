//! Citizenship bonding and gating tests for governance flows.
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
    domain::{Domain, DomainId},
    isi::governance::{RegisterCitizen, UnregisterCitizen},
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_primitives::numeric::Quantity;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
fn build_world(def_id: &AssetDefinitionId) -> World {
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account = iroha_data_model::account::Account::new(ALICE_ID.clone()).build(&ALICE_ID);
    let escrow_account = iroha_data_model::account::Account::new(BOB_ID.clone()).build(&BOB_ID);
    let asset_def = AssetDefinition::numeric(
        def_id.clone(),
        "xor".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&ALICE_ID);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Quantity::from(1_000_u64),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Quantity::from(0_u64),
    );
    World::with_assets(
        [domain],
        [alice_account, escrow_account],
        [asset_def],
        [alice_asset, escrow_asset],
        [],
    )
}
#[test]
fn register_and_revoke_citizenship_moves_bond() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 50_u64.into();
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen bond succeeds");
    let record = stx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    assert_eq!(record.owner, *ALICE_ID);
    assert_eq!(record.amount, Quantity::from(50_u64));
    assert_eq!(record.bonded_height, 1);
    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Quantity::from(950_u64)
    );
    assert_eq!(
        **stx.world.asset_mut(&escrow_asset_id).expect("escrow asset"),
        Quantity::from(50_u64)
    );
    UnregisterCitizen {
        owner: ALICE_ID.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen unbond succeeds");
    assert!(stx.world.citizens().get(&*ALICE_ID).is_none());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Quantity::from(1_000_u64)
    );
    assert!(stx.world.assets().get(&escrow_asset_id).is_none());
}

#[test]
fn citizenship_gate_blocks_and_allows_governance() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 10_u64.into();
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0_u64.into();
    gov_cfg.conviction_step_blocks = 10;
    state.set_gov(gov_cfg);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    // Seed referendum and permissions.
    stx.world.governance_referenda_mut().insert(
        "citizen-ref".to_string(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 0,
            // The H=1 ballot below locks through H=21; keep the referendum's
            // inclusive end within that exact lock lifetime.
            h_end: 21,
            status: iroha_core::state::GovernanceReferendumStatus::Proposed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
    let ballot_perm: Permission = CanSubmitGovernanceBallot {
        referendum_id: "citizen-ref".to_string(),
    }
    .into();
    Grant::account_permission(ballot_perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant ballot permission");
    // Ballot should be rejected until citizenship is bonded.
    let ballot = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: "citizen-ref".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
        duration_blocks: 20,
        direction: 0,
    };
    let ballot_err = ballot.clone().execute(&ALICE_ID, &mut stx).unwrap_err();
    assert!(format!("{ballot_err:?}").contains("citizenship bond required"));
    let rejected = stx.world.take_external_events();
    assert!(rejected.iter().any(|event| matches!(
        event.as_data_event(),
        Some(iroha_data_model::events::data::DataEvent::Governance(
            iroha_data_model::events::data::governance::GovernanceEvent::BallotRejected(rej)
        )) if rej.referendum_id == "citizen-ref" && rej.reason.contains("citizenship")
    )));
    // Bond citizenship and retry.
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 10_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen bond succeeds");
    ballot
        .execute(&ALICE_ID, &mut stx)
        .expect("ballot allowed once citizen bonded");
    assert!(
        stx.world
            .citizens()
            .get(&*ALICE_ID)
            .is_some_and(|rec| rec.amount >= Quantity::from(10_u64))
    );
    assert!(stx.world.governance_locks().get("citizen-ref").is_some());
}
#[test]
fn citizenship_records_persist_across_transactions() {
    let def_id: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "xor".parse().unwrap(),
        );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 50_u64.into();
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    state.set_gov(gov_cfg);
    let header_1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block_1 = state.block(header_1);
    let mut stx_1 = block_1.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_1)
    .expect("citizen bond succeeds");
    stx_1.apply();
    block_1
        .commit_world_overlay_for_testing()
        .expect("first block with citizen registration commits");
    let citizen_record = state
        .view()
        .world()
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record should persist after tx apply");
    assert_eq!(citizen_record.amount, Quantity::from(50_u64));
    let header_2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block_2 = state.block(header_2);
    let stx_2 = block_2.transaction();
    assert_eq!(
        stx_2
            .world
            .citizens()
            .get(&*ALICE_ID)
            .expect("citizen record remains visible in the next transaction")
            .amount,
        Quantity::from(50_u64)
    );
}
#[test]
fn citizenship_top_up_preserves_the_original_bond_interval() {
    let def_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain"),
        "xor".parse().expect("asset name"),
    );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);
    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 50_u64.into();
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    state.set_gov(gov_cfg);
    let mut block_1 = state.block(BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0));
    let mut stx_1 = block_1.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_1)
    .expect("initial citizen bond succeeds");
    stx_1.apply();
    block_1
        .commit_world_overlay_for_testing()
        .expect("initial citizen bond block commits");
    let mut block_2 = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut stx_2 = block_2.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 75_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("citizenship top-up succeeds");
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 75_u64.into(),
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("same-amount citizenship registration is an idempotent no-op");
    let retained = stx_2
        .world
        .citizens()
        .get(&*ALICE_ID)
        .expect("topped-up citizen record");
    assert_eq!(retained.amount, Quantity::from(75_u64));
    assert_eq!(retained.bonded_height, 1);
    assert_eq!(
        **stx_2
            .world
            .asset_mut(&AssetId::new(def_id.clone(), ALICE_ID.clone()))
            .expect("alice citizenship asset"),
        Quantity::from(925_u64)
    );
    assert_eq!(
        **stx_2
            .world
            .asset_mut(&AssetId::new(def_id, BOB_ID.clone()))
            .expect("citizenship escrow asset"),
        Quantity::from(75_u64)
    );
}
