/// Authenticate the dynamic predecessor and static policy from the carrier's
/// retained originals, before any replay preview or publication pruning.
#[expect(
    clippy::too_many_arguments,
    reason = "each carrier-owned policy and MV original is independently bound"
)]
fn runtime_catalog_carrier_predecessor(
    original_runtime: &SnapshotNexusRuntime,
    original_parameters: &Parameters,
    policy: &canonical_runtime::CapturedRuntimePolicy,
    nexus: &iroha_config::parameters::actual::Nexus,
    manifests: &LaneManifestRegistry,
    compliance: Option<&LaneComplianceEngine>,
    zk: &iroha_config::parameters::actual::Zk,
    pending: Option<&PendingAutoscaleLaneLifecycle>,
) -> Result<iroha_config::parameters::actual::Nexus, LaneLifecycleError> {
    let old_catalog = runtime_catalog_from_parameters(original_parameters)?;
    let predecessor =
        original_runtime.nexus_projection_with_catalog(&policy.nexus, old_catalog.as_ref())?;
    let Some(pending) = pending.filter(|value| value.runtime_catalog.is_some()) else {
        return Ok(predecessor);
    };
    let update = &pending.catalog_update;
    let original_incarnations = original_runtime.active_incarnations()?;
    let original_heights = original_runtime.active_activation_heights()?;
    if update.previous_catalog != predecessor.lane_catalog
        || update.previous_dataspace_catalog != predecessor.dataspace_catalog
        || update.previous_lane_config != predecessor.lane_config
        || update.previous_routing_policy != predecessor.routing_policy
        || update.previous_autoscale != predecessor.autoscale
        || update.previous_lane_incarnations != original_incarnations
        || update.previous_lane_incarnation_activation_heights != original_heights
        || update.previous_lane_incarnation_lineage != original_runtime.lineage_projection()
        || update.previous_storage_geometry
            != historical_lane_geometry(
                &predecessor.lane_catalog,
                &original_incarnations,
                &original_heights,
                old_catalog.as_ref(),
            )?
    {
        return Err(runtime_catalog_invalid(
            "catalog transition predecessor differs from its original MV runtime",
        ));
    }
    let mut expected = predecessor.clone();
    runtime_catalog_project_retired_routes(&mut expected, pending.runtime_catalog.as_ref())?;
    expected.lane_catalog = update.updated_catalog.clone();
    expected.lane_config = update.updated_lane_config.clone();
    expected.dataspace_catalog = update.updated_dataspace_catalog.clone();
    if nexus.lane_catalog != expected.lane_catalog
        || nexus.lane_config != expected.lane_config
        || nexus.dataspace_catalog != expected.dataspace_catalog
        || update.updated_routing_policy != expected.routing_policy
        || update.updated_autoscale != expected.autoscale
    {
        return Err(runtime_catalog_invalid(
            "catalog transition working projection differs from its original policy",
        ));
    }
    let expected_policy =
        iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
            &expected,
            policy
                .compliance
                .as_deref()
                .map(LaneComplianceEngine::consensus_policy_digest),
            Some(policy.manifests.baseline_consensus_policy_digest()),
        )
        .map_err(runtime_catalog_invalid)?;
    let current_policy =
        iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
            nexus,
            compliance.map(LaneComplianceEngine::consensus_policy_digest),
            Some(manifests.baseline_consensus_policy_digest()),
        )
        .map_err(runtime_catalog_invalid)?;
    if current_policy != expected_policy || compute_zk_consensus_policy_hash(zk) != policy.zk_hash {
        return Err(runtime_catalog_invalid(
            "catalog transition substituted its captured native execution policy",
        ));
    }
    Ok(predecessor)
}

