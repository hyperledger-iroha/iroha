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
    loop {
        apply_certified_merge_sidecar_closed_prefixes(&mut lane_work, services)?;
        apply_certified_merge_sidecar_chunk_admissions(
            &mut lane_work,
            services,
            control_queue_capacity,
        )?;
        let retired = services
            .handoff_applied_height_output_to_durable_reconstruction(
                receipt,
                artifact,
                &durable_lane_authority,
            )
            .map_err(V2RunnerError::Service)?;
        apply_certified_merge_sidecar_chunk_admissions(
            &mut lane_work,
            services,
            control_queue_capacity,
        )?;
        if !services
            .has_pending_exact_output()
            .map_err(V2RunnerError::Service)?
        {
            break;
        }
        if retired == 0 {
            return Err(V2RunnerError::Service(
                "finalized exact output has no durable or move-only successor source".to_owned(),
            ));
        }
    }
    let _ = services
        .handoff_applied_height_output_to_durable_reconstruction(
            receipt,
            artifact,
            &durable_lane_authority,
        )
        .map_err(V2RunnerError::Service)?;
    if lane_work.has_pending_committed_output_handoff()
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
