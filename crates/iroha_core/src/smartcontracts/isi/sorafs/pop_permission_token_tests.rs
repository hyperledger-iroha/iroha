/// Neither direct nor role grants may substitute a PoP permission's payload.
#[test]
fn pop_permission_tokens_require_canonical_direct_and_role_grants() {
    use iroha_data_model::prelude::{Role, RoleId};
    use iroha_executor_data_model::permission::sorafs::{
        CanManageSorafsPopRegistry, CanOperateSorafsPopIssuer,
    };

    let operator = keypair(0x71);
    let owner = account(&operator);
    let state = state(&operator, &[]);
    let mut block = state.block(BlockHeader::new(
        nonzero!(2_u64),
        None,
        None,
        None,
        NOW * 1_000,
        0,
    ));
    let mut transaction = block.transaction();
    for required in [
        Permission::from(CanManageSorafsPopRegistry),
        Permission::from(CanOperateSorafsPopIssuer),
    ] {
        let malformed = Permission::new(required.name().to_owned(), Json::new(false));
        assert_ne!(malformed, required);
        transaction.world.account_permissions.insert(
            owner.clone(),
            std::collections::BTreeSet::from([malformed.clone()]),
        );
        require_permission(&transaction, &owner, required.name())
            .expect_err("malformed direct grant must fail closed");
        transaction.world.account_permissions.insert(
            owner.clone(),
            std::collections::BTreeSet::from([required.clone()]),
        );
        require_permission(&transaction, &owner, required.name())
            .expect("canonical direct grant remains authorized");
        transaction.world.account_permissions.remove(owner.clone());

        let role_id: RoleId = "pop_permission_scope".parse().expect("role id");
        let role = Role::new(role_id.clone(), owner.clone())
            .add_permission(malformed)
            .build(&owner);
        transaction.world.roles.insert(role_id.clone(), role);
        transaction.world.account_roles.insert(
            crate::role::RoleIdWithOwner::new(owner.clone(), role_id.clone()),
            (),
        );
        require_permission(&transaction, &owner, required.name())
            .expect_err("malformed role grant must fail closed");
        let role = Role::new(role_id.clone(), owner.clone())
            .add_permission(required.clone())
            .build(&owner);
        transaction.world.roles.insert(role_id.clone(), role);
        require_permission(&transaction, &owner, required.name())
            .expect("canonical role grant remains authorized");
        transaction
            .world
            .account_roles
            .remove(crate::role::RoleIdWithOwner::new(
                owner.clone(),
                role_id.clone(),
            ));
        transaction.world.roles.remove(role_id);
    }
}
