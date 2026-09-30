// Production nested dispatch controls for prepaid return-envelope admission and cleanup.

fn nested_return_funding_fixture() -> (State, AccountId, ContractAddress, ContractAddress) {
    let authority = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller = install_contract(
        &state,
        &authority,
        r#"
seiyaku Caller {
  error enum CalleeError { ForcedFailure = 1 }
  view fn main() -> int { return 0; }
}
"#,
        0,
    );
    let callee = install_contract(
        &state,
        &authority,
        r#"
seiyaku Callee {
  error enum CalleeError { ForcedFailure = 1 }
  state int counter;
  hajimari() { counter = 0; }
  kotoage fn write() -> int authorize("AssetOps") {
    counter = 9;
    return counter;
  }
  kotoage fn fail_after_write() -> int authorize("AssetOps") {
    counter = 9;
    require(false, CalleeError::ForcedFailure);
    return 0;
  }
}
"#,
        1,
    );
    grant_asset_ops_to_account(&state, &authority, caller.subject_id());
    (state, authority, caller, callee)
}

#[test]
fn prepaid_nested_return_refuses_before_checkout_effects_and_gas_then_retries() {
    let (state, authority, caller, callee) = nested_return_funding_fixture();
    let budget = iroha_allocation::AllocationBudget::new(0);
    let cache = PreparedContractCache::with_execution_budget(1, budget.clone());
    let (refused, vm, effects) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller,
        &callee,
        "write",
        Json::new(()),
        cache.clone(),
    );
    let error = refused.expect_err("return allowance cannot fit a zero pool");
    assert!(matches!(error, ivm::VMError::AllocationDeferred(_)));
    assert_eq!(error.metered_gas(), None);
    assert_eq!(vm.remaining_gas(), 1_000_000);
    assert_eq!(
        vm.validate_tlv(vm.register(10)).unwrap().payload,
        callee.as_ref().as_bytes()
    );
    assert!(effects.is_empty());
    assert_eq!(
        cache.stats().runtime_misses,
        0,
        "reserve before child checkout"
    );
    assert_eq!(cache.stats().runtime_prepared_loads, 0);
    assert_eq!(budget.reserved_bytes(), 0);

    budget.set_limit_bytes(128 * 1024 * 1024);
    let (success, vm, effects) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller,
        &callee,
        "write",
        Json::new(()),
        cache.clone(),
    );
    let cold_gas = success.expect("same invocation retries after local capacity returns");
    assert!(
        !effects.is_empty(),
        "only successful retry publishes the child write"
    );
    assert_eq!(
        decode_nested_return(
            vm.validate_tlv(vm.register(10)).unwrap().payload,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
        ),
        norito::json!("9")
    );
    let after_success = cache.stats();
    let idle_bytes = budget.reserved_bytes();
    // Concurrent aggregate-cache pressure may evict idle storage. Zero still refuses the next
    // return allowance without depending on whether that optional retention survived.
    budget.set_limit_bytes(0);
    let (refused, vm, effects) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller,
        &callee,
        "write",
        Json::new(()),
        cache.clone(),
    );
    assert!(matches!(refused, Err(ivm::VMError::AllocationDeferred(_))));
    assert_eq!(vm.remaining_gas(), 1_000_000);
    assert!(effects.is_empty());
    assert_eq!(cache.stats().runtime_hits, after_success.runtime_hits);
    assert!(budget.reserved_bytes() <= idle_bytes);

    budget.set_limit_bytes(128 * 1024 * 1024);
    let (retry, vm, effects) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller,
        &callee,
        "write",
        Json::new(()),
        cache.clone(),
    );
    assert_eq!(
        retry.expect("retry after capacity returns"),
        cold_gas,
        "funding never changes gas"
    );
    assert!(!effects.is_empty());
    assert_eq!(
        decode_nested_return(
            vm.validate_tlv(vm.register(10)).unwrap().payload,
            iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
        ),
        norito::json!("9")
    );
    drop(cache);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "all nested owners were reclaimed"
    );
}

#[test]
fn prepaid_nested_return_is_released_when_child_effects_roll_back() {
    let (state, authority, caller, callee) = nested_return_funding_fixture();
    let budget = iroha_allocation::AllocationBudget::new(128 * 1024 * 1024);
    let cache = PreparedContractCache::with_execution_budget(1, budget.clone());
    let (result, vm, effects) = call_contract_syscall_with_prepared_cache(
        &state,
        &authority,
        &caller,
        &callee,
        "fail_after_write",
        Json::new(()),
        cache.clone(),
    );
    let error = result.expect_err("callee aborts after staging its write");
    assert!(matches!(
        error.as_unmetered(),
        ivm::VMError::ContractAbort { code: 1, .. }
    ));
    assert!(
        error.metered_gas().is_some(),
        "contract faults remain metered"
    );
    assert!(
        vm.remaining_gas() < 1_000_000,
        "child execution still spends its gas"
    );
    assert!(effects.is_empty(), "failed child write is rolled back");
    assert_eq!(
        vm.validate_tlv(vm.register(10)).unwrap().payload,
        callee.as_ref().as_bytes()
    );
    drop(cache);
    assert_eq!(
        budget.reserved_bytes(),
        0,
        "unused return credit cannot leak on child failure"
    );
}
