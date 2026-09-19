//! Exact replicated geometry inputs retained before State journals are consumed.
//!
//! This owner binds the actual MV predecessor and accepted successor. It grants
//! no storage publication authority. The private consuming carrier can retain
//! exact raw/tiered progress under an already-held Kura lease. TODO: join source,
//! retirement and complete pre-vote resource owners before exposing that consumer;
//! physical custody alone cannot authorize geometry or State publication.

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
    network_id: NetworkId,
    raw: Option<crate::kura::RawGeometryAttempt>,
    tiered: Option<tiered::TieredGeometryAttempt>,
    // Keep the original Kura alive behind all captured physical custody.
    kura: Arc<Kura>,
}

/// Borrowed storage completion at this exact held boundary, not State permission.
/// Keeping both owners borrowed prevents a mapping handoff from outliving the
/// original geometry or the lease which authenticated its completed catalog.
pub(super) struct CompletedCarrierGeometry<'geometry, 'kura> {
    geometry: &'geometry PreparedCarrierGeometry,
    _lease: &'geometry crate::kura::KuraPublicationLease<'kura>,
}

impl CompletedCarrierGeometry<'_, '_> {
    /// Original updated mapping for the enclosing admitted State-effect owner.
    /// This does not mutate DA cursors or permit rebuilding a mapping from State.
    pub(super) fn updated_da_mapping(&self) -> Option<&LaneConfig> {
        self.geometry
            ._pending
            .as_ref()
            .filter(|pending| pending.transition.requires_geometry())
            .map(|pending| &pending.catalog_update.updated_lane_config)
    }
}

impl PreparedCarrierGeometry {
    /// Capture descriptors once without changing files, catalogs or State caches.
    /// The enclosing owner must cover this preparation with retained capacity;
    /// this seam supplies no admission policy or publication authorization.
    pub(super) fn prepare_under(
        &mut self,
        backend: &tiered::TieredStateBackend,
        lease: &crate::kura::KuraPublicationLease<'_>,
    ) -> Result<(), LaneLifecycleError> {
        if !lease.belongs_to(&self.kura) {
            return Err(LaneLifecycleError::Storage(
                "carrier geometry lease belongs to another original Kura".to_owned(),
            ));
        }
        let Some(pending) = self
            ._pending
            .as_ref()
            .filter(|pending| pending.transition.requires_geometry())
        else {
            return Ok(());
        };
        let update = &pending.catalog_update;
        if self.tiered.is_none() {
            let diff = lane_topology_diff(
                &update.previous_lane_config,
                &update.updated_lane_config,
                &update.replaced_lane_ids,
            );
            self.tiered = Some(
                backend
                    .prepare_lane_geometry_attempt(
                        &update.previous_lane_config,
                        &update.updated_lane_config,
                        &diff.replacements,
                        &diff.relabelled,
                    )
                    .map_err(|error| {
                        LaneLifecycleError::Storage(format!(
                            "carrier tiered geometry preparation: {error:#}"
                        ))
                    })?,
            );
        }
        self.tiered
            .as_ref()
            .expect("tiered descriptor was retained above")
            .authenticate_backend(backend)
            .map_err(|error| {
                LaneLifecycleError::Storage(format!("carrier tiered geometry binding: {error:#}"))
            })?;
        if self.raw.is_none() {
            let request = crate::kura::ReplayGeometryBindingRequest {
                previous: &update.previous_lane_config,
                updated: &update.updated_lane_config,
                previous_incarnations: &update.previous_lane_incarnations,
                updated_incarnations: &update.updated_lane_incarnations,
                previous_activation_heights: &update.previous_lane_incarnation_activation_heights,
                updated_activation_heights: &update.updated_lane_incarnation_activation_heights,
                previous_lineage_root: lane_incarnation_lineage_root(
                    &self.network_id,
                    &update.previous_lane_incarnation_lineage,
                ),
                updated_lineage_root: lane_incarnation_lineage_root(
                    &self.network_id,
                    &update.updated_lane_incarnation_lineage,
                ),
                transition_height: self._header.height().get(),
            };
            self.raw = Some(
                lease
                    .begin_raw_geometry_attempt(
                        &request,
                        &update.replaced_lane_ids,
                        &self._certified_frontiers,
                    )
                    .map_err(LaneLifecycleError::GeometryStorage)?,
            );
        }
        Ok(())
    }

