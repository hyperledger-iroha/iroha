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
    isi::governance::{CouncilDerivationKind, RegisterCitizen, UnregisterCitizen},
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::CanSubmitGovernanceBallot;
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn build_world(def_id: &AssetDefinitionId) -> World {
    let domain_id: DomainId = "wonderland".parse().expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&ALICE_ID);
    let alice_account =
        iroha_data_model::account::Account::new(ALICE_ID.clone())
            .build(&ALICE_ID);
    let escrow_account =
        iroha_data_model::account::Account::new(BOB_ID.clone()).build(&BOB_ID);
    let asset_def = AssetDefinition::numeric(def_id.clone()).build(&ALICE_ID);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Numeric::new(1_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Numeric::new(0, 0),
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
    let def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 50;
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    state.set_gov(gov_cfg);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();

    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50,
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
    assert_eq!(record.amount, 50);
    assert_eq!(record.bonded_height, 1);

    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Numeric::new(950, 0)
    );
    assert_eq!(
        **stx.world.asset_mut(&escrow_asset_id).expect("escrow asset"),
        Numeric::new(50, 0)
    );

    UnregisterCitizen {
        owner: ALICE_ID.clone(),
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen unbond succeeds");

    assert!(stx.world.citizens().get(&*ALICE_ID).is_none());
    assert_eq!(
        **stx.world.asset_mut(&alice_asset_id).expect("alice asset"),
        Numeric::new(1_000, 0)
    );
    assert!(stx.world.assets().get(&escrow_asset_id).is_none());
}

#[test]
fn citizenship_gate_blocks_and_allows_governance() {
    let def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.voting_asset_id = def_id.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 10;
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    gov_cfg.plain_voting_enabled = true;
    gov_cfg.min_bond_amount = 0;
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
            h_end: 25,
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

    // Council persistence should fail without a citizen bond.
    let council_res = iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
        verified: 0,
        candidates_count: 1,
        derived_by: CouncilDerivationKind::Fallback,
    }
    .execute(&ALICE_ID, &mut stx);
    assert!(matches!(
        council_res,
        Err(iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(_))
    ));

    // Ballot should be rejected until citizenship is bonded.
    let ballot = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: "citizen-ref".to_string(),
        owner: ALICE_ID.clone(),
        amount: 10,
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
        amount: 10,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("citizen bond succeeds");

    iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
        verified: 0,
        candidates_count: 1,
        derived_by: CouncilDerivationKind::Fallback,
    }
    .execute(&ALICE_ID, &mut stx)
    .expect("council persists after citizen bond");

    ballot
        .execute(&ALICE_ID, &mut stx)
        .expect("ballot allowed once citizen bonded");
    assert!(
        stx.world
            .citizens()
            .get(&*ALICE_ID)
            .is_some_and(|rec| rec.amount >= 10)
    );
    assert!(stx.world.governance_locks().get("citizen-ref").is_some());
}

#[test]
fn citizenship_records_persist_across_transactions() {
    let def_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
        "wonderland".parse().unwrap(),
        "xor".parse().unwrap(),
    );
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 50;
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    state.set_gov(gov_cfg);

    let header_1 = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block_1 = state.block(header_1);
    let mut stx_1 = block_1.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50,
    }
    .execute(&ALICE_ID, &mut stx_1)
    .expect("citizen bond succeeds");
    stx_1.apply();
    block_1
        .commit()
        .expect("first block with citizen registration commits");

    let citizen_record = state
        .view()
        .world()
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record should persist after tx apply");
    assert_eq!(citizen_record.amount, 50);

    let header_2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
    let mut block_2 = state.block(header_2);
    let mut stx_2 = block_2.transaction();
    iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
        verified: 0,
        candidates_count: 1,
        derived_by: CouncilDerivationKind::Fallback,
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("persist council should succeed when citizen record persisted");
}
