/// Global SoraFS grants require the exact canonical unit payload.
#[test]
fn sorafs_permission_tokens_reject_same_name_with_substituted_payload() {
    use iroha_executor_data_model::permission::sorafs::{
        CanBindSorafsAlias, CanCompleteSorafsReplicationOrder, CanFileSorafsCapacityDispute,
        CanIssueSorafsReplicationOrder, CanSetSorafsPricing, CanUpsertSorafsProviderCredit,
    };

    let state = make_state();
    let mut block = state.block(block_header());
    let mut transaction = block.transaction();
    for required in [
        Permission::from(CanBindSorafsAlias),
        Permission::from(CanCompleteSorafsReplicationOrder),
        Permission::from(CanFileSorafsCapacityDispute),
        Permission::from(CanIssueSorafsReplicationOrder),
        Permission::from(CanSetSorafsPricing),
        Permission::from(CanUpsertSorafsProviderCredit),
    ] {
        for payload in [Json::new(false), Json::new(42_u32)] {
            let malformed = Permission::new(required.name().to_owned(), payload);
            assert_ne!(malformed, required);
            transaction
                .world
                .account_permissions
                .insert(alice(), std::collections::BTreeSet::from([malformed]));
            assert!(!has_permission(&transaction, &alice(), required.name()));
            require_permission(&transaction, &alice(), required.name())
                .expect_err("a matching name cannot replace an exact canonical grant");
        }
        transaction.world.account_permissions.insert(
            alice(),
            std::collections::BTreeSet::from([required.clone()]),
        );
        require_permission(&transaction, &alice(), required.name())
            .expect("the exact typed grant remains authorized");
    }
}
