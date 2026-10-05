/// Bind the protected World parameter and new committees to its retained predecessor.
/// The owned validator additionally binds frozen policy and both runtime journals.
fn validate_runtime_catalog_block_overlay(
    original_parameters: &Parameters,
    accepted_world: &impl WorldReadOnly,
    network_id: &iroha_data_model::NetworkId,
    original_nexus: &iroha_config::parameters::actual::Nexus,
    pending: Option<&PendingAutoscaleLaneLifecycle>,
    block_height: u64,
) -> Result<(), LaneLifecycleError> {
    use iroha_data_model::nexus::NexusRuntimeCatalogV1;
    let id = NexusRuntimeCatalogV1::parameter_id();
    let _old_runtime = runtime_catalog_from_parameters(original_parameters)?;
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
    /// Validate frozen policy and actual predecessor/current journals before publication.
    #[allow(clippy::too_many_lines)]
    fn validate_owned_runtime_catalog_overlay(&self) -> Result<(), LaneLifecycleError> {
        // Static execution inputs belong to the captured owner. A later physical
        // cache is neither a new policy authority nor this block's predecessor.
        let captured_digest =
            iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
                &self.runtime_policy.nexus,
                self.runtime_policy
                    .compliance
                    .as_deref()
                    .map(LaneComplianceEngine::consensus_policy_digest),
                Some(
                    self.runtime_policy
                        .manifests
                        .baseline_consensus_policy_digest(),
                ),
            )
            .map_err(runtime_catalog_invalid)?;
        let working_digest =
            iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
                &self.nexus,
                self.lane_compliance
                    .as_deref()
                    .map(LaneComplianceEngine::consensus_policy_digest),
                Some(self.lane_manifests.baseline_consensus_policy_digest()),
            )
            .map_err(runtime_catalog_invalid)?;
        if working_digest != captured_digest
            || compute_zk_consensus_policy_hash(&self.zk) != self.runtime_policy.zk_hash
        {
            return Err(runtime_catalog_invalid(
                "runtime catalog working policy differs from its captured authority",
            ));
        }
        self.validate_canonical_runtime_projection()
            .map_err(runtime_catalog_invalid)?;
        let original_parameters = self.world.parameters.get_before_block();
        let original_catalog = runtime_catalog_from_parameters(original_parameters)?;
        let original_runtime = self.canonical_runtime.get_before_block();
        let predecessor = original_runtime
            .nexus_projection_with_catalog(&self.runtime_policy.nexus, original_catalog.as_ref())?;
        let accepted_catalog = runtime_catalog_from_world(&self.world)?;
        // This joins the actual current runtime owner to the same accepted World
        // catalog rather than validating two independent, merely plausible views.
        self.canonical_runtime
            .get()
            .nexus_projection_with_catalog(&self.runtime_policy.nexus, accepted_catalog.as_ref())?;
        if self.world.dataspace_catalog != self.nexus.dataspace_catalog {
            return Err(runtime_catalog_invalid(
                "accepted World dataspace projection differs from its runtime owner",
            ));
        }
        let height = self._curr_block.height().get();
        validate_runtime_catalog_block_overlay(
            original_parameters,
            &self.world,
            &self.network_id,
            &predecessor,
            self.pending_autoscale_lifecycle.as_ref(),
            height,
        )?;
        let incarnations = original_runtime.active_incarnations()?;
        let activation_heights = original_runtime.active_activation_heights()?;
        let lineage = original_runtime.lineage_projection();
        let Some(pending) = self.pending_autoscale_lifecycle.as_ref() else {
            // Self-consistency cannot authorize an unstaged topology or lineage.
            // Ordinary samples may advance, but their original physical geometry
            // and transition cursor still belong to this scope's retained undo.
            if self.nexus.lane_catalog != predecessor.lane_catalog
                || !lane_config_entries_match(&self.nexus.lane_config, &predecessor.lane_config)
                || self.lane_incarnations != incarnations
                || self.lane_incarnation_activation_heights != activation_heights
                || self.lane_incarnation_lineage != lineage
                || self.nexus.autoscale.last_transition_height
                    != original_runtime.autoscale_last_transition_height
            {
                return Err(runtime_catalog_invalid(
                    "unstaged runtime geometry differs from its retained original owner",
                ));
            }
            if self.lane_manifests.consensus_policy_digest()
                != self.runtime_policy.manifests.consensus_policy_digest()
                || (!Arc::ptr_eq(&self.runtime_policy.manifests, &self.lane_manifests)
                    && !self
                        .runtime_policy
                        .manifests
                        .has_same_authority_as(&self.lane_manifests))
            {
                return Err(runtime_catalog_invalid(
                    "unstaged runtime catalog manifests differ from their captured source authority",
                ));
            }
            return Ok(());
        };
        ensure_physical_catalog_additions_only(&pending.plan)?;
        if pending.transition_height != height {
            return Err(LaneLifecycleError::AutoscaleTransitionPlanMismatch {
                transition: pending.transition.name(),
                reason: "transition_height must match the committing block height",
            });
        }
        let update = &pending.catalog_update;
        if predecessor.lane_catalog != update.previous_catalog
            || predecessor.dataspace_catalog != update.previous_dataspace_catalog
            || predecessor.routing_policy != update.previous_routing_policy
            || predecessor.autoscale != update.previous_autoscale
            || !lane_config_entries_match(&predecessor.lane_config, &update.previous_lane_config)
            || incarnations != update.previous_lane_incarnations
            || activation_heights != update.previous_lane_incarnation_activation_heights
            || lineage != update.previous_lane_incarnation_lineage
        {
            return Err(runtime_catalog_invalid(
                "staged runtime catalog predecessor differs from the retained World and runtime undo",
            ));
        }
        let actual_root =
            lane_lifecycle_incarnation_root(&predecessor.lane_catalog, &incarnations)?;
        if pending.expected_incarnation_root != actual_root {
            return Err(LaneLifecycleError::StaleIncarnationRoot {
                expected: pending.expected_incarnation_root,
                actual: actual_root,
            });
        }
        let mut prospective = predecessor.clone();
        if let Some(catalog) = &pending.runtime_catalog {
            prospective.dataspace_catalog = runtime_catalog_transition_dataspaces_from_parameters(
                &predecessor,
                self.runtime_policy.manifests.as_ref(),
                original_parameters,
                catalog,
                &pending.plan,
            )?;
        }
        ensure_lane_lifecycle_compliance_ready(
            &prospective,
            self.runtime_policy.compliance.as_deref(),
            &pending.plan,
        )?;
        let mut expected = prepare_lane_lifecycle_update(
            &prospective,
            &incarnations,
            &lineage,
            &activation_heights,
            &self.network_id,
            self._curr_block.hash(),
            &pending.plan,
            height,
            false,
        )?;
        ensure_runtime_catalog_lanes_preserved_from_parameters(
            original_parameters,
            &predecessor.lane_catalog,
            &expected.updated_catalog,
        )?;
        expected.previous_dataspace_catalog = predecessor.dataspace_catalog.clone();
        expected.updated_dataspace_catalog = prospective.dataspace_catalog.clone();
        if expected.updated_catalog != update.updated_catalog
            || expected.updated_dataspace_catalog != update.updated_dataspace_catalog
            || !lane_config_entries_match(
                &expected.updated_lane_config,
                &update.updated_lane_config,
            )
            || expected.lanes_to_reset != update.lanes_to_reset
            || expected.replaced_lane_ids != update.replaced_lane_ids
            || expected.updated_lane_incarnations != update.updated_lane_incarnations
            || expected.updated_lane_incarnation_lineage != update.updated_lane_incarnation_lineage
            || expected.updated_lane_incarnation_activation_heights
                != update.updated_lane_incarnation_activation_heights
            || self.nexus.lane_catalog != expected.updated_catalog
            || self.nexus.dataspace_catalog != expected.updated_dataspace_catalog
            || !lane_config_entries_match(&self.nexus.lane_config, &expected.updated_lane_config)
            || self.lane_incarnations != expected.updated_lane_incarnations
            || self.lane_incarnation_lineage != expected.updated_lane_incarnation_lineage
            || self.lane_incarnation_activation_heights
                != expected.updated_lane_incarnation_activation_heights
        {
            return Err(runtime_catalog_invalid(
                "accepted runtime catalog update differs from the signed plan and retained predecessor",
            ));
        }
        let manifests = if let Some(catalog) = &pending.runtime_catalog {
            Arc::new(
                self.runtime_policy
                    .manifests
                    .with_runtime_additions(
                        &catalog.manifests,
                        &expected.updated_catalog,
                        &expected.updated_dataspace_catalog,
                        &predecessor.governance,
                    )
                    .map_err(runtime_catalog_invalid)?,
            )
        } else {
            rebind_lane_manifests_for_lifecycle(
                self.runtime_policy.manifests.as_ref(),
                &expected.updated_catalog,
                &predecessor.governance,
            )?
        };
        for accepted in [&pending.updated_lane_manifests, &self.lane_manifests] {
            if manifests.consensus_policy_digest() != accepted.consensus_policy_digest()
                || !manifests.has_same_authority_as(accepted)
            {
                return Err(runtime_catalog_invalid(
                    "accepted runtime catalog manifests differ from their captured source authority",
                ));
            }
        }
        Ok(())
    }
}

/// The committed overlay authorizes additive lanes; ordinary lifecycle cannot replace or retire
/// that authority while retaining its signed manifest in the protected catalog.
fn ensure_runtime_catalog_lanes_preserved(
    world: &impl WorldReadOnly,
    previous: &LaneCatalog,
    updated: &LaneCatalog,
) -> Result<(), LaneLifecycleError> {
    ensure_runtime_catalog_lanes_preserved_from_parameters(world.parameters(), previous, updated)
}

/// Preserve only additions actually owned by the retained parameter predecessor.
fn ensure_runtime_catalog_lanes_preserved_from_parameters(
    parameters: &Parameters,
    previous: &LaneCatalog,
    updated: &LaneCatalog,
) -> Result<(), LaneLifecycleError> {
    let Some(runtime) = runtime_catalog_from_parameters(parameters)? else {
        return Ok(());
    };
    for manifest in &runtime.manifests {
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
