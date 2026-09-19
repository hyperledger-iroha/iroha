//! Exact replicated geometry inputs retained before State journals are consumed.
//!
//! This owner binds the actual MV predecessor and accepted successor. It grants
//! no storage publication authority. TODO: join a pure Kura retirement scan and
//! retained route/resource reservation before exposing geometry persistence;
//! the existing raw commit guards remain in force until that owner is complete.

use super::*;

/// Move-only captured geometry inputs; no State or pending-update mutation escapes.
pub(super) struct PreparedCarrierGeometry {
    _header: BlockHeader,
    _predecessor: SnapshotNexusRuntime,
    _successor: SnapshotNexusRuntime,
    _previous_runtime_catalog: Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>,
    _accepted_runtime_catalog: Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>,
    _manifests: LaneManifestRegistryHandle,
    _pending: Option<PendingAutoscaleLaneLifecycle>,
    _certified_frontiers: BTreeMap<(LaneId, DataSpaceId, Hash), LaneDrainFrontierV1>,
}

impl PreparedCarrierGeometry {
    /// Identity-only geometry has no namespace transition to persist. A pending
    /// lifecycle must retain its real geometry/Queue owner before publication.
    /// TODO: consume the captured pending transition through the guarded Kura
    /// geometry publisher; never treat its absence of permission as completion.
    pub(super) fn is_identity_transition(&self, header: BlockHeader) -> bool {
        self._header == header
            && self._pending.is_none()
            && self._certified_frontiers.is_empty()
            && self._previous_runtime_catalog == self._accepted_runtime_catalog
            && self._predecessor.lanes == self._successor.lanes
            && self._predecessor.lane_count == self._successor.lane_count
            && self._predecessor.lane_incarnation_lineage
                == self._successor.lane_incarnation_lineage
            && self._predecessor.owner_policy == self._successor.owner_policy
            && self._predecessor.autoscale_last_transition_height
                == self._successor.autoscale_last_transition_height
    }
}

