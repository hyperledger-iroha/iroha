#[test]
fn nested_state_reads_log_the_callee_scope_not_only_the_root_scope() {
    crate::test_alias::ensure();
    let authority: AccountId = fixture_account("alice");
    let state = contract_test_state(&authority);
    let caller_contract = install_contract(
        &state,
        &authority,
        r#"
seiyaku Caller {
  view fn main() -> int { return 0; }
}
"#,
        0,
    );
    let callee_contract = install_contract(
        &state,
        &authority,
        r#"
seiyaku Callee {
  state StateMap<int, int> Values;

  view fn value() -> int {
    return Values.get(1).unwrap_or(0);
  }
}
"#,
        1,
    );
    let (result, log) = call_contract_syscall_access_log(
        &state,
        &authority,
        &caller_contract,
        &callee_contract,
        "value",
        Json::new(()),
    );
    result.expect("nested StateMap read");
    let logical_path = log
        .read_keys
        .iter()
        .find(|key| key.starts_with("Values/"))
        .expect("StateMap read key must be logged");
    assert!(
        !log.durable_read_paths.contains(logical_path),
        "deployed contracts must not read the raw unscoped namespace"
    );
    let callee_digest = hex::encode(Hash::new(callee_contract.to_string().as_bytes()).as_ref());
    let caller_digest = hex::encode(Hash::new(caller_contract.to_string().as_bytes()).as_ref());
    assert!(
        log.durable_read_paths
            .contains(&format!("sc/{callee_digest}/{logical_path}")),
        "selective retry must fingerprint the actual nested contract namespace"
    );
    assert!(
        !log.durable_read_paths
            .contains(&format!("sc/{caller_digest}/{logical_path}")),
        "nested reads must not be mislabeled as root-contract state"
    );
}
#[test]
fn nested_view_rollback_preserves_reads_but_discards_writes() {
    crate::test_alias::ensure();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority).with_access_logging();
    host.state_access_log.durable_read_paths_complete = true;
    let key: StatePath = "counter".parse().expect("state key");
    host.durable_state_overlay
        .insert(key.clone(), Some(vec![1]));
    let snapshot = host.snapshot_nested_contract_call();
    host.stage_durable_state_update(key.clone(), Some(vec![9]));
    host.log_state_read_key(key.as_ref());
    host.log_state_write_key(key.as_ref());
    host.finish_nested_contract_call(
        snapshot,
        NestedContractCallOutcome::RollbackViewPreservingReads,
    )
    .expect("roll back view effects");
    assert_eq!(
        host.durable_state_overlay.get(&key),
        Some(&Some(vec![1])),
        "view state writes must not escape"
    );
    assert!(host.state_access_log.read_keys.contains(key.as_ref()));
    assert!(
        host.state_access_log
            .durable_read_paths
            .contains(key.as_ref())
    );
    assert!(!host.state_access_log.write_keys.contains(key.as_ref()));
    assert!(host.state_access_log.state_writes.is_empty());
}
#[test]
fn failed_nested_call_discards_reads_and_composed_state_changes() {
    crate::test_alias::ensure();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority).with_access_logging();
    host.state_access_log.durable_read_paths_complete = true;
    let key: StatePath = "counter".parse().expect("state key");
    host.durable_state_overlay
        .insert(key.clone(), Some(vec![1]));
    let outer = host.snapshot_nested_contract_call();
    host.stage_durable_state_update(key.clone(), Some(vec![2]));
    let inner = host.snapshot_nested_contract_call();
    host.stage_durable_state_update(key.clone(), Some(vec![3]));
    host.log_state_read_key(key.as_ref());
    host.finish_nested_contract_call(inner, NestedContractCallOutcome::Commit)
        .expect("commit inner call into outer frame");
    assert_eq!(host.durable_state_overlay.get(&key), Some(&Some(vec![3])));
    host.finish_nested_contract_call(outer, NestedContractCallOutcome::Rollback)
        .expect("roll back outer call");
    assert_eq!(host.durable_state_overlay.get(&key), Some(&Some(vec![1])));
    assert!(!host.state_access_log.read_keys.contains(key.as_ref()));
    assert!(host.state_access_log.durable_read_paths.is_empty());
}
#[test]
fn nested_snapshot_shares_large_rollback_state_until_mutated() {
    crate::test_alias::ensure();
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority);
    let verified_ballot = Arc::clone(&host.zk_verified_ballot);
    let replay_ledger = Arc::clone(&host.axt_replay_ledger);
    let proof_cache = Arc::clone(&host.axt_proof_cache);
    let handle_budget_ledger = Arc::clone(&host.axt_handle_budget_ledger);
    host.fastpq_batch_entries = Some(Vec::new());
    let snapshot = host.snapshot_nested_contract_call();
    assert!(Arc::ptr_eq(&snapshot.zk_verified_ballot, &verified_ballot));
    assert!(Arc::ptr_eq(&snapshot.axt_replay_ledger, &replay_ledger));
    assert!(Arc::ptr_eq(&snapshot.axt_proof_cache, &proof_cache));
    assert!(Arc::ptr_eq(
        &snapshot.axt_handle_budget_ledger,
        &handle_budget_ledger
    ));
    assert!(
        host.fastpq_batch_entries.is_none(),
        "frame-local batch storage must be moved, not cloned"
    );
    Arc::make_mut(&mut host.zk_verified_ballot).push_back([7; 32]);
    Arc::make_mut(&mut host.axt_handle_budget_ledger).clear();
    assert!(!Arc::ptr_eq(&host.zk_verified_ballot, &verified_ballot));
    assert!(!Arc::ptr_eq(
        &host.axt_handle_budget_ledger,
        &handle_budget_ledger
    ));
    host.finish_nested_contract_call(snapshot, NestedContractCallOutcome::Rollback)
        .expect("restore shared rollback state");
    assert!(Arc::ptr_eq(&host.zk_verified_ballot, &verified_ballot));
    assert!(Arc::ptr_eq(
        &host.axt_handle_budget_ledger,
        &handle_budget_ledger
    ));
    assert!(host.zk_verified_ballot.is_empty());
    assert!(host.fastpq_batch_entries.is_some());
}
