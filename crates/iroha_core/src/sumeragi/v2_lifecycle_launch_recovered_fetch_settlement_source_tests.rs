#[test]
fn recovered_decision_fetch_store_settlement_is_restart_closed_and_tail_infallible() {
    let launch = include_str!("v2_lifecycle_launch.rs");
    let settlement = launch
        .split_once("pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(")
        .expect("recovered Fetch has one Store settlement transaction")
        .1
        .split_once("/// Reserve, claim, and queue one recovered Sign")
        .expect("recovered Fetch Store settlement stays bounded")
        .0;
    let selector = settlement
        .find("prepare_lifecycle_ingress_selector(")
        .expect("fresh selector preflight exists");
    let request = settlement
        .find("prepare_recovered_decision_fetch_owner_retirement(")
        .expect("request/response retirement preflight exists");
    let marker_prepare = settlement
        .find("prepare_published_lifecycle_store_retry_marker(body.durable())")
        .expect("active Store marker catalog is preflighted");
    let ingress = settlement
        .find("into_locked_recovered_decision_fetch_dequeue(")
        .expect("exact ingress occurrence is locked");
    let carrier = settlement
        .find("prepare_recovered_decision_fetch_store_adapter_authority(")
        .expect("claimed recovered carrier preflight exists");
    let adapter = settlement
        .find("prepare_recovered_decision_fetch_store_adapter(")
        .expect("fixed reducer preview exists");
    let registry = settlement
        .find("prepare_recovered_decision_fetch_store_successor(")
        .expect("dedicated Store carrier preflight exists");
    let marker_bind = settlement
        .find(".bind_store_successor(")
        .expect("active Store marker is bound to the sealed successor");
    let transition = settlement
        .find("prepare_recovered_decision_fetch_store_transition(")
        .expect("Fetch-to-Store coordinator successor is staged");
    let output = settlement
        .find("begin_fail_stop_operation()")
        .expect("output fail-stop cut precedes publication");
    let fsync = settlement
        .find("transition.persist_exact_successor().is_err()")
        .expect("exact LedgerV1 successor is fsynced once");
    let coordinator_commit = settlement
        .find("transition.commit_after_publication();")
        .expect("coordinator/registry/adapter tail exists");
    let marker_commit = settlement
        .find("commit_published_lifecycle_store_retry_marker(retry_marker);")
        .expect("active Store marker commits after durable publication");
    let request_commit = settlement
        .find("commit_recovered_decision_fetch_owner_retirement(retirement);")
        .expect("dedicated request owner retires after publication");
    let ingress_commit = settlement
        .find("locked_dequeue.commit();")
        .expect("locked ingress occurrence retires after publication");
    let worker_commit = settlement
        .find("completion.acknowledge_after_publication();")
        .expect("worker owner retires and disarms after publication");
    let output_commit = settlement
        .find("operation.complete();")
        .expect("output fail-stop cut closes last");
    assert!(
        marker_prepare < selector
            && selector < request
            && request < ingress
            && ingress < carrier
            && carrier < adapter
            && adapter < registry
            && registry < marker_bind
            && marker_bind < transition
            && transition < output
            && output < fsync
            && fsync < coordinator_commit
            && coordinator_commit < marker_commit
            && marker_commit < request_commit
            && request_commit < ingress_commit
            && ingress_commit < worker_commit
            && worker_commit < output_commit
    );
    let tail = &settlement[coordinator_commit..];
    assert!(!tail.contains("return "));
    assert!(!tail.contains("Result<"));
    assert!(!tail.contains(".is_err()"));

    let worker = include_str!("v2_worker_completion.rs");
    let guarded = worker
        .split_once("impl GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {")
        .expect("recovered Fetch completion has one armed guard")
        .1
        .split_once("impl GuardedCertifiedFetchBodyPersistenceCompletion")
        .expect("recovered Fetch guard stays bounded")
        .0;
    assert!(guarded.contains("let _completion = self"));
    assert!(guarded.contains(".take()"));
    assert!(guarded.contains("self.drop_guard.disarm();"));
    let prepared = worker
        .split_once("impl PreparedRecoveredDecisionFetchBodyCompletionV1 {")
        .expect("parked recovered Fetch completion has one consuming acknowledgement")
        .1
        .split_once("impl PreparedRecoveredLifecycleSignCompletionV1")
        .expect("parked recovered Fetch acknowledgement stays bounded")
        .0;
    let index = prepared
        .find("acknowledge_recovered_decision_fetch_body(key, id, response_hash);")
        .expect("exact worker index is removed");
    let disarm = prepared
        .find("self.guarded.acknowledge_after_publication();")
        .expect("restart guard is disarmed after index removal");
    assert!(index < disarm);

    let ledger = [
        include_str!("v2_lifecycle_ledger.rs"),
        include_str!("v2_lifecycle_ledger_operations.rs"),
    ]
    .concat();
    let open = include_str!("v2_lifecycle_open.rs");
    let registry_source = [
        include_str!("v2_lifecycle_work_registry_validate_recovery.rs"),
        include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"),
    ]
    .concat();
    for required in [
        "authenticate_recovered_decision_fetch_store",
        "open_recovered_decision_store_startup",
        "stage_recovered_decision_apply_projection",
        "successor_records_after_live_store",
    ] {
        assert!(ledger.contains(required), "cold restart omitted {required}");
    }
    assert!(open.contains("RecoveredWalStartupProjectionV1::DecisionStore"));
    assert!(registry_source.contains("install_recovered_wal_decision_store"));
}
