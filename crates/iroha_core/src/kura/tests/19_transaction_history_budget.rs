// The configured pool is one original owner, independent of block-hash history.

#[test]
fn transaction_history_budget_rejects_zero_before_storage_creation() {
    let temp = TempDir::new().expect("temporary parent");
    let store_root = temp.path().join("kura");
    let mut config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    config.transaction_history_bytes = iroha_config_base::util::Bytes(0);
    let error =
        open_configured_kura_with_pending_limits(&config, &SumeragiV2RuntimeLimits::default())
            .expect_err("zero transaction history capacity is invalid");
    assert!(matches!(error, Error::IO(ref source, ref path)
        if source.kind() == ErrorKind::InvalidInput
            && source.to_string().contains("kura.transaction_history_bytes")
            && path == &store_root));
    assert!(
        !store_root.exists(),
        "refusal must precede storage creation"
    );
}

#[test]
fn transaction_history_budget_retains_the_configured_pool_and_refunds_once() {
    let (_temp, mut config) = kura_storage_fixture("membership budget", BLOCKS_IN_MEMORY);
    config.transaction_history_bytes = iroha_config_base::util::Bytes(17);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
    let first = kura.transaction_history_budget();
    let second = kura.transaction_history_budget();
    let hash_history = kura.block_hash_history_budget();
    assert_eq!(first.limit_bytes(), 17);
    assert_eq!(first.reserved_bytes(), 0);
    let reserved = first
        .try_reserve_bytes(17)
        .expect("exact configured capacity");
    assert_eq!(second.reserved_bytes(), 17);
    assert_eq!(
        hash_history.reserved_bytes(),
        0,
        "hash history has its own pool"
    );
    assert!(matches!(
        second.try_reserve_bytes(1),
        Err(mv::allocation::AllocationRefusal::Capacity { .. })
    ));
    drop(first);
    drop(kura);
    assert_eq!(
        second.reserved_bytes(),
        17,
        "the charge retains the original pool"
    );
    drop(reserved);
    assert_eq!(second.reserved_bytes(), 0);
    let retry = second
        .try_reserve_bytes(17)
        .expect("refund funds the original pool");
    assert_eq!(second.reserved_bytes(), 17);
    drop(retry);
    assert_eq!(second.reserved_bytes(), 0);
}

#[test]
fn transaction_history_budget_test_stores_have_finite_independent_pools() {
    let first = Kura::blank_kura_for_testing();
    let second = Kura::blank_kura_for_testing();
    let first_budget = first.transaction_history_budget();
    let second_budget = second.transaction_history_budget();
    assert_eq!(
        first_budget.limit_bytes() as u64,
        iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES.get()
    );
    assert_eq!(second_budget.limit_bytes(), first_budget.limit_bytes());
    let full = first_budget
        .try_reserve_bytes(first_budget.limit_bytes())
        .expect("finite test policy");
    assert_eq!(second_budget.reserved_bytes(), 0);
    let other = second_budget
        .try_reserve_bytes(1)
        .expect("distinct original store pool");
    drop(full);
    assert_eq!(first_budget.reserved_bytes(), 0);
    assert_eq!(second_budget.reserved_bytes(), 1);
    drop(other);
}
