/// Proof signer policy management requires the exact canonical permission token.
#[test]
fn proof_outcome_permission_tokens_require_canonical_direct_and_role_grants() {
    use iroha_data_model::prelude::{Role, RoleId};
    use iroha_primitives::json::Json;

    let manager = ed25519_keypair(0x71);
    let scheduler = ed25519_keypair(0x72);
    let relayer_a = ed25519_keypair(0x73);
    let relayer_b = ed25519_keypair(0x74);
    let owner = account(&manager);
    let state = state_with_accounts(&manager, &scheduler, &relayer_a, &relayer_b);
    let mut block = state.block(block_header_at(2, NOW));
    let mut transaction = block.transaction();
    let required = Permission::from(CanManageSorafsProofOutcomePolicy);
    let malformed = Permission::new(required.name().to_owned(), Json::new(false));
    assert_ne!(malformed, required);
    transaction.world.account_permissions.insert(
        owner.clone(),
        std::collections::BTreeSet::from([malformed.clone()]),
    );
    assert!(!has_policy_permission(&transaction, &owner));
    transaction.world.account_permissions.insert(
        owner.clone(),
        std::collections::BTreeSet::from([required.clone()]),
    );
    assert!(has_policy_permission(&transaction, &owner));
    transaction.world.account_permissions.remove(owner.clone());

    let role_id: RoleId = "proof_policy_scope".parse().expect("role id");
    let role = Role::new(role_id.clone(), owner.clone())
        .add_permission(malformed)
        .build(&owner);
    transaction.world.roles.insert(role_id.clone(), role);
    transaction.world.account_roles.insert(
        crate::role::RoleIdWithOwner::new(owner.clone(), role_id.clone()),
        (),
    );
    assert!(!has_policy_permission(&transaction, &owner));
    let role = Role::new(role_id.clone(), owner.clone())
        .add_permission(required)
        .build(&owner);
    transaction.world.roles.insert(role_id, role);
    assert!(has_policy_permission(&transaction, &owner));
}
