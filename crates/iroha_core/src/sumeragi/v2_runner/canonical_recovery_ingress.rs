#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CanonicalRecoveryIngressDrain {
    drained: usize,
    exact_response_progress: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CanonicalRecoveryEffectDispatch {
    handled: usize,
    request_dispatched: bool,
}
const fn canonical_recovery_source_work_remains(has_pending: bool, effect_count: usize) -> bool {
    has_pending || effect_count != 0
}
const fn canonical_recovery_ingress_advances_requester(
    is_response: bool,
    outcome: V2LaneIngressOutcome,
) -> bool {
    is_response && matches!(outcome, V2LaneIngressOutcome::Inserted)
}
fn refresh_canonical_recovery_retry_deadline(
    deadline: &mut Instant,
    sent_at: Instant,
    retransmit_interval: Duration,
    request_queued: bool,
) {
    if request_queued {
        *deadline = deadline_after(sent_at, retransmit_interval);
    }
}
fn refresh_canonical_recovery_retry_deadline_after_progress(
    deadline: &mut Instant,
    sent_at: Instant,
    retransmit_interval: Duration,
    request_queued: bool,
) {
    if request_queued {
        *deadline = deadline_after(sent_at, retransmit_interval);
    } else {
        *deadline = (*deadline).min(sent_at);
    }
}
fn canonical_recovery_idle_wait(next_retry: Instant, now: Instant) -> Duration {
    let wait = next_retry.saturating_duration_since(now).min(IDLE_POLL);
    if wait.is_zero() { IDLE_POLL } else { wait }
}
fn service_canonical_executed_block_recovery(
    recovery: &mut CanonicalExecutedBlockRecovery,
    services: &ProductionV2Services,
) -> Result<bool, V2RunnerError> {
    let current_archive_targets = services.current_archive_targets();
    recovery
        .service_next_with_archive_targets(&current_archive_targets)
        .map_err(V2RunnerError::from)
}
fn drain_canonical_executed_block_recovery_ingress(
    receiver: &FairV2Ingress,
    recovery: &mut CanonicalExecutedBlockRecovery,
    limit: usize,
) -> Result<CanonicalRecoveryIngressDrain, V2RunnerError> {
    let mut summary = CanonicalRecoveryIngressDrain::default();
    for _ in 0..limit.max(1) {
        let Some(inbound) = receiver
            .try_recv_if_checked(|inbound| recovery.admits_message(inbound.message()))
            .map_err(V2RunnerError::Service)?
        else {
            break;
        };
        let is_response = matches!(
            inbound.message(),
            BlockMessage::LaneHistoricalRecoveryResponse(_)
        );
        let outcome = recovery.accept_with_ingress_ownership(inbound)?;
        summary.exact_response_progress |=
            canonical_recovery_ingress_advances_requester(is_response, outcome);
        summary.drained = summary.drained.saturating_add(1);
        if !recovery.has_pending() {
            break;
        }
    }
    Ok(summary)
}
fn apply_retired_canonical_recovery_requests(
    recovery: &mut CanonicalExecutedBlockRecovery,
    services: &ProductionV2Services,
) -> Result<usize, V2RunnerError> {
    let request_hashes = recovery.drain_retired_request_hashes();
    if request_hashes.is_empty() {
        return Ok(0);
    }
    match services.cancel_historical_lane_recovery_requests(&request_hashes) {
        Ok(cancelled) => Ok(cancelled),
        Err(error) => {
            recovery.requeue_retired_request_hashes(request_hashes)?;
            Err(V2RunnerError::Service(error))
        }
    }
}
fn dispatch_canonical_executed_block_recovery_effects(
    recovery: &mut CanonicalExecutedBlockRecovery,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<CanonicalRecoveryEffectDispatch, V2RunnerError> {
    let _ = apply_retired_canonical_recovery_requests(recovery, services)?;
    services
        .retry_pending_exact_output()
        .map_err(V2RunnerError::Service)?;
    let scan_limit = recovery.effect_count();
    let mut summary = CanonicalRecoveryEffectDispatch::default();
    for _ in 0..scan_limit.min(limit.max(1)) {
        let Some(effect) = recovery.drain_effects(1).pop() else {
            break;
        };
        let is_request = matches!(
            &effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneHistoricalRecoveryRequest(_),
                ..
            }
        );
        let is_current_request = recovery.is_current_request_effect(&effect);
        if !matches!(&effect, V2LaneWorkEffect::PostLaneBlock { .. }) {
            return Err(V2RunnerError::Service(
                "canonical executed-block recovery emitted a non-recovery lane effect".to_owned(),
            ));
        }
        if is_request && !is_current_request {
            summary.handled = summary.handled.saturating_add(1);
            continue;
        }
        if !services
            .can_retain_lane_work_effect(&effect)
            .map_err(V2RunnerError::Service)?
        {
            if !recovery.requeue_effect(effect) {
                return Err(V2RunnerError::Service(
                    "canonical executed-block recovery could not retain a backpressured effect"
                        .to_owned(),
                ));
            }
            break;
        }
        match dispatch_lane_work_effect(services, effect)? {
            LaneWorkEffectDispatch::Complete => {
                summary.handled = summary.handled.saturating_add(1);
                summary.request_dispatched |= is_current_request;
            }
            LaneWorkEffectDispatch::SourceRetained(effect) => {
                if !recovery.requeue_effect(effect) {
                    return Err(V2RunnerError::Service(
                        "canonical executed-block recovery lost a source-retained effect"
                            .to_owned(),
                    ));
                }
                break;
            }
        }
    }
    Ok(summary)
}
#[cfg(test)]
/// Exercise one production canonical-recovery output turn in ownership tests.
pub(in crate::sumeragi) fn dispatch_canonical_executed_block_recovery_effects_for_test(
    recovery: &mut CanonicalExecutedBlockRecovery,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<(usize, bool), V2RunnerError> {
    dispatch_canonical_executed_block_recovery_effects(recovery, services, limit)
        .map(|summary| (summary.handled, summary.request_dispatched))
}
/// Advance one retained historical lane owner on the ordinary retransmission
/// cadence, even when no lane or relay ingress arrives to trigger recovery.
fn service_historical_recovery_tick(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
) -> Result<HistoricalRecoveryServiceOutcome, V2RunnerError> {
    let current_archive_targets = lane_work
        .has_pending_historical_recovery()
        .then(|| services.current_archive_targets())
        .unwrap_or_default();
    lane_work
        .service_next_historical_recovery_with_archive_targets(&current_archive_targets)
        .map_err(V2RunnerError::from)
}
