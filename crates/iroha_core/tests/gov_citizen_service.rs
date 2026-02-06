//! Citizen service discipline enforcement for governance roles.
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
    isi::governance::{
        CitizenServiceEvent, CouncilDerivationKind, PersistCouncilForEpoch,
        RecordCitizenServiceOutcome, RegisterCitizen,
    },
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::CanRecordCitizenService;
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn build_world(def_id: &AssetDefinitionId) -> World {
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let domain = Domain::new("wonderland".parse().expect("domain")).build(&alice_id);
    let alice_account = iroha_data_model::account::Account::new(alice_id.clone()).build(&alice_id);
    let escrow_account = iroha_data_model::account::Account::new(bob_id.clone()).build(&bob_id);
    let asset_def = AssetDefinition::numeric(def_id.clone()).build(&alice_id);
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

fn configure_state(def_id: &AssetDefinitionId, seat_cooldown_blocks: u64) -> State {
    let world = build_world(def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 10;
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    gov_cfg.bond_escrow_account = BOB_ID.clone();
    gov_cfg.slash_receiver_account = BOB_ID.clone();
    gov_cfg.citizen_service.seat_cooldown_blocks = seat_cooldown_blocks;
    gov_cfg.citizen_service.max_seats_per_epoch = 1;
    gov_cfg
        .citizen_service
        .role_bond_multipliers
        .insert("council".to_string(), 2);
    state.set_gov(gov_cfg);
    state
}

#[test]
fn council_persist_enforces_service_discipline() {
    let def_id: AssetDefinitionId = "xor#wonderland".parse().expect("asset id");
    // Seat cap when cooldown is disabled.
    let seat_err = {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 25,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("citizen bond succeeds");

        PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("first council persist succeeds");

        let record = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");
        assert_eq!(record.seats_in_epoch, 1);
        assert!(record.cooldown_until > 0, "cooldown applied");

        PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err()
    };
    assert!(
        format!("{seat_err:?}").contains("seat limit"),
        "seat cap enforced"
    );

    // Cooldown guard when a validator tries to re-enter before the cooldown elapses.
    let cooldown_err = {
        let state = configure_state(&def_id, 5);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 25,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("citizen bond succeeds");

        PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("first council persist succeeds");

        PersistCouncilForEpoch {
            epoch: 2,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .unwrap_err()
    };
    assert!(
        format!("{cooldown_err:?}").contains("cooldown"),
        "cooldown blocks subsequent epoch"
    );
}

#[test]
fn citizen_service_outcome_slashes_after_free_decline() {
    let def_id: AssetDefinitionId = "xor#wonderland".parse().expect("asset id");
    let world = build_world(&def_id);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query_handle);

    let mut gov_cfg = state.gov.clone();
    gov_cfg.citizenship_asset_id = def_id.clone();
    gov_cfg.citizenship_bond_amount = 10;
    gov_cfg.citizenship_escrow_account = BOB_ID.clone();
    gov_cfg.bond_escrow_account = BOB_ID.clone();
    gov_cfg.slash_receiver_account = ALICE_ID.clone();
    gov_cfg.citizen_service.decline_slash_bps = 500;
    gov_cfg.citizen_service.free_declines_per_epoch = 1;
    gov_cfg.citizen_service.no_show_slash_bps = 1_000;
    state.set_gov(gov_cfg);

    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();

    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 100,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("citizen bond succeeds");

    let perm: Permission = CanRecordCitizenService {
        owner: ALICE_ID.clone(),
    }
    .into();
    Grant::account_permission(perm, ALICE_ID.clone())
        .execute(&ALICE_ID, &mut tx)
        .expect("grant service discipline permission");

    RecordCitizenServiceOutcome {
        owner: ALICE_ID.clone(),
        epoch: 1,
        role: "council".to_string(),
        event: CitizenServiceEvent::Decline,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("first decline allowed without slash");

    let record = tx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    assert_eq!(record.declines_used, 1);
    assert_eq!(record.amount, 100);

    RecordCitizenServiceOutcome {
        owner: ALICE_ID.clone(),
        epoch: 1,
        role: "council".to_string(),
        event: CitizenServiceEvent::Decline,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("second decline slashes");

    let record = tx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    assert_eq!(record.declines_used, 2);
    assert_eq!(record.amount, 95, "bond reduced by slashing");

    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **tx.world
            .asset_mut(&escrow_asset_id)
            .expect("escrow asset present"),
        Numeric::new(95, 0)
    );
}