    /// Resume only previously prepared operations under the original Kura lease.
    /// Only the private, eventually authorized carrier consumer may invoke this
    /// seam. Success retains the raw claim at FilesApplied; it neither publishes
    /// the catalog nor updates DA cursors, manifests, runtime cells or World.
    /// A catalog retry only rechecks completed tiered work; it cannot return to
    /// raw Apply or create replacement descriptors after choosing publication.
    pub(super) fn resume_under(
        &mut self,
        backend: &mut tiered::TieredStateBackend,
        lease: &crate::kura::KuraPublicationLease<'_>,
    ) -> Result<(), LaneLifecycleError> {
        if !lease.belongs_to(&self.kura) {
            return Err(LaneLifecycleError::Storage(
                "carrier geometry lease belongs to another original Kura".to_owned(),
            ));
        }
        if !self
            ._pending
            .as_ref()
            .is_some_and(|pending| pending.transition.requires_geometry())
        {
            return Ok(());
        }
        let tiered = self.tiered.as_mut().ok_or_else(|| {
            LaneLifecycleError::Storage(
                "carrier geometry has no previously prepared tiered owner".to_owned(),
            )
        })?;
        let raw = self.raw.as_mut().ok_or_else(|| {
            LaneLifecycleError::Storage(
                "carrier geometry has no previously prepared raw owner".to_owned(),
            )
        })?;
        tiered.authenticate_backend(backend).map_err(|error| {
            LaneLifecycleError::Storage(format!("carrier tiered geometry binding: {error:#}"))
        })?;
        match raw.phase() {
            crate::kura::RawGeometryPhase::PublishingCatalog
            | crate::kura::RawGeometryPhase::CatalogPublished => {
                tiered.authenticate_applied(backend).map_err(|error| {
                    LaneLifecycleError::Storage(format!(
                        "retained carrier completed tiered geometry: {error:#}"
                    ))
                })?;
                if raw.phase() == crate::kura::RawGeometryPhase::CatalogPublished {
                    raw.reauthenticate_catalog_under(lease)
                        .map_err(LaneLifecycleError::GeometryStorage)?;
                }
            }
            _ => {
                raw.resume_under(lease)
                    .map_err(LaneLifecycleError::GeometryStorage)?;
                tiered.resume(backend).map_err(|error| {
                    LaneLifecycleError::Storage(format!(
                        "retained carrier tiered geometry: {error:#}"
                    ))
                })?;
            }
        }
        Ok(())
    }

    /// Finish the exact retained storage operation before any State visibility.
    /// Refusal leaves every journal, pending sync and namespace receipt in this
    /// owner. Success authenticates the original catalog under the same lease;
    /// it grants no source, resource, retirement or State publication authority.
    pub(super) fn complete_under<'geometry, 'kura>(
        &'geometry mut self,
        backend: &mut tiered::TieredStateBackend,
        lease: &'geometry crate::kura::KuraPublicationLease<'kura>,
    ) -> Result<CompletedCarrierGeometry<'geometry, 'kura>, LaneLifecycleError> {
        self.resume_under(backend, lease)?;
        if let Some(raw) = &mut self.raw {
            if raw.phase() != crate::kura::RawGeometryPhase::CatalogPublished {
                raw.publish_catalog_under(lease, None)
                    .map_err(LaneLifecycleError::GeometryStorage)?;
            }
            raw.reauthenticate_catalog_under(lease)
                .map_err(LaneLifecycleError::GeometryStorage)?;
        }
        Ok(CompletedCarrierGeometry {
            geometry: self,
            _lease: lease,
        })
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
            network_id: self.network_id,
            raw: None,
            tiered: None,
            kura: Arc::clone(&self.state_ref.kura),
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
