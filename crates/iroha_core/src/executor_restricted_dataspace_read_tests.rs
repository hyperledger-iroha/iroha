//! Restricted data space read authority changes only through the governed native successor.

use super::*;
use crate::executor::Executor;
use iroha_data_model::{
    account::AccountAddress,
    sns::{NameControllerV1, NameRecordV1},
};

const MODULE: &str = crate::runtime::catalog::additions::RESTRICTED_DATASPACE_READ.module;

fn read_permission(id: u64) -> Permission {
    executor_permission::query::CanReadRestrictedDataspace {
        dataspace: DataSpaceId::new(id),
    }
    .into()
}

fn lease(world: &mut World, alias: &str, id: u64, owner: &AccountId) {
    let selector = crate::sns::selector_for_dataspace_alias(alias).unwrap();
    let address = AccountAddress::from_account_id(owner).unwrap();
    let mut record = NameRecordV1::new(
        selector.clone(),
        owner.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    record.metadata.insert(
        crate::sns::SNS_DATASPACE_ID_METADATA_KEY.parse().unwrap(),
        Json::new(id),
    );
    world
        .smart_contract_state_mut_for_testing()
        .insert(crate::sns::record_storage_key(&selector), record.encode());
}

fn fixture(active: bool) -> (State, AccountId, AccountId, AccountId) {
    let owner = checked_account_id();
    let reader = checked_account_id();
    let other = checked_account_id();
    let mut world = World::with(
        [],
        [&owner, &reader, &other].map(|id| Account::new(id.clone()).build(id)),
        [],
    );
    lease(&mut world, "restricted-bank", 10, &owner);
    lease(&mut world, "other-bank", 11, &other);
    let manage: Permission = executor_permission::role::CanManageRoles.into();
    world
        .account_permissions
        .insert(owner.clone(), BTreeSet::from([manage.clone()]));
    world.account_permissions.insert(
        reader.clone(),
        BTreeSet::from([read_permission(10), manage]),
    );
    if active {
        crate::runtime::activate_additions_for_testing(&world, MODULE).unwrap();
    }
    (state_after_genesis(world), owner, reader, other)
}

fn header(state: &State, time: u64) -> BlockHeader {
    BlockHeader::new(
        nonzero!(2_u64),
        Some(state.view().latest_block_hash().unwrap()),
        None,
        time,
        0,
    )
}

#[test]
fn restricted_read_successor_preserves_baseline_and_requires_each_binding() {
    for active in [false, true] {
        let (state, owner, _, destination) = fixture(active);
        let mut block = state.block(header(&state, 50));
        let mut tx = block.transaction();
        let exact = read_permission(10);
        assert!(
            initial_permission_is_genesis_only(&exact),
            "the baseline remains immutable"
        );
        assert!(!initial_permission_delegation_allowed(&tx, &owner, &exact).unwrap());
        let grant = Grant::account_permission(exact.clone(), destination.clone()).into();
        assert_eq!(
            crate::runtime::restricted_dataspace_read::selected_for(
                tx.world.runtime_selection(),
                &grant
            ),
            active
        );
        let result = Executor::Initial.execute_instruction(&mut tx, &owner, &grant);
        assert_eq!(
            result.is_ok(),
            active,
            "owner must not acquire authority before activation: {result:?}"
        );
        if !active {
            continue;
        }
        // Restoring one predecessor binding cannot inherit the other successor bindings' policy.
        let catalog = crate::runtime::release_catalog().unwrap();
        for wire in ["iroha.grant", "iroha.revoke", "iroha.register"] {
            let successor = tx.world.runtime_selection.get().clone();
            let old = catalog
                .baseline
                .bindings
                .iter()
                .find(|entry| entry.wire_id == wire)
                .unwrap()
                .clone();
            *tx.world
                .runtime_selection
                .get_mut()
                .bindings
                .iter_mut()
                .find(|entry| entry.wire_id == wire)
                .unwrap() = old;
            let instruction: InstructionBox = match wire {
                "iroha.grant" => Grant::account_permission(exact.clone(), owner.clone()).into(),
                "iroha.revoke" => {
                    Revoke::account_permission(exact.clone(), destination.clone()).into()
                }
                _ => Register::role(
                    Role::new("unselected_reader".parse().unwrap(), owner.clone())
                        .add_permission(exact.clone()),
                )
                .into(),
            };
            Executor::Initial
                .execute_instruction(&mut tx, &owner, &instruction)
                .expect_err("unselected mutation must retain baseline policy");
            *tx.world.runtime_selection.get_mut() = successor;
        }
        assert!(authority_has_permission(&tx.world, &destination, &exact).unwrap());
    }
}

#[test]
fn restricted_read_successor_owner_grants_and_revokes_direct_and_role_access() {
    let (state, owner, reader, destination) = fixture(true);
    let mut block = state.block(header(&state, 50));
    let mut tx = block.transaction();
    let exact = read_permission(10);
    assert!(
        !authority_has_permission(&tx.world, &owner, &exact).unwrap(),
        "ownership does not itself grant reads"
    );
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Grant::account_permission(
                executor_permission::query::CanReadAllLedgerData,
                owner.clone(),
            )
            .into(),
        )
        .expect_err("exact data space ownership cannot create global read authority");
    let malformed = Permission::new(
        "CanReadRestrictedDataspace".to_owned(),
        Json::new(norito::json!({"dataspace": 10, "extra": 11})),
    );
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Grant::account_permission(malformed, destination.clone()).into(),
        )
        .expect_err("noncanonical or widened payloads must not acquire read authority");
    for account in [&owner, &destination] {
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &owner,
                &Grant::account_permission(exact.clone(), account.clone()).into(),
            )
            .unwrap();
        assert!(authority_has_permission(&tx.world, account, &exact).unwrap());
        Executor::Initial
            .execute_instruction(
                &mut tx,
                &owner,
                &Revoke::account_permission(exact.clone(), account.clone()).into(),
            )
            .unwrap();
        assert!(!authority_has_permission(&tx.world, account, &exact).unwrap());
    }
    let role: RoleId = "owner_managed_reader".parse().unwrap();
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Register::role(Role::new(role.clone(), owner.clone()).add_permission(exact.clone()))
                .into(),
        )
        .unwrap();
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Grant::account_role(role.clone(), destination.clone()).into(),
        )
        .unwrap();
    assert!(authority_has_permission(&tx.world, &destination, &exact).unwrap());
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Revoke::account_role(role.clone(), destination.clone()).into(),
        )
        .unwrap();
    assert!(!authority_has_permission(&tx.world, &destination, &exact).unwrap());
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Grant::account_role(role.clone(), destination.clone()).into(),
        )
        .unwrap();
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Revoke::role_permission(exact.clone(), role.clone()).into(),
        )
        .unwrap();
    assert!(!authority_has_permission(&tx.world, &destination, &exact).unwrap());
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Grant::role_permission(exact.clone(), role).into(),
        )
        .unwrap();
    assert!(authority_has_permission(&tx.world, &destination, &exact).unwrap());
    assert!(
        authority_has_permission(&tx.world, &reader, &exact).unwrap(),
        "activation preserves an existing genesis read grant"
    );
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Revoke::account_permission(exact.clone(), reader.clone()).into(),
        )
        .unwrap();
    assert!(
        !authority_has_permission(&tx.world, &reader, &exact).unwrap(),
        "owner may revoke a preexisting exact grant"
    );
}