/// Bind the accepted World to its actual MV predecessor and frozen policy.
/// A later live State view or rebuildable runtime cache is not this authority.
fn validate_runtime_catalog_block_overlay(
    original_parameters: &Parameters,
    accepted_world: &impl WorldReadOnly,
    network_id: &iroha_data_model::NetworkId,
    original_nexus: &iroha_config::parameters::actual::Nexus,
    pending: Option<&PendingAutoscaleLaneLifecycle>,
    block_height: u64,
    now_ms: u64,
) -> Result<(), LaneLifecycleError> {
    use iroha_data_model::nexus::NexusRuntimeCatalogV1;
    let id = NexusRuntimeCatalogV1::parameter_id();
    let old_runtime = runtime_catalog_from_parameters(original_parameters)?;
    let accepted_runtime = runtime_catalog_from_world(accepted_world)?;
    let old_parameter = original_parameters.custom().get(&id);
    let accepted_parameter = accepted_world.parameters().custom().get(&id);
    let Some(pending) = pending.filter(|pending| pending.runtime_catalog.is_some()) else {
        if old_parameter != accepted_parameter {
            return Err(runtime_catalog_invalid(
                "protected runtime catalog changed without a staged catalog transition",
            ));
        }
        return Ok(());
    };
    if block_height <= 1 || pending.transition != PendingAutoscaleTransition::Manual {
        return Err(runtime_catalog_invalid(
            "runtime catalog authority requires a signed post-genesis manual transition",
        ));
    }
    if accepted_runtime.as_ref() != pending.runtime_catalog.as_ref() {
        return Err(runtime_catalog_invalid(
            "accepted World catalog differs from the prepared cumulative catalog",
        ));
    }
    let accepted_runtime = accepted_runtime
        .as_ref()
        .expect("pending catalog equals accepted catalog");
    validate_runtime_retirement_delta(
        old_runtime.as_ref(),
        accepted_runtime,
        original_nexus,
        &pending.plan,
    )?;
    validate_physical_retirement_accepted_world(
        old_runtime.as_ref(),
        accepted_runtime,
        original_nexus,
        accepted_world,
        &pending.catalog_update,
        block_height,
        now_ms,
    )?;
    let authority_height = block_height
        .checked_add(1)
        .ok_or_else(|| runtime_catalog_invalid("runtime catalog activation height overflows"))?;
    let mut nexus = original_nexus.clone();
    nexus.dataspace_catalog = pending.catalog_update.updated_dataspace_catalog.clone();
    nexus.lane_catalog = pending.catalog_update.updated_catalog.clone();
    nexus.lane_config = pending.catalog_update.updated_lane_config.clone();
    for lane in &pending.plan.additions {
        validate_runtime_catalog_committee(
            accepted_world,
            network_id,
            &nexus,
            &pending.updated_lane_manifests,
            lane,
            authority_height,
        )?;
    }
    Ok(())
}

impl StateBlock<'_> {
    /// Validate the actual block journals without opening a competing live view.
    fn validate_owned_runtime_catalog_overlay(&self) -> Result<(), LaneLifecycleError> {
        let predecessor = runtime_catalog_carrier_predecessor(
            self.canonical_runtime.get_before_block(),
            self.world.parameters.get_before_block(),
            &self.runtime_policy,
            &self.nexus,
            &self.lane_manifests,
            self.lane_compliance.as_deref(),
            &self.zk,
            self.pending_autoscale_lifecycle.as_ref(),
        )?;
        validate_runtime_catalog_block_overlay(
            self.world.parameters.get_before_block(),
            &self.world,
            &self.network_id,
            &predecessor,
            self.pending_autoscale_lifecycle.as_ref(),
            self._curr_block.height().get(),
            self._curr_block
                .creation_time()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }
}

/// The committed overlay authorizes additive lanes; ordinary lifecycle cannot replace or retire
/// that authority while retaining its signed manifest in the protected catalog.
fn ensure_runtime_catalog_lanes_preserved(
    world: &impl WorldReadOnly,
    previous: &LaneCatalog,
    updated: &LaneCatalog,
) -> Result<(), LaneLifecycleError> {
    let Some(runtime) = runtime_catalog_from_world(world)? else {
        return Ok(());
    };
    for manifest in &runtime.manifests {
        if runtime
            .retired_lanes
            .iter()
            .any(|record| record.lane.id == manifest.lane_id)
        {
            continue;
        }
        let old = previous
            .lanes()
            .iter()
            .find(|lane| lane.id == manifest.lane_id);
        let new = updated
            .lanes()
            .iter()
            .find(|lane| lane.id == manifest.lane_id);
        if old.is_none() || old != new {
            return Err(runtime_catalog_invalid(
                "ordinary lifecycle cannot replace or retire a committed runtime lane",
            ));
        }
    }
    Ok(())
}
