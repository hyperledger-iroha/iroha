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

/// Permission checks borrow exact canonical JSON even under an exhausted decode budget.
#[test]
fn sorafs_permission_tokens_are_exact_under_zero_decode_allocation() {
    use iroha_executor_data_model::permission::sorafs::CanIssueSorafsReplicationOrder;

    let authority = alice();
    let required = Permission::from(CanIssueSorafsReplicationOrder);
    assert_eq!(required.payload().get().as_str(), "null");
    let state = make_state();
    let mut block = state.block(block_header());
    let mut transaction = block.transaction();
    let zero_allocation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    for (name, raw_payload, expected) in [
        (required.name(), "null", true),
        (required.name(), "\"null\"", false),
        (required.name(), "{\"provider\":null}", false),
        (required.name(), "false", false),
        (required.name(), "[]", false),
        (required.name(), "42", false),
        ("CanIssueSorafsReplicationOrderSubstituted", "null", false),
    ] {
        let grant = Permission::new(
            name.to_owned(),
            Json::from_raw_json(raw_payload.to_owned()).expect("canonical fixture payload"),
        );
        assert_eq!(grant == required, expected);
        transaction
            .world
            .account_permissions
            .insert(authority.clone(), std::collections::BTreeSet::from([grant]));
        let ((present, result), usage) =
            norito::core::with_decode_limits_measured(zero_allocation, || {
                (
                    has_permission(&transaction, &authority, required.name()),
                    require_permission(&transaction, &authority, required.name()),
                )
            });
        assert_eq!(usage.total_allocated_bytes(), 0, "{name}({raw_payload})");
        assert_eq!(present, expected, "{name}({raw_payload})");
        if expected {
            result.expect("the exact typed null grant remains authorized");
        } else {
            assert!(
                matches!(result, Err(InstructionExecutionError::InvalidParameter(
                InvalidParameterError::SmartContract(message)
            )) if message == format!("permission {} required for SoraFS operation", required.name()))
            );
        }
    }
    for has_empty_entry in [true, false] {
        if has_empty_entry {
            transaction
                .world
                .account_permissions
                .insert(authority.clone(), Permissions::new());
        } else {
            transaction
                .world
                .account_permissions
                .remove(authority.clone());
        }
        let ((present, result), usage) =
            norito::core::with_decode_limits_measured(zero_allocation, || {
                (
                    has_permission(&transaction, &authority, required.name()),
                    require_permission(&transaction, &authority, required.name()),
                )
            });
        assert_eq!(usage.total_allocated_bytes(), 0);
        assert!(!present);
        assert!(
            matches!(result, Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(message)
        )) if message == format!("permission {} required for SoraFS operation", required.name()))
        );
    }
}