#[test]
fn restricted_read_successor_denies_holder_escalation_in_every_role_path_and_native_dispatch() {
    let (state, owner, reader, destination) = fixture(true);
    let mut block = state.block(header(&state, 50));
    let mut tx = block.transaction();
    let exact = read_permission(10);
    let role: RoleId = "held_reader".parse().unwrap();
    Register::role(Role::new(role.clone(), reader.clone()).add_permission(exact.clone()))
        .execute(&owner, &mut tx)
        .unwrap();
    Grant::account_role(role.clone(), destination.clone())
        .execute(&owner, &mut tx)
        .unwrap();
    let empty: RoleId = "held_empty".parse().unwrap();
    Register::role(Role::new(empty.clone(), reader.clone()))
        .execute(&owner, &mut tx)
        .unwrap();
    let denied: [InstructionBox; 8] = [
        Grant::account_permission(exact.clone(), owner.clone()).into(),
        Revoke::account_permission(exact.clone(), reader.clone()).into(),
        Grant::account_role(role.clone(), owner.clone()).into(),
        Revoke::account_role(role.clone(), destination.clone()).into(),
        Grant::role_permission(exact.clone(), empty.clone()).into(),
        Revoke::role_permission(exact.clone(), role.clone()).into(),
        Unregister::role(role.clone()).into(),
        Register::role(
            Role::new("escalated_reader".parse().unwrap(), reader.clone())
                .add_permission(exact.clone()),
        )
        .into(),
    ];
    for instruction in &denied {
        Executor::Initial
            .execute_instruction(&mut tx, &reader, instruction)
            .expect_err("readers cannot delegate or revoke");
        crate::smartcontracts::isi::execute_borrowed_instruction(instruction, &reader, &mut tx)
            .expect_err("native successor must also guard user-executor dispatch");
    }
    assert!(!authority_has_permission(&tx.world, &owner, &exact).unwrap());
    assert!(authority_has_permission(&tx.world, &reader, &exact).unwrap());
    assert!(authority_has_role(&tx.world, &destination, &role));
    assert!(
        !tx.world
            .roles()
            .get(&empty)
            .unwrap()
            .permissions()
            .any(|token| token == &exact)
    );
    assert!(
        tx.world
            .roles()
            .get(&role)
            .unwrap()
            .permissions()
            .any(|token| token == &exact)
    );
    assert!(
        tx.world
            .roles()
            .get(&"escalated_reader".parse().unwrap())
            .is_none()
    );
}

