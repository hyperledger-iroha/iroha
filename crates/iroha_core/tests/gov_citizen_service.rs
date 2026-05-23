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
    domain::{Domain, DomainId},
    isi::governance::{
        CitizenServiceEvent, CouncilDerivationKind, PersistCouncilForEpoch,
        RecordCitizenServiceOutcome, RegisterCitizen,
    },
    permission::Permission,
    prelude::{AssetDefinitionId, AssetId, Grant},
};
use iroha_executor_data_model::permission::governance::{
    CanManageParliament, CanRecordCitizenService,
};
use iroha_primitives::numeric::Numeric;
use iroha_test_samples::{ALICE_ID, BOB_ID, CARPENTER_ID};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn build_world(def_id: &AssetDefinitionId) -> World {
    let alice_id = ALICE_ID.clone();
    let bob_id = BOB_ID.clone();
    let carpenter_id = CARPENTER_ID.clone();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&alice_id);
    let alice_account = iroha_data_model::account::Account::new(alice_id.clone()).build(&alice_id);
    let escrow_account = iroha_data_model::account::Account::new(bob_id.clone()).build(&bob_id);
    let carpenter_account =
        iroha_data_model::account::Account::new(carpenter_id.clone()).build(&carpenter_id);
    let asset_def = AssetDefinition::numeric(def_id.clone()).build(&alice_id);
    let alice_asset = Asset::new(
        AssetId::new(def_id.clone(), ALICE_ID.clone()),
        Numeric::new(1_000, 0),
    );
    let carpenter_asset = Asset::new(
        AssetId::new(def_id.clone(), CARPENTER_ID.clone()),
        Numeric::new(1_000, 0),
    );
    let escrow_asset = Asset::new(
        AssetId::new(def_id.clone(), BOB_ID.clone()),
        Numeric::new(0, 0),
    );

    World::with_assets(
        [domain],
        [alice_account, escrow_account, carpenter_account],
        [asset_def],
        [alice_asset, carpenter_asset, escrow_asset],
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

fn xor_definition_id() -> AssetDefinitionId {
    iroha_data_model::asset::AssetDefinitionId::new(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "xor".parse().unwrap(),
    )
}

#[test]
fn council_persist_enforces_service_discipline() {
    let def_id: AssetDefinitionId = xor_definition_id();
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

        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");

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

        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");

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
fn council_persist_requires_manage_permission_before_mutating_state() {
    let def_id: AssetDefinitionId = xor_definition_id();
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

    let before = tx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    let err = PersistCouncilForEpoch {
        epoch: 1,
        members: vec![ALICE_ID.clone()],
        alternates: Vec::new(),
        verified: 0,
        candidates_count: 1,
        derived_by: CouncilDerivationKind::Fallback,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect_err("permissionless council persist must fail");
    assert!(
        format!("{err:?}").contains("CanManageParliament"),
        "unexpected permission error: {err:?}"
    );
    let after = tx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record still stored");
    assert_eq!(
        before, after,
        "failed council persistence must not consume seat quota or set cooldown"
    );
    assert!(
        tx.world.council().get(&1).is_none(),
        "permissionless persist must not write council state"
    );
    assert!(
        tx.world.parliament_bodies().get(&1).is_none(),
        "permissionless persist must not derive body rosters"
    );
}

#[test]
fn council_persist_rejects_unregistered_and_underbonded_roster_entries() {
    let def_id: AssetDefinitionId = xor_definition_id();

    let unregistered_err = {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");
        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("unregistered member must fail");
        assert!(tx.world.council().get(&1).is_none());
        err
    };
    assert!(
        format!("{unregistered_err:?}").contains("registered citizens"),
        "unexpected unregistered-member error: {unregistered_err:?}"
    );

    let underbonded_member_err = {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 19,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("underbonded citizen registration succeeds");
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");
        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("underbonded member must fail");
        let record = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");
        assert_eq!(record.amount, 19);
        assert_eq!(record.seats_in_epoch, 0);
        assert!(tx.world.council().get(&1).is_none());
        err
    };
    assert!(
        format!("{underbonded_member_err:?}").contains("bond floor"),
        "unexpected underbonded-member error: {underbonded_member_err:?}"
    );

    let underbonded_alternate_err = {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 25,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("member bond succeeds");
        RegisterCitizen {
            owner: CARPENTER_ID.clone(),
            amount: 10,
        }
        .execute(&CARPENTER_ID, &mut tx)
        .expect("alternate citizen registration succeeds");
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");
        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: vec![CARPENTER_ID.clone()],
            verified: 0,
            candidates_count: 2,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("underbonded alternate must fail");
        assert!(
            tx.world.council().get(&1).is_none(),
            "failed alternate validation must not write council state"
        );
        err
    };
    assert!(
        format!("{underbonded_alternate_err:?}").contains("bond floor"),
        "unexpected underbonded-alternate error: {underbonded_alternate_err:?}"
    );
}

#[test]
fn council_persist_rejects_duplicate_or_overlapping_roster_entries_without_seat_use() {
    let def_id: AssetDefinitionId = xor_definition_id();

    {
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
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");

        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone(), ALICE_ID.clone()],
            alternates: Vec::new(),
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("duplicate member must fail");
        assert!(
            format!("{err:?}").contains("duplicate member"),
            "unexpected duplicate-member error: {err:?}"
        );
        let record = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");
        assert_eq!(record.seats_in_epoch, 0);
        assert!(tx.world.council().get(&1).is_none());
        assert!(tx.world.parliament_bodies().get(&1).is_none());
    }

    {
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
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");

        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: vec![ALICE_ID.clone()],
            verified: 0,
            candidates_count: 1,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("member/alternate overlap must fail");
        assert!(
            format!("{err:?}").contains("both member and alternate"),
            "unexpected overlap error: {err:?}"
        );
        let record = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");
        assert_eq!(record.seats_in_epoch, 0);
        assert!(tx.world.council().get(&1).is_none());
        assert!(tx.world.parliament_bodies().get(&1).is_none());
    }

    {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 25,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("member bond succeeds");
        RegisterCitizen {
            owner: CARPENTER_ID.clone(),
            amount: 25,
        }
        .execute(&CARPENTER_ID, &mut tx)
        .expect("alternate bond succeeds");
        Grant::account_permission(Permission::from(CanManageParliament), ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant parliament management permission");

        let err = PersistCouncilForEpoch {
            epoch: 1,
            members: vec![ALICE_ID.clone()],
            alternates: vec![CARPENTER_ID.clone(), CARPENTER_ID.clone()],
            verified: 0,
            candidates_count: 2,
            derived_by: CouncilDerivationKind::Fallback,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("duplicate alternate must fail");
        assert!(
            format!("{err:?}").contains("duplicate alternate"),
            "unexpected duplicate-alternate error: {err:?}"
        );
        for account_id in [&*ALICE_ID, &*CARPENTER_ID] {
            let record = tx
                .world
                .citizens()
                .get(account_id)
                .cloned()
                .expect("citizen record stored");
            assert_eq!(record.seats_in_epoch, 0);
        }
        assert!(tx.world.council().get(&1).is_none());
        assert!(tx.world.parliament_bodies().get(&1).is_none());
    }
}

#[test]
fn citizen_registration_rejects_authority_mismatch_without_bond_transfer() {
    let def_id: AssetDefinitionId = xor_definition_id();
    let state = configure_state(&def_id, 0);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();

    let err = RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 10,
    }
    .execute(&BOB_ID, &mut tx)
    .expect_err("non-owner must not register citizen bond for another account");
    assert!(
        format!("{err:?}").contains("owner must equal authority"),
        "unexpected authority-mismatch error: {err:?}"
    );
    assert!(
        tx.world.citizens().get(&*ALICE_ID).is_none(),
        "authority mismatch must not create a citizen record"
    );

    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **tx.world
            .asset_mut(&alice_asset_id)
            .expect("alice asset present"),
        Numeric::new(1_000, 0),
        "failed registration must not withdraw from the owner's account"
    );
    assert_eq!(
        **tx.world
            .asset_mut(&escrow_asset_id)
            .expect("escrow asset present"),
        Numeric::new(0, 0),
        "failed registration must not deposit escrow collateral"
    );
}

#[test]
fn citizen_bond_decrease_is_rejected_without_releasing_collateral() {
    let def_id: AssetDefinitionId = xor_definition_id();
    let state = configure_state(&def_id, 0);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut tx = block.transaction();

    RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 25,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect("initial citizen bond succeeds");

    let err = RegisterCitizen {
        owner: ALICE_ID.clone(),
        amount: 20,
    }
    .execute(&ALICE_ID, &mut tx)
    .expect_err("bond reduction must fail");
    assert!(
        format!("{err:?}").contains("cannot decrease"),
        "unexpected bond-decrease error: {err:?}"
    );

    let record = tx
        .world
        .citizens()
        .get(&*ALICE_ID)
        .cloned()
        .expect("citizen record stored");
    assert_eq!(record.amount, 25);
    let alice_asset_id = AssetId::new(def_id.clone(), ALICE_ID.clone());
    let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
    assert_eq!(
        **tx.world
            .asset_mut(&alice_asset_id)
            .expect("alice asset present"),
        Numeric::new(975, 0),
        "rejected bond decrease must not refund owner funds"
    );
    assert_eq!(
        **tx.world
            .asset_mut(&escrow_asset_id)
            .expect("escrow asset present"),
        Numeric::new(25, 0),
        "rejected bond decrease must keep the original collateral locked"
    );
}

#[test]
fn service_outcome_rejections_do_not_mutate_citizen_bond_or_counters() {
    let def_id: AssetDefinitionId = xor_definition_id();

    {
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
        let before = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");

        let err = RecordCitizenServiceOutcome {
            owner: ALICE_ID.clone(),
            epoch: 1,
            role: "council".to_string(),
            event: CitizenServiceEvent::NoShow,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("service record without permission must fail");
        assert!(
            format!("{err:?}").contains("CanRecordCitizenService"),
            "unexpected permission error: {err:?}"
        );
        let after = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record still stored");
        assert_eq!(
            before, after,
            "permissionless service outcome must not slash or update counters"
        );
    }

    {
        let state = configure_state(&def_id, 0);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();

        RegisterCitizen {
            owner: ALICE_ID.clone(),
            amount: 10,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect("minimum citizen bond succeeds");
        let perm: Permission = CanRecordCitizenService {
            owner: ALICE_ID.clone(),
        }
        .into();
        Grant::account_permission(perm, ALICE_ID.clone())
            .execute(&ALICE_ID, &mut tx)
            .expect("grant service discipline permission");

        let err = RecordCitizenServiceOutcome {
            owner: ALICE_ID.clone(),
            epoch: 1,
            role: "council".to_string(),
            event: CitizenServiceEvent::NoShow,
        }
        .execute(&ALICE_ID, &mut tx)
        .expect_err("underbonded citizen must not be slashed for a role they cannot serve");
        assert!(
            format!("{err:?}").contains("below role requirement"),
            "unexpected underbonded service error: {err:?}"
        );
        let record = tx
            .world
            .citizens()
            .get(&*ALICE_ID)
            .cloned()
            .expect("citizen record stored");
        assert_eq!(record.amount, 10);
        assert_eq!(record.no_show_strikes, 0);
        let escrow_asset_id = AssetId::new(def_id.clone(), BOB_ID.clone());
        assert_eq!(
            **tx.world
                .asset_mut(&escrow_asset_id)
                .expect("escrow asset present"),
            Numeric::new(10, 0),
            "failed underbonded service record must not slash collateral"
        );
    }
}

#[test]
fn citizen_service_outcome_slashes_after_free_decline() {
    let def_id: AssetDefinitionId = xor_definition_id();
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
