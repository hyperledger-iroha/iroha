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
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal, ParliamentBodies,
        ParliamentBody, ParliamentRoster, ProposalKind,
    },
    isi::governance::{RegisterCitizen, UnregisterCitizen},
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::{
    CanManageParliament, CanSubmitGovernanceBallot,
};
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
fn active_parliament_snapshot_retains_citizenship_bond_until_referendum_closes() {
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

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();
    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 50_u64.into(),
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("citizen bond succeeds");

    let proposal_id = [0xA5; 32];
    let referendum_id = hex::encode(proposal_id);
    let body = ParliamentBody::PolicyJury;
    let bodies = ParliamentBodies {
        selection_epoch: 1,
        rosters: std::collections::BTreeMap::from([(
            body,
            ParliamentRoster {
                body,
                epoch: 1,
                members: vec![ALICE_ID.clone()],
                alternates: Vec::new(),
                candidate_count: 1,
                derived_by: Default::default(),
            },
        )]),
    };
    tx.world.governance_proposals_mut().insert(
        proposal_id,
        iroha_core::state::GovernanceProposalRecord {
            proposer: BOB_ID.clone(),
            kind: ProposalKind::DeployContract(DeployContractProposal {
                contract_address: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                    .parse()
                    .expect("contract address"),
                code_hash_hex: ContractCodeHash::new([0x11; 32]),
                abi_hash_hex: ContractAbiHash::new([0x22; 32]),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
            pipeline: iroha_core::state::GovernancePipeline::default(),
            parliament_snapshot: Some(iroha_core::state::GovernanceParliamentSnapshot {
                selection_epoch: 1,
                beacon: [0x33; 32],
                roster_root: [0x44; 32],
                bodies,
            }),
            finalization_evidence: None,
            enacted_at_height: None,
        },
    );
    tx.world.governance_referenda_mut().insert(
        referendum_id.clone(),
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            h_end: 10,
            status: iroha_core::state::GovernanceReferendumStatus::Open,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );

    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id, BOB_ID.clone());
    let alice_before = tx
        .world
        .assets()
        .get(&alice_asset_id)
        .expect("Alice asset")
        .0
        .clone();
    let escrow_before = tx
        .world
        .assets()
        .get(&escrow_asset_id)
        .expect("escrow asset")
        .0
        .clone();
    let err = UnregisterCitizen {
        owner: ALICE_ID.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .expect_err("active Parliament service must retain citizenship collateral");
    assert!(
        format!("{err:?}").contains("active governance snapshot"),
        "unexpected unbond error: {err:?}"
    );
    assert!(tx.world.citizens().get(&*ALICE_ID).is_some());
    assert_eq!(
        tx.world
            .assets()
            .get(&alice_asset_id)
            .expect("Alice asset")
            .0,
        alice_before
    );
    assert_eq!(
        tx.world
            .assets()
            .get(&escrow_asset_id)
            .expect("escrow asset")
            .0,
        escrow_before
    );

    tx.world.governance_referenda_mut().insert(
        referendum_id,
        iroha_core::state::GovernanceReferendumRecord {
            h_start: 1,
            h_end: 10,
            status: iroha_core::state::GovernanceReferendumStatus::Closed,
            mode: iroha_core::state::GovernanceReferendumMode::Plain,
        },
    );
    UnregisterCitizen {
        owner: ALICE_ID.clone(),
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("citizenship collateral releases after referendum closure");
    assert!(tx.world.citizens().get(&*ALICE_ID).is_none());
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
    Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx)
        .expect("grant parliament management permission");
    // Council persistence should fail without a citizen bond.
    let council_res = iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
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
    iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
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
        .commit()
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
    let mut stx_2 = block_2.transaction();
    Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
        .execute(&ALICE_ID, &mut stx_2)
        .expect("grant parliament management permission");
    iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
    }
    .execute(&ALICE_ID, &mut stx_2)
    .expect("persist council should succeed when citizen record persisted");
}
#[test]
fn citizenship_top_up_preserves_the_original_bond_interval_and_service_state() {
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
        .commit()
        .expect("initial citizen bond block commits");
    let mut block_2 = state.block(BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0));
    let mut stx_2 = block_2.transaction();
    let mut serviced = stx_2
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("persisted citizen record");
    serviced.seats_in_epoch = 2;
    serviced.last_epoch_seen = 7;
    serviced.cooldown_until = 42;
    serviced.declines_used = 1;
    serviced.no_show_strikes = 3;
    serviced.misconduct_strikes = 4;
    stx_2
        .world
        .citizens_mut()
        .insert(ALICE_ID.clone(), serviced);
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
    assert_eq!(retained.seats_in_epoch, 2);
    assert_eq!(retained.last_epoch_seen, 7);
    assert_eq!(retained.cooldown_until, 42);
    assert_eq!(retained.declines_used, 1);
    assert_eq!(retained.no_show_strikes, 3);
    assert_eq!(retained.misconduct_strikes, 4);
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