#[test]
fn restricted_read_successor_denies_sibling_unknown_and_expired_owner_for_grants_and_revokes() {
    for time in [50, 100, 200, 300] {
        let (state, owner, reader, _) = fixture(true);
        let mut block = state.block(header(&state, time));
        let mut tx = block.transaction();
        let scopes: &[u64] = if time == 50 { &[11, 12] } else { &[10, 11, 12] };
        for id in scopes {
            let exact = read_permission(*id);
            for instruction in [
                Grant::account_permission(exact.clone(), reader.clone()).into(),
                Revoke::account_permission(exact, reader.clone()).into(),
            ] {
                Executor::Initial
                    .execute_instruction(&mut tx, &owner, &instruction)
                    .expect_err("only the active owner of the exact known scope can mutate reads");
            }
        }
        assert!(
            authority_has_permission(&tx.world, &reader, &read_permission(10)).unwrap(),
            "rejected revocation must preserve access"
        );
    }
}

#[test]
fn restricted_read_successor_uses_current_owner_and_rejects_inert_ownership_generation() {
    let (state, former_owner, reader, new_owner) = fixture(true);
    let mut block = state.block(header(&state, 50));
    let mut tx = block.transaction();
    let role: RoleId = "former_owner_reader".parse().unwrap();
    Register::role(
        Role::new(role.clone(), former_owner.clone()).add_permission(read_permission(10)),
    )
    .execute(&former_owner, &mut tx)
    .unwrap();
    Grant::account_role(role.clone(), reader.clone())
        .execute(&former_owner, &mut tx)
        .unwrap();
    assert!(!authority_has_role(&tx.world, &new_owner, &role));
    let selector = crate::sns::selector_for_dataspace_alias("restricted-bank").unwrap();
    let address = AccountAddress::from_account_id(&new_owner).unwrap();
    let mut record = NameRecordV1::new(
        selector.clone(),
        new_owner.clone(),
        vec![NameControllerV1::account(&address)],
        0,
        0,
        100,
        200,
        300,
        Metadata::default(),
    );
    record.ownership_generation = 2;
    record.metadata.insert(
        crate::sns::SNS_DATASPACE_ID_METADATA_KEY.parse().unwrap(),
        Json::new(10_u64),
    );
    tx.world
        .smart_contract_state
        .insert(crate::sns::record_storage_key(&selector), record.encode());
    let exact = read_permission(10);
    let revoke = Revoke::account_permission(exact.clone(), reader.clone()).into();
    Executor::Initial
        .execute_instruction(&mut tx, &former_owner, &revoke)
        .expect_err("former owner cannot revoke after transfer");
    Executor::Initial
        .execute_instruction(&mut tx, &new_owner, &revoke)
        .unwrap();
    assert!(authority_has_permission(&tx.world, &reader, &exact).unwrap());
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &new_owner,
            &Revoke::role_permission(exact.clone(), role).into(),
        )
        .unwrap();
    assert!(
        !authority_has_permission(&tx.world, &reader, &exact).unwrap(),
        "current owner can revoke old roles without membership"
    );
    let grant = Grant::account_permission(exact.clone(), reader.clone()).into();
    Executor::Initial
        .execute_instruction(&mut tx, &former_owner, &grant)
        .expect_err("former owner cannot grant after transfer");
    record.ownership_generation = 0;
    tx.world
        .smart_contract_state
        .insert(crate::sns::record_storage_key(&selector), record.encode());
    Executor::Initial
        .execute_instruction(&mut tx, &new_owner, &grant)
        .expect_err("zero ownership generation is not an active authority");
    assert!(!authority_has_permission(&tx.world, &reader, &exact).unwrap());
}