impl StateBlock<'_> {
    /// Capture original predecessor/successor identity without consulting live caches.
    ///
    /// Call while the original State journals are retained, before decomposition.
    /// No Kura lock, Queue lookup, recovery, filesystem write or second State
    /// scope is acquired here. Storage reservation is deliberately not implied.
    pub(super) fn prepare_carrier_geometry(
        &self,
    ) -> Result<PreparedCarrierGeometry, LaneLifecycleError> {
        self.validate_canonical_runtime_projection()
            .map_err(LaneLifecycleError::Storage)?;
        let predecessor = self.canonical_runtime.get_before_block();
        let successor = self.canonical_runtime.get();
        let previous_nexus = predecessor.nexus_projection(&self.runtime_policy.nexus)?;
        let previous_incarnations = predecessor.active_incarnations()?;
        let previous_activations = predecessor.active_activation_heights()?;
        let previous_lineage = predecessor.lineage_projection();
        let previous_runtime_catalog = predecessor_runtime_catalog(&self.world)?;
        let accepted_runtime_catalog = runtime_catalog_from_world(&self.world)?;
        if runtime_catalog_dataspaces(
            &self.nexus.configured_dataspace_catalog,
            previous_runtime_catalog.as_ref(),
        )? != previous_nexus.dataspace_catalog
        {
            return Err(runtime_catalog_invalid(
                "geometry predecessor differs from its original World catalog",
            ));
        }
        if runtime_catalog_dataspaces(
            &self.nexus.configured_dataspace_catalog,
            accepted_runtime_catalog.as_ref(),
        )? != self.nexus.dataspace_catalog
        {
            return Err(runtime_catalog_invalid(
                "geometry successor differs from its accepted World catalog",
            ));
        }
        let mut certified_frontiers = BTreeMap::new();
        if let Some(pending) = self.pending_autoscale_lifecycle.as_ref() {
            let update = &pending.catalog_update;
            if pending.transition_height != self._curr_block.height().get()
                || update.previous_catalog != previous_nexus.lane_catalog
                || update.previous_dataspace_catalog != previous_nexus.dataspace_catalog
                || update.previous_routing_policy != previous_nexus.routing_policy
                || update.previous_autoscale != previous_nexus.autoscale
                || !lane_config_entries_match(
                    &update.previous_lane_config,
                    &previous_nexus.lane_config,
                )
                || update.previous_lane_incarnations != previous_incarnations
                || update.previous_lane_incarnation_activation_heights != previous_activations
                || update.previous_lane_incarnation_lineage != previous_lineage
            {
                return Err(runtime_catalog_invalid(
                    "geometry transition differs from its captured MV predecessor or carrier height",
                ));
            }
            let actual_root = lane_lifecycle_incarnation_root(
                &previous_nexus.lane_catalog,
                &previous_incarnations,
            )?;
            if pending.expected_incarnation_root != actual_root {
                return Err(LaneLifecycleError::StaleIncarnationRoot {
                    expected: pending.expected_incarnation_root,
                    actual: actual_root,
                });
            }
            if update.updated_catalog != self.nexus.lane_catalog
                || update.updated_dataspace_catalog != self.nexus.dataspace_catalog
                || !lane_config_entries_match(&update.updated_lane_config, &self.nexus.lane_config)
                || update.updated_lane_incarnations != self.lane_incarnations
                || update.updated_lane_incarnation_activation_heights
                    != self.lane_incarnation_activation_heights
                || update.updated_lane_incarnation_lineage != self.lane_incarnation_lineage
                || pending.updated_lane_manifests.consensus_policy_digest()
                    != self.lane_manifests.consensus_policy_digest()
            {
                return Err(runtime_catalog_invalid(
                    "geometry transition differs from its accepted successor or manifest policy",
                ));
            }
            match &pending.runtime_catalog {
                Some(catalog) if accepted_runtime_catalog.as_ref() == Some(catalog) => {}
                None if accepted_runtime_catalog == previous_runtime_catalog => {}
                _ => {
                    return Err(runtime_catalog_invalid(
                        "geometry transition differs from its protected catalog journal",
                    ));
                }
            }
            if pending.transition.requires_geometry() {
                let mut prospective = previous_nexus.clone();
                prospective.dataspace_catalog = update.updated_dataspace_catalog.clone();
                let derivation_header = if let Some(entry) = self
                    .staged_merge_entry
                    .as_ref()
                    .filter(|_| pending.runtime_catalog.is_some())
                    .filter(|entry| entry.execution_batch.is_some())
                {
                    let batch = entry
                        .execution_batch
                        .as_ref()
                        .expect("filtered execution batch");
                    if !crate::merge::merge_execution_batch_commitments_match(batch)
                        || batch.application_block_header
                            != crate::merge::merge_application_header_from_carrier(
                                &self._curr_block,
                            )
                        || batch.application_block_header.height() != self._curr_block.height()
                        || entry.merge_qc.carrier_height != self._curr_block.height().get()
                        || batch.application_block_header.prev_block_hash()
                            != Some(entry.merge_qc.carrier_parent_hash)
                        || batch.application_block_header.view_change_index() != entry.merge_qc.view
                    {
                        return Err(runtime_catalog_invalid(
                            "geometry derivation has no exact certified carrier header",
                        ));
                    }
                    batch.application_block_header.hash()
                } else {
                    self._curr_block.hash()
                };
                let expected = prepare_lane_lifecycle_update(
                    &prospective,
                    &previous_incarnations,
                    &previous_lineage,
                    &previous_activations,
                    &self.network_id,
                    derivation_header,
                    &pending.plan,
                    pending.transition_height,
                    pending.transition != PendingAutoscaleTransition::Manual,
                )?;
                if expected.updated_catalog != update.updated_catalog
                    || !lane_config_entries_match(
                        &expected.updated_lane_config,
                        &update.updated_lane_config,
                    )
                    || expected.updated_lane_incarnations != update.updated_lane_incarnations
                    || expected.updated_lane_incarnation_lineage
                        != update.updated_lane_incarnation_lineage
                    || expected.updated_lane_incarnation_activation_heights
                        != update.updated_lane_incarnation_activation_heights
                    || expected.lanes_to_reset != update.lanes_to_reset
                    || expected.replaced_lane_ids != update.replaced_lane_ids
                {
                    return Err(runtime_catalog_invalid(
                        "geometry successor differs from its exact plan and derivation header",
                    ));
                }
            }
            if let PendingAutoscaleTransition::ScaleIn { lane, .. } = &pending.transition {
                let lane = *lane;
                let previous_lane = previous_nexus
                    .lane_catalog
                    .lanes()
                    .iter()
                    .find(|candidate| candidate.id == lane)
                    .ok_or_else(|| {
                        runtime_catalog_invalid(
                            "geometry retirement is absent from its predecessor",
                        )
                    })?;
                let incarnation = previous_incarnations[&lane];
                let drain = decode_autoscale_lane_drain_state(previous_lane)
                    .map_err(runtime_catalog_invalid)?
                    .ok_or_else(|| {
                        runtime_catalog_invalid("geometry retirement has no captured drain intent")
                    })?;
                let commitment = drain.commitment.ok_or_else(|| {
                    runtime_catalog_invalid("geometry retirement has no captured drain commitment")
                })?;
                if !autoscale_lane_drain_state_matches_context(
                    previous_lane,
                    &drain,
                    &self.network_id,
                    incarnation,
                ) || commitment.carrier_height >= self._curr_block.height().get()
                {
                    return Err(runtime_catalog_invalid(
                        "geometry retirement drain differs from its captured route or height",
                    ));
                }
                certified_frontiers.insert(
                    (lane, previous_lane.dataspace_id, incarnation),
                    commitment.frontier,
                );
            }
        } else if predecessor.lanes != successor.lanes
            || predecessor.lane_count != successor.lane_count
            || predecessor.lane_incarnation_lineage != successor.lane_incarnation_lineage
            || predecessor.owner_policy != successor.owner_policy
            || predecessor.autoscale_last_transition_height
                != successor.autoscale_last_transition_height
            || accepted_runtime_catalog != previous_runtime_catalog
        {
            return Err(runtime_catalog_invalid(
                "geometry changed without its exact pending lifecycle owner",
            ));
        }
        Ok(PreparedCarrierGeometry {
            _header: self._curr_block,
            _predecessor: predecessor.clone(),
            _successor: successor.clone(),
            _previous_runtime_catalog: previous_runtime_catalog,
            _accepted_runtime_catalog: accepted_runtime_catalog,
            _manifests: Arc::clone(&self.lane_manifests),
            _pending: self.pending_autoscale_lifecycle.clone(),
            _certified_frontiers: certified_frontiers,
        })
    }
}

fn predecessor_runtime_catalog(
    world: &WorldBlock<'_>,
) -> Result<Option<iroha_data_model::nexus::NexusRuntimeCatalogV1>, LaneLifecycleError> {
    runtime_catalog_from_parameters(world.parameters.get_before_block())
}

#[cfg(test)]
#[path = "carrier_geometry_preparation_tests.rs"]
mod tests;
