// One process-owned lane geometry operation. Local I/O errors return to the
// caller without discarding either storage plan or recapturing moved paths.

struct LaneGeometryPublication {
    raw: Option<crate::kura::RawGeometryAttempt>,
    tiered: Option<tiered::TieredGeometryAttempt>,
    previous: LaneConfig,
    current: LaneConfig,
    previous_incarnations: BTreeMap<LaneId, Hash>,
    current_incarnations: BTreeMap<LaneId, Hash>,
    previous_activation_heights: BTreeMap<LaneId, u64>,
    current_activation_heights: BTreeMap<LaneId, u64>,
    previous_lineage_root: Hash,
    current_lineage_root: Hash,
    replaced: BTreeSet<LaneId>,
    certified: BTreeMap<(LaneId, DataSpaceId, Hash), LaneDrainFrontierV1>,
    startup_owner: Option<Arc<()>>,
    startup_attached: bool,
    transition_height: u64,
    cursors_updated: bool,
    publish_cursors: bool,
}

fn geometry_lease_error(error: crate::kura::KuraPublicationPreparationError) -> LaneLifecycleError {
    match error {
        crate::kura::KuraPublicationPreparationError::Busy { field, wait } => {
            LaneLifecycleError::PublicationBusy { field, wait }
        }
        crate::kura::KuraPublicationPreparationError::Storage(error) => {
            LaneLifecycleError::GeometryStorage(error)
        }
    }
}

impl State {
    fn resume_lane_geometry_publication(
        &self,
        request: &crate::kura::ReplayGeometryBindingRequest<'_>,
        replaced: &BTreeSet<LaneId>,
        certified: &BTreeMap<(LaneId, DataSpaceId, Hash), LaneDrainFrontierV1>,
        mut startup: Option<&mut crate::kura::StartupReplayGeometryTransition>,
        mut releases: Option<&mut LaneLifecycleReleases<'_>>,
    ) -> Result<(), LaneLifecycleError> {
        let publish_cursors = releases.is_some();
        let mut slot = self.geometry_publication.lock();
        if let Some(original) = slot.as_ref() {
            let startup_matches = match (&original.startup_owner, startup.as_deref()) {
                (None, None) => true,
                (Some(owner), Some(transition)) => {
                    Arc::ptr_eq(owner, &transition.original_owner_identity())
                }
                _ => false,
            };
            if original.publish_cursors != publish_cursors
                || !lane_config_entries_match(&original.previous, request.previous)
                || !lane_config_entries_match(&original.current, request.updated)
                || original.previous_incarnations != *request.previous_incarnations
                || original.current_incarnations != *request.updated_incarnations
                || original.previous_activation_heights != *request.previous_activation_heights
                || original.current_activation_heights != *request.updated_activation_heights
                || original.previous_lineage_root != request.previous_lineage_root
                || original.current_lineage_root != request.updated_lineage_root
                || original.transition_height != request.transition_height
                || original.replaced != *replaced
                || original.certified != *certified
                || !startup_matches
                || original
                    .raw
                    .as_ref()
                    .is_some_and(|raw| !raw.matches_request(request, replaced, certified))
            {
                return Err(LaneLifecycleError::Storage(
                    "another exact lane geometry operation is already retained".to_owned(),
                ));
            }
        } else {
            if startup.is_some() && !certified.is_empty() {
                return Err(LaneLifecycleError::Storage(
                    "startup replay cannot replace live certified drain ownership".to_owned(),
                ));
            }
            *slot = Some(LaneGeometryPublication {
                raw: None,
                tiered: None,
                certified: certified.clone(),
                startup_owner: startup
                    .as_deref()
                    .map(|transition| transition.original_owner_identity()),
                startup_attached: false,
                previous: request.previous.clone(),
                current: request.updated.clone(),
                previous_incarnations: request.previous_incarnations.clone(),
                current_incarnations: request.updated_incarnations.clone(),
                previous_activation_heights: request.previous_activation_heights.clone(),
                current_activation_heights: request.updated_activation_heights.clone(),
                previous_lineage_root: request.previous_lineage_root,
                current_lineage_root: request.updated_lineage_root,
                replaced: replaced.clone(),
                transition_height: request.transition_height,
                cursors_updated: false,
                publish_cursors,
            });
        }
        let original = slot.as_mut().ok_or_else(|| {
            LaneLifecycleError::Storage("lane geometry operation was not installed".to_owned())
        })?;
        if original.tiered.is_none() {
            let diff = lane_topology_diff(request.previous, request.updated, replaced);
            original.tiered = Some(
                self.tiered_backend
                    .lock()
                    .prepare_lane_geometry_attempt(
                        request.previous,
                        request.updated,
                        &diff.replacements,
                        &diff.relabelled,
                    )
                    .map_err(|error| {
                        LaneLifecycleError::Storage(format!("tiered preparation: {error:#}"))
                    })?,
            );
        }
        if original.raw.is_none() {
            let lease = self
                .kura
                .try_publication_lease()
                .map_err(geometry_lease_error)?;
            original.raw = Some(
                lease
                    .begin_raw_geometry_attempt(request, replaced, certified)
                    .map_err(LaneLifecycleError::GeometryStorage)?,
            );
        }
        let raw = original.raw.as_mut().ok_or_else(|| {
            LaneLifecycleError::Storage("missing retained raw geometry owner".to_owned())
        })?;
        if !original.startup_attached {
            if let Some(startup) = startup.as_deref_mut() {
                raw.attach_startup_transition(startup)
                    .map_err(LaneLifecycleError::GeometryStorage)?;
            }
            original.startup_attached = true;
        }
        // Catalog retry must not send an already chosen publication direction
        // through apply again. Its completed tiered plan is retained too.
        if !matches!(
            raw.phase(),
            crate::kura::RawGeometryPhase::PublishingCatalog
                | crate::kura::RawGeometryPhase::CatalogPublished
        ) {
            let lease = self
                .kura
                .try_publication_lease()
                .map_err(geometry_lease_error)?;
            raw.resume_under(&lease)
                .map_err(LaneLifecycleError::GeometryStorage)?;
        }
        original
            .tiered
            .as_mut()
            .ok_or_else(|| {
                LaneLifecycleError::Storage("missing retained tiered geometry owner".to_owned())
            })?
            .resume(&mut self.tiered_backend.lock())
            .map_err(|error| {
                LaneLifecycleError::Storage(format!("retained tiered geometry: {error:#}"))
            })?;
        if !original.cursors_updated {
            if let Some(releases) = releases.as_mut() {
                releases.shard_cursors.write().sync_mapping(request.updated);
                let should_persist = releases.hydrated.read().is_some();
                original.cursors_updated = true;
                if should_persist {
                    self.persist_lane_lifecycle_cursor_journal(releases);
                }
            } else {
                // Replay geometry has no authority to publish live cursor indexes.
                original.cursors_updated = true;
            }
        }
        Ok(())
    }

