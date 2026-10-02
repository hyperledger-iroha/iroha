// Exact original Treasury credit readers retain local outer Norito refusal.

fn credit_decode_limits(allocation: usize) -> norito::DecodeLimits {
    norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, 32)
}

fn credit_decode_allocation_used(operation: impl FnOnce()) -> usize {
    const CEILING: usize = 8 * 1024 * 1024;
    norito::with_decode_limits_scope(credit_decode_limits(CEILING), || {
        operation();
        let norito::Error::TotalAllocationExceeded { attempted, limit } =
            norito::core::reserve_decode_allocation(CEILING + 1).unwrap_err()
        else {
            panic!("the over-limit probe observes cumulative allocation without charging it");
        };
        assert_eq!(limit, u64::try_from(CEILING).unwrap());
        usize::try_from(attempted).unwrap() - CEILING - 1
    })
}

fn check_original_treasury_credit_decode_refusal(asset_binding: bool) {
    let (chain, binding) = signed_payout_lifecycle_registry_fixture();
    let mut policy = policy_with_treasury_payout_lifecycle(binding.clone());
    policy.network_id = chain.network_id();
    let registry = policy_registry(std::slice::from_ref(&policy));
    let state = std::sync::Arc::clone(chain.state());
    let original_tip = state.view().latest_block_hash();
    let original_height = state.view().height();
    let mut block = state.block(BlockHeader::new(
        u64::try_from(original_height + 1)
            .unwrap()
            .try_into()
            .unwrap(),
        original_tip,
        None,
        4_000,
        0,
    ));
    let mut transaction = block.transaction();
    install_policy_registry_fixture(&registry, &mut transaction);
    let credit = ValidationFeeCredit::from_policy_minor_units(
        binding.treasury_account_id.clone(),
        binding.lifecycle_seal().unwrap(),
        policy_fee_asset(&policy),
        policy.ds_scale,
        10,
    )
    .unwrap();
    commit_validation_fee_credit(&mut transaction, Some(&credit))
        .expect("seed canonical lifecycle-bound Treasury balance and asset leaves");
    let (value_key, asset_key) = validation_fee_credit_state_keys(&transaction, &credit).unwrap();
    let value_bytes = transaction
        .world
        .smart_contract_state
        .get(&value_key)
        .unwrap()
        .clone();
    let asset_bytes = transaction
        .world
        .smart_contract_state
        .get(&asset_key)
        .unwrap()
        .clone();
    let original_value_pointer = transaction
        .world
        .smart_contract_state
        .get(&value_key)
        .unwrap()
        .as_ptr();
    let original_asset_pointer = transaction
        .world
        .smart_contract_state
        .get(&asset_key)
        .unwrap()
        .as_ptr();
    assert_eq!(
        read_validation_fee_credit_balance(&transaction, &credit).unwrap(),
        credit.amount
    );

    // Measure only the original authenticated prefix, not a guessed fixed capacity. The
    // exact same prefix must succeed under the chosen limit before the target decoder is
    // observed refusing; a root-metadata failure cannot stand in for this leaf regression.
    let prefix = || {
        assert_eq!(
            validation_fee_credit_state_keys(&transaction, &credit).unwrap(),
            (value_key.clone(), asset_key.clone())
        );
        if asset_binding {
            assert_eq!(
                decode_validation_fee_credit_state_value(&value_bytes)
                    .expect("admit the original balance before its asset binding"),
                credit.amount
            );
        }
    };
    let prefix_bytes = credit_decode_allocation_used(prefix);
    assert!(prefix_bytes > 0);
    norito::with_decode_limits_scope(credit_decode_limits(prefix_bytes), prefix);
    let producer_error =
        norito::with_decode_limits_scope(credit_decode_limits(prefix_bytes), || {
            prefix();
            if asset_binding {
                norito::decode_from_bytes::<AssetDefinitionId>(&asset_bytes).unwrap_err()
            } else {
                norito::decode_from_bytes::<StateValueRecordV1>(&value_bytes).unwrap_err()
            }
        });
    assert!(
        matches!(
            producer_error,
            norito::Error::TotalAllocationExceeded { .. }
        ),
        "target outer decoder did not reach its inherited allocation refusal: {producer_error:?}"
    );
    println!(
        "asset_binding={asset_binding}, admitted prefix bytes={prefix_bytes}, original producer={producer_error:?}"
    );

    let refused = norito::with_decode_limits_scope(credit_decode_limits(prefix_bytes), || {
        read_validation_fee_credit_balance(&transaction, &credit)
    });
    assert!(
        matches!(&refused, Err(ExecutionAttemptError::Deferred(reason)) if reason.reason() == ivm::error::ExecutionDeferral::ActiveMemoryCapacity && reason.allocation_refusal().is_none()),
        "original Treasury credit decoder refusal became a permanent malformed-leaf verdict: {refused:?}"
    );
    assert_eq!(
        transaction.world.smart_contract_state.get(&value_key),
        Some(&value_bytes)
    );
    assert_eq!(
        transaction.world.smart_contract_state.get(&asset_key),
        Some(&asset_bytes)
    );
    assert_eq!(
        transaction
            .world
            .smart_contract_state
            .get(&value_key)
            .unwrap()
            .as_ptr(),
        original_value_pointer
    );
    assert_eq!(
        transaction
            .world
            .smart_contract_state
            .get(&asset_key)
            .unwrap()
            .as_ptr(),
        original_asset_pointer
    );
    assert_eq!(
        read_validation_fee_credit_balance(&transaction, &credit).unwrap(),
        credit.amount
    );
    assert!(transaction.execution_deferral().is_none());
    norito::with_decode_limits_scope(credit_decode_limits(prefix_bytes), || {
        commit_validation_fee_credit(&mut transaction, Some(&credit))
            .expect_err("the connected fee writer must retain this unfinished read");
    });
    let deferred = transaction
        .execution_deferral()
        .expect("sticky original refusal");
    assert_eq!(
        deferred.reason(),
        ivm::error::ExecutionDeferral::ActiveMemoryCapacity
    );
    assert!(deferred.allocation_refusal().is_none());
    assert_eq!(
        transaction.world.smart_contract_state.get(&value_key),
        Some(&value_bytes)
    );
    assert_eq!(
        transaction.world.smart_contract_state.get(&asset_key),
        Some(&asset_bytes)
    );
    transaction.apply();
    assert!(block.world.smart_contract_state.get(&value_key).is_none());
    assert!(block.world.smart_contract_state.get(&asset_key).is_none());
    drop(block);
    assert_eq!(state.view().height(), original_height);
    assert_eq!(state.view().latest_block_hash(), original_tip);
    assert_eq!(chain.kura().blocks_count(), original_height);
}

#[test]
fn original_treasury_credit_record_decode_refusal_preserves_balance_and_retries() {
    check_original_treasury_credit_decode_refusal(false);
}

#[test]
fn original_treasury_credit_asset_decode_refusal_preserves_binding_and_retries() {
    check_original_treasury_credit_decode_refusal(true);
}
