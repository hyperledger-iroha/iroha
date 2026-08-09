struct LaneBlockExecutionInputPublicationPlan {
    namespace: BoundProgressNamespace,
    additional_physical_peak_bytes: u64,
}

impl Kura {
    /// Recover every active lane's execution-input append/prepend protocol to
    /// a durable fixed point before startup can replay historical seals.
    ///
    /// Live historical replay intentionally performs no recovery mutation
    /// before its whole-batch capacity gate, so this startup corridor is the
    /// sole owner of any append intent left by a crashed replay.
    fn recover_lane_block_execution_input_pairs_on_startup(&self) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        self.durable_mutation_authorized()?;
        let entries = {
            let _geometry_guard = self.lane_geometry_lock.lock();
            self.lane_storage_entries
                .lock()
                .values()
                .cloned()
                .collect::<Vec<_>>()
        };
        for expected_entry in entries {
            let _geometry_guard = self.lane_geometry_lock.lock();
            let entry = self.lane_storage_entry(expected_entry.lane_id)?;
            if entry != expected_entry {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "lane geometry changed during execution-input startup recovery",
                ));
            }
            let (data_path, index_path) =
                Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
            let _sidecar_guard = self.sidecar_lock.lock();
            if !self.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                LaneBlockExecutionInputArtifact::FORMAT_LABEL,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    data_path,
                    "lane execution-input pair failed startup recovery",
                ));
            }
        }
        Ok(())
    }

    /// Persist verified recovered payload input for a certified standalone lane block.
    ///
    /// # Errors
    ///
    /// Returns an error when the recovered input is internally inconsistent, the
    /// lane has no configured storage segment, or the sidecar write fails.
    pub fn persist_lane_block_execution_input(
        &self,
        recovered: &RecoveredLaneBlockPayload,
    ) -> Result<()> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        self.persist_lane_block_execution_input_under_prune_and_canonical_guards(
            recovered,
            pending_canonical_bytes,
        )
    }

    /// Persist one verified execution input while the caller continuously
    /// holds `prune_lock`. Historical recovery uses this seam after its whole
    /// batch has acquired the outer prune fence.
    fn persist_lane_block_execution_input_under_prune_and_canonical_guards(
        &self,
        recovered: &RecoveredLaneBlockPayload,
        pending_canonical_bytes: u64,
    ) -> Result<()> {
        self.ensure_prune_recovery_not_required()?;
        let verified = self
            .recover_lane_block_execution_input_source(
                &recovered.proposal,
                recovered.autonomous_chain_id_hash,
                recovered.autonomous_epoch,
                recovered.autonomous_payload_hash,
                false,
            )
            .map_err(|availability| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    format!("lane execution input recovery failed: {availability:?}"),
                )
            })?;
        if &verified != recovered {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "lane execution input does not match canonical recovered payload",
            ));
        }
        let artifact = LaneBlockExecutionInputArtifact::new(verified);
        let execution_input_authorization = match (
            artifact.autonomous_chain_id_hash,
            artifact.autonomous_epoch,
            artifact.autonomous_payload_hash,
        ) {
            (Some(chain_id_hash), Some(epoch), Some(payload_hash)) => {
                let descriptor = &artifact.proposal.descriptor;
                let autonomous = self
                    .read_autonomous_lane_block_artifact_with_recovery_policy(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                        chain_id_hash,
                        epoch,
                        false,
                    )
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            self.store_root.clone(),
                            "autonomous execution-input authorization lacks its exact durable payload",
                        )
                    })?;
                if autonomous.executable_payload.payload_hash != payload_hash {
                    return Err(Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "autonomous execution-input authorization recovered another payload",
                    ));
                }
                Some(
                    self.authorize_autonomous_execution_input_persistence(
                        &autonomous.executable_payload,
                        &artifact,
                    )
                    .map_err(|message| {
                        Self::invalid_lane_artifact_error(self.store_root.clone(), message)
                    })?,
                )
            }
            (None, None, None) => None,
            _ => {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "lane execution input has a partial autonomous context",
                ));
            }
        };
        self.write_lane_block_execution_input_artifact(
            &artifact,
            execution_input_authorization,
            pending_canonical_bytes,
        )
    }

    fn write_lane_block_execution_input_artifact(
        &self,
        artifact: &LaneBlockExecutionInputArtifact,
        mut execution_input_authorization: Option<
            AutonomousLaneExecutionInputPersistenceAuthorization,
        >,
        pending_canonical_bytes: u64,
    ) -> Result<()> {
        self.durable_mutation_authorized()?;
        Self::validate_lane_block_execution_input_artifact(artifact).map_err(|message| {
            Self::invalid_lane_artifact_error(self.store_root.clone(), message.to_string())
        })?;
        let autonomous_input = matches!(
            (
                artifact.autonomous_chain_id_hash,
                artifact.autonomous_epoch,
                artifact.autonomous_payload_hash,
            ),
            (Some(_), Some(_), Some(_))
        );
        match (autonomous_input, execution_input_authorization.as_ref()) {
            (true, Some(authorization)) if authorization.matches_input(artifact) => {}
            (false, None) => {}
            (true, Some(_)) => {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous execution-input authorization does not match the artifact",
                ));
            }
            (true, None) => {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "autonomous execution-input persistence lacks its authorization",
                ));
            }
            (false, Some(_)) => {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "ordinary execution-input persistence received autonomous authorization",
                ));
            }
        }
        let descriptor = &artifact.proposal.descriptor;
        let lane_id = descriptor.lane_id;
        let lane_block_height = descriptor.lane_block_height;
        if lane_block_height == 0 {
            return Err(Self::invalid_lane_artifact_error(
                self.store_root.clone(),
                "lane execution input height must be non-zero",
            ));
        }

        let observed_existing =
            self.read_lane_block_execution_input_for_write_observation(lane_id, lane_block_height);
        let observed_existing_is_canonical = observed_existing.as_ref().is_some_and(|existing| {
            self.lane_block_execution_input_matches_canonical_payload(existing, false)
        });

        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        self.require_active_lane_artifact(&entry, descriptor)?;
        let (data_path, index_path) =
            Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
        if data_path.parent().is_none() {
            return Err(Self::invalid_lane_artifact_error(
                data_path,
                "lane execution input path has no parent directory",
            ));
        }
        let _guard = self.sidecar_lock.lock();
        let observation_namespace = self.open_bound_progress_namespace(&data_path, &index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &observation_namespace,
            &data_path,
            &index_path,
            "lane block execution input",
        )?;
        if let Some(existing) = Self::read_indexed_sidecar_from_paths_with_recovery(
            lane_block_height,
            &data_path,
            &index_path,
            norito::decode_canonical::<LaneBlockExecutionInputArtifact>,
            "lane block execution input",
            false,
        ) {
            if existing == *artifact {
                if !Self::sync_indexed_sidecar_barriers(
                    &data_path,
                    &index_path,
                    "lane block execution input",
                ) {
                    return Err(Error::IO(
                        std::io::Error::other(
                            "failed to make existing lane block execution input durable",
                        ),
                        data_path,
                    ));
                }
                return Ok(());
            }
            if self
                .require_active_lane_artifact(&entry, &existing.proposal.descriptor)
                .is_ok()
            {
                if observed_existing.as_ref() != Some(&existing) {
                    return Err(Self::invalid_lane_artifact_error(
                        data_path,
                        "active lane execution input changed while canonicality was validated",
                    ));
                }
                if observed_existing_is_canonical {
                    return Err(Self::invalid_lane_artifact_error(
                        data_path,
                        format!(
                            "canonical lane execution input already exists for lane {} height {} with a different payload",
                            lane_id.as_u32(),
                            lane_block_height
                        ),
                    ));
                }
            }
            iroha_logger::warn!(
                lane = %lane_id.as_u32(),
                lane_block_height,
                "overwriting stale lane execution input sidecar with recovered canonical payload"
            );
        }

        let payload = artifact.encode_framed()?;
        let payload_len = u64::try_from(payload.len())?;
        let publication = self.preflight_lane_block_execution_input_publication_locked(
            pending_canonical_bytes,
            &data_path,
            &index_path,
            lane_block_height,
            payload_len,
        )?;
        debug_assert!(publication.additional_physical_peak_bytes >= payload_len);
        if autonomous_input {
            let projection: ProductionInFlightFirstReleaseTransitionProjection =
                execution_input_authorization
                    .take()
                    .and_then(|authorization| authorization.consume_for_persistence(artifact))
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            data_path.clone(),
                            "autonomous execution-input authorization changed before persistence",
                        )
                    })?;
            if projection.action != IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "autonomous execution-input persistence received the wrong transition",
                ));
            }
            let checked = check_production_in_flight_first_release_transition(projection)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        data_path.clone(),
                        "autonomous execution input failed the composed first-release transition gate",
                    )
                })?;
            if checked.into_projection() != projection {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.clone(),
                    "checked autonomous execution-input projection changed before persistence",
                ));
            }
        }
        let before_bytes = match Self::sidecar_tracked_bytes(&data_path, &index_path, None) {
            Ok(bytes) => Some(bytes),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    lane = %lane_id.as_u32(),
                    lane_block_height,
                    "failed to measure lane execution input bytes before write"
                );
                None
            }
        };
        let accounting_mutation = self.begin_total_disk_usage_mutation();
        let wrote = Self::append_indexed_progress_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block execution input",
            None,
            SidecarIndexOrigin::FirstWrite,
            &publication.namespace,
        );
        if !wrote {
            return Err(Error::IO(
                std::io::Error::other("failed to persist lane block execution input"),
                data_path,
            ));
        }
        let mut accounting_complete = before_bytes.is_some();
        if let Some(before_bytes) = before_bytes {
            match Self::sidecar_tracked_bytes(&data_path, &index_path, None) {
                Ok(after_bytes) => self.update_disk_usage_delta(before_bytes, after_bytes),
                Err(err) => {
                    accounting_complete = false;
                    iroha_logger::warn!(
                        ?err,
                        lane = %lane_id.as_u32(),
                        lane_block_height,
                        "failed to measure lane execution input bytes after write"
                    );
                }
            }
        }
        if accounting_complete {
            accounting_mutation.finish();
        }
        self.note_committed_lane_status_change();
        Ok(())
    }

    /// Capture the exact decodable execution-input bytes for the writer's
    /// optimistic concurrency check, including structurally stale payloads.
    ///
    /// Canonical validation happens separately. Retaining a malformed but
    /// stable observation lets the writer replace it, while a sidecar that
    /// appears or changes after this observation still fails closed.
    fn read_lane_block_execution_input_for_write_observation(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Option<LaneBlockExecutionInputArtifact> {
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id).ok()?;
        let (data_path, index_path) =
            Self::lane_block_execution_input_paths_for_entry(&entry, &self.store_root);
        let _guard = self.sidecar_lock.lock();
        if self.prune_recovery_is_required() {
            return None;
        }
        Self::read_indexed_sidecar_from_paths_with_recovery(
            lane_block_height,
            &data_path,
            &index_path,
            norito::decode_canonical::<LaneBlockExecutionInputArtifact>,
            "lane block execution input",
            false,
        )
    }
}