#[test]
fn restricted_read_successor_preserves_actual_genesis_issuance_without_sns_owner() {
    for active in [false, true] {
        let reader = checked_account_id();
        let world = World::with([], [Account::new(reader.clone()).build(&reader)], []);
        if active {
            crate::runtime::activate_additions_for_testing(&world, MODULE).unwrap();
        }
        let exact = read_permission(123);
        let state = state_after_genesis_with_instructions(world, |_| {
            vec![Grant::account_permission(exact.clone(), reader.clone()).into()]
        });
        assert!(authority_has_permission(state.view().world(), &reader, &exact).unwrap());
    }
}

#[test]
fn restricted_read_successor_limits_mixed_role_revocation_to_the_owned_scope() {
    let (state, owner, reader, other_owner) = fixture(true);
    let mut block = state.block(header(&state, 50));
    let mut tx = block.transaction();
    let role: RoleId = "mixed_scope_readers".parse().unwrap();
    let a = read_permission(10);
    let b = read_permission(11);
    let ordinary: Permission = executor_permission::parameter::CanSetParameters.into();
    Register::role(
        Role::new(role.clone(), reader.clone())
            .add_permission(a.clone())
            .add_permission(b.clone())
            .add_permission(ordinary.clone()),
    )
    .execute(&reader, &mut tx)
    .unwrap();
    Grant::account_role(role.clone(), other_owner.clone())
        .execute(&reader, &mut tx)
        .unwrap();
    assert!(!authority_has_role(&tx.world, &owner, &role));
    for instruction in [
        Grant::account_role(role.clone(), owner.clone()).into(),
        Revoke::account_role(role.clone(), other_owner.clone()).into(),
        Unregister::role(role.clone()).into(),
        Revoke::role_permission(b.clone(), role.clone()).into(),
    ] {
        Executor::Initial
            .execute_instruction(&mut tx, &owner, &instruction)
            .expect_err("one scope owner cannot mutate the sibling capability");
        crate::smartcontracts::isi::execute_borrowed_instruction(&instruction, &owner, &mut tx)
            .expect_err("native dispatch retains every scope owner in a mixed role");
    }
    Executor::Initial
        .execute_instruction(
            &mut tx,
            &owner,
            &Revoke::role_permission(a, role.clone()).into(),
        )
        .unwrap();
    assert_eq!(
        tx.world
            .roles()
            .get(&role)
            .unwrap()
            .permissions()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([b, ordinary])
    );
    assert!(authority_has_role(&tx.world, &other_owner, &role));
}