    fn finish_lane_geometry_publication(
        &self,
        current: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        configured_baseline: Option<Hash>,
        startup: Option<&mut crate::kura::StartupReplayGeometryTransition>,
    ) -> Result<(), LaneGeometryCatalogPublicationFailure> {
        let finish = || -> Result<(), LaneLifecycleError> {
            let mut slot = self.geometry_publication.lock();
            let original = slot.as_mut().ok_or_else(|| {
                LaneLifecycleError::Storage(
                    "catalog publication requires its original retained geometry operation"
                        .to_owned(),
                )
            })?;
            let raw = original.raw.as_mut().ok_or_else(|| {
                LaneLifecycleError::Storage(
                    "catalog publication has no original raw geometry owner".to_owned(),
                )
            })?;
            if !lane_config_entries_match(&original.current, current)
                || original.current_incarnations != *incarnations
                || original.current_activation_heights != *activation_heights
                || original.current_lineage_root != lineage_root
                || !raw.matches_startup_transition(startup.as_deref())
                || !original
                    .tiered
                    .as_ref()
                    .is_some_and(|tiered| tiered.is_applied())
                || !original.cursors_updated
            {
                return Err(LaneLifecycleError::Storage(
                    "catalog publication differs from its original completed geometry operation"
                        .to_owned(),
                ));
            }
            let lease = self
                .kura
                .try_publication_lease()
                .map_err(geometry_lease_error)?;
            if raw.phase() != crate::kura::RawGeometryPhase::CatalogPublished {
                raw.publish_catalog_under(&lease, configured_baseline)
                    .map_err(LaneLifecycleError::GeometryStorage)?;
            }
            if let Some(startup) = startup {
                raw.return_startup_namespace_receipts(startup)
                    .map_err(LaneLifecycleError::GeometryStorage)?;
            }
            *slot = None;
            Ok(())
        };
        finish().map_err(|error| LaneGeometryCatalogPublicationFailure {
            error,
            // The owner chooses and retains a publication direction, including
            // a rename whose directory sync failed. Never reconstruct rollback.
            rollback_safe: false,
        })
    }