impl Kura {
    /// Bind an execution-input pair at a read-only fixed point and reserve the
    /// complete physical publication peak before the append journal, data, or
    /// index can change.
    fn preflight_lane_block_execution_input_publication_locked(
        &self,
        pending_canonical_bytes: u64,
        data_path: &Path,
        index_path: &Path,
        lane_block_height: u64,
        payload_len: u64,
    ) -> Result<LaneBlockExecutionInputPublicationPlan> {
        let namespace = self.open_bound_progress_namespace(data_path, index_path)?;
        self.ensure_bound_progress_pair_has_no_recovery_artifacts_locked(
            &namespace,
            data_path,
            index_path,
            "lane block execution input",
        )?;
        let parent = data_path.parent().ok_or_else(|| {
            Self::invalid_lane_artifact_error(
                data_path.to_path_buf(),
                "lane block execution input path has no parent directory",
            )
        })?;
        let data_metadata = self.regular_sidecar_metadata(data_path, parent)?;
        let index_metadata = self.regular_sidecar_metadata(index_path, parent)?;
        let layout = match (data_metadata, index_metadata) {
            (None, None) => SidecarIndexLayout::legacy(0),
            (Some(_), Some(index_metadata)) => {
                let mut index = Self::open_direct_sidecar_file_in_namespace(
                    index_path,
                    false,
                    false,
                    Some(&namespace),
                )
                .map_err(|error| Error::IO(error, index_path.to_path_buf()))?;
                let layout = SidecarIndexLayout::read_from(&mut index, index_metadata.file.len())
                    .map_err(|reason| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        format!("lane block execution input index is malformed: {reason}"),
                    )
                })?;
                if layout.aligned_len != index_metadata.file.len() {
                    return Err(Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        "lane block execution input index has trailing or partial bytes",
                    ));
                }
                layout
            }
            _ => {
                return Err(Self::invalid_lane_artifact_error(
                    data_path.to_path_buf(),
                    "lane block execution input data/index pair is only partially present",
                ));
            }
        };

        let append_intent_max = u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)?;
        let transient_bytes = if layout.is_based() && lane_block_height < layout.base_height {
            let prepend = layout
                .base_height
                .checked_sub(lane_block_height)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        "lane block execution input prepend accounting underflows",
                    )
                })?;
            if prepend > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                return Err(Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    "lane block execution input prepend exceeds the bounded index window",
                ));
            }
            let projected_entries = layout.entry_count.checked_add(prepend).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    index_path.to_path_buf(),
                    "lane block execution input prepend entry count overflows",
                )
            })?;
            let projected_index_len = projected_entries
                .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .and_then(|bytes| {
                    bytes.checked_add(if lane_block_height > 1 {
                        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64
                    } else {
                        0
                    })
                })
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        index_path.to_path_buf(),
                        "lane block execution input prepend temp accounting overflows",
                    )
                })?;
            // The complete replacement index exists beside the old index until
            // promotion; the appended payload exists beside both.
            payload_len
                .checked_add(projected_index_len)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        data_path.to_path_buf(),
                        "lane block execution input prepend peak accounting overflows",
                    )
                })?
        } else {
            payload_len
                .checked_add(Self::maximum_index_growth_for_unresolved_sidecar_write(
                    lane_block_height,
                ))
                .and_then(|bytes| bytes.checked_add(append_intent_max))
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        data_path.to_path_buf(),
                        "lane block execution input append peak accounting overflows",
                    )
                })?
        };
        self.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            transient_bytes,
            false,
            false,
            data_path,
        )?;
        Ok(LaneBlockExecutionInputPublicationPlan {
            namespace,
            additional_physical_peak_bytes: transient_bytes,
        })
    }

    fn autonomous_view_state_inventory_with_allowed_temp_locked(
        &self,
        entry: &LaneConfigEntry,
        lane_block_height: u64,
        allowed_temp: Option<&Path>,
    ) -> Result<AutonomousLaneAttemptInventoryBudget> {
        self.autonomous_lane_attempt_inventory_counts_with_allowed_view_temp_locked(
            entry,
            lane_block_height,
            allowed_temp,
        )
    }

    fn validate_autonomous_view_state_namespace_peak(
        inventory: &AutonomousLaneAttemptInventoryBudget,
        additional_files: usize,
        additional_bytes: u64,
    ) -> std::result::Result<(), &'static str> {
        if inventory
            .conceptual_files
            .checked_add(additional_files)
            .is_none_or(|files| files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES)
        {
            return Err("autonomous view-state mutation exceeds the namespace file-count peak");
        }
        if inventory
            .conceptual_bytes
            .checked_add(additional_bytes)
            .is_none_or(|bytes| bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64)
        {
            return Err("autonomous view-state mutation exceeds the namespace byte peak");
        }
        Ok(())
    }

    /// Preflight promotion of a valid named view-state temp. The named temp is
    /// retained until the atomic replacement has synced, so both files and the
    /// additional `.kura-sidecar-*` temp coexist at the physical peak.
    fn preflight_autonomous_view_state_recovery_promotion_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        lane_block_height: u64,
        path: &Path,
        temp_path: &Path,
        replacement_len: u64,
    ) -> Result<()> {
        let inventory = self.autonomous_view_state_inventory_with_allowed_temp_locked(
            entry,
            lane_block_height,
            Some(temp_path),
        )?;
        Self::validate_autonomous_view_state_namespace_peak(&inventory, 1, replacement_len)
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        self.validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked(
            pending_canonical_bytes,
            replacement_len,
            false,
            false,
            path,
            Some(temp_path),
        )
    }

    /// Preflight an ordinary view-state CAS. A present named temp is removed
    /// first, so its exact bytes are credited before the atomic replacement
    /// temp is materialized. No file is removed until every namespace and
    /// configured-capacity check succeeds.
    #[allow(clippy::too_many_arguments)]
    fn preflight_autonomous_view_state_write_locked(
        &self,
        pending_canonical_bytes: u64,
        entry: &LaneConfigEntry,
        identity: (u64, u64),
        path: &Path,
        temp_path: &Path,
        main_len: u64,
        temp_present: bool,
        temp_len: u64,
        replacement_len: u64,
        replacing_existing: bool,
    ) -> Result<()> {
        let allowed_temp = temp_present.then_some(temp_path);
        let mut inventory = self.autonomous_view_state_inventory_with_allowed_temp_locked(
            entry,
            identity.0,
            allowed_temp,
        )?;
        if temp_present {
            inventory.conceptual_files =
                inventory.conceptual_files.checked_sub(1).ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        temp_path.to_path_buf(),
                        "autonomous view-state temp file accounting underflows",
                    )
                })?;
            inventory.conceptual_bytes = inventory
                .conceptual_bytes
                .checked_sub(temp_len)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        temp_path.to_path_buf(),
                        "autonomous view-state temp byte accounting underflows",
                    )
                })?;
        }
        Self::validate_autonomous_identity_artifact_cas_budget(
            &inventory,
            identity,
            main_len,
            replacement_len,
            replacing_existing,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        Self::validate_autonomous_view_state_namespace_peak(&inventory, 1, replacement_len)
            .map_err(|message| Self::invalid_lane_artifact_error(path.to_path_buf(), message))?;
        let additional_physical_peak = replacement_len.saturating_sub(temp_len);
        let creates_lifecycle_identity =
            !replacing_existing && inventory.needs_terminal_reservation_for_new_identity(identity);
        self.validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked(
            pending_canonical_bytes,
            additional_physical_peak,
            creates_lifecycle_identity,
            false,
            path,
            allowed_temp,
        )
    }
}
