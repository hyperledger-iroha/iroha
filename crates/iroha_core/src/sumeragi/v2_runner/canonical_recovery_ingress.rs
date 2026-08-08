fn drain_canonical_executed_block_recovery_ingress(
    receiver: &FairV2Ingress,
    recovery: &mut CanonicalExecutedBlockRecovery,
    limit: usize,
) -> Result<usize, V2RunnerError> {
    let mut drained = 0usize;
    for _ in 0..limit.max(1) {
        let Some(inbound) = receiver
            .try_recv_if_checked(|inbound| recovery.admits_message(inbound.message()))
            .map_err(V2RunnerError::Service)?
        else {
            break;
        };
        let _ = recovery.accept_with_ingress_ownership(inbound)?;
        drained = drained.saturating_add(1);
    }
    Ok(drained)
}

fn dispatch_canonical_executed_block_recovery_effects(
    recovery: &mut CanonicalExecutedBlockRecovery,
    services: &ProductionV2Services,
    limit: usize,
) -> Result<usize, V2RunnerError> {
    let scan_limit = recovery.effect_count();
    let mut dispatched = 0usize;
    for _ in 0..scan_limit.min(limit.max(1)) {
        let Some(effect) = recovery.drain_effects(1).pop() else {
            break;
        };
        if !matches!(&effect, V2LaneWorkEffect::PostLaneBlock { .. }) {
            return Err(V2RunnerError::Service(
                "canonical executed-block recovery emitted a non-recovery lane effect".to_owned(),
            ));
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
                dispatched = dispatched.saturating_add(1);
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
    Ok(dispatched)
}