    fn rollback_owned_lane_geometry(
        &self,
        previous: &LaneConfig,
        current: &LaneConfig,
        incarnations: &BTreeMap<LaneId, Hash>,
        activation_heights: &BTreeMap<LaneId, u64>,
        lineage_root: Hash,
        replaced: &BTreeSet<LaneId>,
        transition_height: u64,
        releases: &mut LaneLifecycleReleases<'_>,
    ) -> Result<(), LaneLifecycleError> {
        let mut slot = self.geometry_publication.lock();
        let original = slot.as_mut().ok_or_else(|| {
            LaneLifecycleError::Storage(
                "rollback requires the original retained geometry operation".to_owned(),
            )
        })?;
        if !lane_config_entries_match(&original.previous, previous)
            || !lane_config_entries_match(&original.current, current)
            || original.previous_incarnations != *incarnations
            || original.previous_activation_heights != *activation_heights
            || original.previous_lineage_root != lineage_root
            || original.replaced != *replaced
            || original.transition_height != transition_height
            || original.startup_owner.is_some()
        {
            return Err(LaneLifecycleError::Storage(
                "rollback differs from its original geometry owner".to_owned(),
            ));
        }
        if let Some(error) = original.raw.as_ref().and_then(|raw| raw.recovery_refusal()) {
            return Err(LaneLifecycleError::GeometryStorage(error));
        }
        // Finish a pending apply write before choosing reversal; the raw owner
        // refuses to switch direction while that exact write remains unsettled.
        if original.raw.as_ref().is_some_and(|raw| {
            raw.has_pending_journal_write()
                && raw.phase() != crate::kura::RawGeometryPhase::RollingBack
        }) {
            return Err(LaneLifecycleError::Storage(
                "original geometry write must finish before rollback".to_owned(),
            ));
        }
        if let Some(tiered) = original.tiered.as_mut() {
            tiered
                .rollback(&mut self.tiered_backend.lock())
                .map_err(|error| {
                    LaneLifecycleError::Storage(format!("retained tiered rollback: {error:#}"))
                })?;
        }
        if let Some(raw) = original.raw.as_mut() {
            let lease = self
                .kura
                .try_publication_lease()
                .map_err(geometry_lease_error)?;
            raw.rollback_under(&lease)
                .map_err(LaneLifecycleError::GeometryStorage)?;
        }
        if original.publish_cursors {
            releases.shard_cursors.write().sync_mapping(previous);
        }
        *slot = None;
        if releases.hydrated.read().is_some() {
            self.persist_lane_lifecycle_cursor_journal(releases);
        }
        Ok(())
    }
}

struct TieredStartupGeometry {
    configuration: iroha_config::parameters::actual::TieredState,
    lanes: LaneConfig,
    attempt: tiered::TieredGeometryAttempt,
}
impl TieredStartupGeometry {
    fn matches(
        &self,
        cfg: &iroha_config::parameters::actual::TieredState,
        lanes: &LaneConfig,
    ) -> bool {
        let original = &self.configuration;
        original.enabled == cfg.enabled
            && original.hot_retained_keys == cfg.hot_retained_keys
            && original.hot_retained_bytes.get() == cfg.hot_retained_bytes.get()
            && original.hot_retained_grace_snapshots == cfg.hot_retained_grace_snapshots
            && original.cold_store_root == cfg.cold_store_root
            && original.da_store_root == cfg.da_store_root
            && original.max_snapshots == cfg.max_snapshots
            && original.max_cold_bytes.get() == cfg.max_cold_bytes.get()
            && lane_config_entries_match(&self.lanes, lanes)
    }
}
