include!("v2_effects_kura_tip_replay.rs");

include!("v2_effects_01_view_churn_and_runtime_steps.rs");
#[test]
fn decision_serve_fence_rejects_durable_decision_loss_without_reopening() {
    let fixture = Fixture::new();
    let mut services = fixture.services();
    let subject = fixture.manifest.subject;
    services
        .begin_decision_serve_reconciliation()
        .expect("raise the initial Decision/Serve fence");
    services
        .finish_decision_serve_reconciliation(Some(subject))
        .expect("publish the durable Serve Decision");
    services
        .begin_decision_serve_reconciliation()
        .expect("raise the next runtime-step fence");
    let error = services
        .finish_decision_serve_reconciliation(None)
        .expect_err("a durable Decision cannot disappear on a later runtime step");
    assert!(error.contains("lost its durable Decision"));
    assert_eq!(services.durable_serve_decision, Some(subject));
    assert!(
        services.decision_serve_reconciliation_pending,
        "failed reconciliation keeps exact Serve admission fenced"
    );
}

#[test]
fn live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.omit_scheduler_ownership = true;
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Broadcast(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote(&fixture))),
        )])));

    assert!(matches!(
        executor.step(Instant::now(), &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("scheduler owner was missing")
    ));
    assert!(services.broadcasts.is_empty());
    assert!(services.statuses.is_empty());
    assert!(services.decision_serve_reconciliation_pending);
    assert!(executor.output_guard.restart_required());
}

#[test]
fn recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.reject_scheduler_ownership = true;
    executor.runtime.steps.push_back(Ok(RuntimeStep::Idle));

    assert!(matches!(
        executor.step_pending_tip_recovery(Instant::now(), &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("scheduler owner was invalid")
    ));
    assert!(services.statuses.is_empty());
    assert!(services.decision_serve_reconciliation_pending);
    assert!(executor.output_guard.restart_required());
}

include!("v2_effects_02_admission_handoffs.rs");
