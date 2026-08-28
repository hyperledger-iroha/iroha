fn drain_finalized_lane_work_output(
    lane_work: &mut V2LaneWorkAdapter,
    services: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    artifact: &wire::finality::V2FinalityArtifact,
    durable_lane_authority: &DurableLaneRolloverAuthority,
    control_queue_capacity: usize,
) -> Result<(), V2RunnerError> {
    loop {
        let _ = apply_retired_merge_sidecar_requests(lane_work, services)?;
        let _ = apply_obsolete_merge_sidecar_generation_hints(lane_work, services)?;
        let _ = apply_acknowledged_merge_sidecar_closes(lane_work, services)?;
        apply_certified_merge_sidecar_closed_prefixes(lane_work, services)?;
        apply_certified_merge_sidecar_chunk_admissions(
            lane_work,
            services,
            control_queue_capacity,
        )?;
        let retired = services
            .handoff_applied_height_output_to_durable_reconstruction(
                receipt,
                artifact,
                durable_lane_authority,
            )
            .map_err(V2RunnerError::Service)?;
        apply_certified_merge_sidecar_chunk_admissions(
            lane_work,
            services,
            control_queue_capacity,
        )?;

        let before = lane_work.effect_count();
        let dispatched =
            dispatch_lane_work_effects_with_progress(lane_work, services, control_queue_capacity)?;
        let after = lane_work.effect_count();
        let pending = services
            .has_pending_exact_output()
            .map_err(V2RunnerError::Service)?;
        // Seal only after one quiescent pass. A just-dispatched reply route can
        // become parked and therefore report no dispatchable pending work even
        // though its move-only fanout still needs the next durable handoff.
        if retired == 0 && dispatched == 0 && after == 0 && !pending {
            return Ok(());
        }
        if retired == 0 && dispatched == 0 && after >= before {
            return Err(V2RunnerError::Service(
                "finalized lane output made no progress toward exact handoff".to_owned(),
            ));
        }
    }
}

/// Keep the finalized height alive until every winning lane session is
/// independently reconstructible from this adapter's own Kura.
///
/// Canonical body recovery can fan out the locally authored proposal, so the
/// caller records that one-shot transition while repeating the idempotent
/// certificate/application persistence check on every readiness turn.
pub(super) fn preflight_finalized_lane_rollover(
    executor: &V2EffectExecutor<SerializedV2Runtime>,
    lane_work: &mut V2LaneWorkAdapter,
    canonical_lane_body_recovered: &mut bool,
) -> Result<bool, V2RunnerError> {
    if !executor.ready_to_finish() {
        return Ok(false);
    }
    let (receipt, artifact) = executor.durable_finality().ok_or_else(|| {
        V2RunnerError::Service("ready Sumeragi executor lost its durable finality owner".to_owned())
    })?;
    if !*canonical_lane_body_recovered {
        let _ = lane_work.recover_decided_canonical_lane_body(receipt, artifact)?;
        *canonical_lane_body_recovered = true;
    }
    let _ = lane_work.persist_anchored_sessions()?;
    let _ = lane_work.service_next_historical_recovery()?;
    if lane_work.has_pending_historical_recovery() {
        return Ok(false);
    }
    lane_work
        .durable_completion_matches_finality(artifact)
        .map_err(V2RunnerError::from)
}

fn rollover_finalized_height_outputs(
    mut lane_work: V2LaneWorkAdapter,
    services: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    artifact: &wire::finality::V2FinalityArtifact,
    successor: &wire::HeightContext,
    control_queue_capacity: usize,
) -> Result<RetainedMergeSidecars, V2RunnerError> {
    // Finality makes every current-height global/lane output either
    // Kura-reconstructible or explicitly superseded. Move those owners across
    // one local durable boundary before the successor opens the merge journal;
    // no predecessor thread survives this function.
    let _ = retry_exact_output_and_apply_sidecar_admissions(
        &mut lane_work,
        services,
        control_queue_capacity,
    )?;
    let _ = lane_work.recover_decided_canonical_lane_body(receipt, artifact)?;
    lane_work.persist_anchored_sessions()?;
    let _ = lane_work.service_next_historical_recovery()?;
    if lane_work.has_pending_historical_recovery() {
        return Err(V2RunnerError::Service(
            "finalized lane output still owns predecessor-height recovery".to_owned(),
        ));
    }
    if !lane_work.durable_completion_matches_finality(artifact)? {
        return Err(V2RunnerError::Service(
            "finalized lane output has not crossed its local durable completion boundary"
                .to_owned(),
        ));
    }
    lane_work.prepare_canonical_lane_rollover(artifact)?;
    let durable_lane_authority = lane_work
        .durable_lane_rollover_authority(artifact)?
        .ok_or_else(|| {
            V2RunnerError::Service(
                "finalized lane output has not crossed its local durable reconstruction boundary"
                    .to_owned(),
            )
        })?;
    lane_work.prune_finalized_merge_sidecars()?;
    lane_work.retain_successor_owned_rollover_effects(artifact, &durable_lane_authority)?;
    drain_finalized_lane_work_output(
        &mut lane_work,
        services,
        receipt,
        artifact,
        &durable_lane_authority,
        control_queue_capacity,
    )?;
    if lane_work.has_pending_committed_output_handoff()
        || lane_work.effect_count() != 0
        || services
            .has_pending_exact_output()
            .map_err(V2RunnerError::Service)?
    {
        return Err(V2RunnerError::Service(
            "finalized output remained owned after durable handoff".to_owned(),
        ));
    }
    let exact_output_handoff = services
        .seal_applied_height_output_handoff(receipt, artifact, &durable_lane_authority)
        .map_err(V2RunnerError::Service)?;
    lane_work
        .into_retained_merge_sidecars(exact_output_handoff, artifact, successor)
        .map_err(V2RunnerError::from)
}

/// Run the existing finalized-output handoff from the opaque lifecycle state.
///
/// The lifecycle module receives only the retained successor sidecars or one
/// fail-stop diagnostic; no runner construction or lane/service parts escape.
pub(in crate::sumeragi) fn rollover_finalized_height_outputs_for_lifecycle(
    _permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1,
    lane_work: V2LaneWorkAdapter,
    services: &ProductionV2Services,
    receipt: &KuraV2CommitReceipt,
    artifact: &wire::finality::V2FinalityArtifact,
    successor: &wire::HeightContext,
    control_queue_capacity: usize,
) -> Result<RetainedMergeSidecars, String> {
    rollover_finalized_height_outputs(
        lane_work,
        services,
        receipt,
        artifact,
        successor,
        control_queue_capacity,
    )
    .map_err(|error| error.to_string())
}
