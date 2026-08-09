impl Kura {
    fn autonomous_lane_attempt_inventory_counts_locked(
        &self,
        entry: &LaneConfigEntry,
        target_lane_block_height: u64,
    ) -> Result<AutonomousLaneAttemptInventoryBudget> {
        self.autonomous_lane_attempt_inventory_counts_with_allowed_view_temp_locked(
            entry,
            target_lane_block_height,
            None,
        )
    }

    /// Inventory one active autonomous namespace while allowing the exact
    /// authenticated view-state named temporary currently being recovered.
    /// Every other recovery artifact remains fail-closed.
    fn autonomous_lane_attempt_inventory_counts_with_allowed_view_temp_locked(
        &self,
        entry: &LaneConfigEntry,
        target_lane_block_height: u64,
        allowed_view_temp: Option<&Path>,
    ) -> Result<AutonomousLaneAttemptInventoryBudget> {
        let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return Ok(AutonomousLaneAttemptInventoryBudget::empty());
            }
            Err(error) => return Err(Error::IO(error, directory)),
        };
        let mut related_files = 0_usize;
        let mut attempts_at_height = 0_usize;
        let mut related_bytes = 0_u64;
        let mut lifecycle_identities = BTreeSet::new();
        let mut terminal_outcome_identities = BTreeSet::new();
        let mut complete_terminal_outcome_identities = BTreeSet::new();
        for directory_entry in entries {
            let directory_entry =
                directory_entry.map_err(|error| Error::IO(error, directory.clone()))?;
            let path = directory_entry.path();
            let name = directory_entry.file_name().into_string().map_err(|_| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous attempt inventory contains a non-UTF-8 artifact",
                )
            })?;
            if name.starts_with(".kura-sidecar-") {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous attempt inventory requires startup cleanup of an atomic temporary artifact",
                ));
            }
            let bootstrap_quarantine = Self::validate_autonomous_publication_quarantine(
                &self.store_root,
                &path,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_MAX_BYTES,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
                "autonomous attempt bootstrap quarantine",
            )?;
            if Self::is_unresolved_autonomous_publication_temporary_name(
                &name,
                AUTONOMOUS_LIFECYCLE_BOOTSTRAP_ATOMIC_TEMP_PREFIX,
            ) {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous lifecycle bootstrap atomic temporary requires fail-closed recovery",
                ));
            }
            if !name.starts_with("autonomous_") && !bootstrap_quarantine {
                continue;
            }
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous attempt inventory contains a non-regular, linked, or symlinked artifact",
                ));
            }
            related_files = related_files.checked_add(1).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous attempt inventory count overflows",
                )
            })?;
            if related_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "autonomous attempt inventory exceeds its hard file-count limit",
                ));
            }
            related_bytes = related_bytes.checked_add(metadata.len()).ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous attempt inventory byte count overflows",
                )
            })?;
            if related_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(Self::invalid_lane_artifact_error(
                    directory,
                    "autonomous attempt inventory exceeds the shared sidecar aggregate byte budget",
                ));
            }
            if bootstrap_quarantine {
                continue;
            }
            if allowed_view_temp.is_some_and(|allowed| allowed == path.as_path()) {
                let identity =
                    Self::autonomous_lane_block_attempt_view_temp_coordinates(&name).ok_or_else(
                        || {
                            Self::invalid_lane_artifact_error(
                                path.clone(),
                                "allowed autonomous view-state recovery temp has a malformed name",
                            )
                        },
                    )?;
                lifecycle_identities.insert(identity);
                continue;
            }
            if let Some((lane_block_height, proposal_height)) =
                Self::autonomous_lane_block_attempt_coordinates(&name)
            {
                if lane_block_height == target_lane_block_height {
                    attempts_at_height = attempts_at_height.saturating_add(1);
                }
                lifecycle_identities.insert((lane_block_height, proposal_height));
                continue;
            }
            if let Some(identity) = Self::autonomous_two_height_coordinates(
                &name,
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
            ) {
                lifecycle_identities.insert(identity);
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_cursor_coordinates(&name) {
                lifecycle_identities.insert(identity);
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_bootstrap_coordinates(&name) {
                lifecycle_identities.insert(identity);
                continue;
            }
            if let Some(identity) = Self::autonomous_lifecycle_terminal_outcome_coordinates(&name) {
                terminal_outcome_identities.insert(identity);
                let maximum_bytes = u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES)?;
                if metadata.len() == 0 || metadata.len() > maximum_bytes {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous lifecycle terminal outcome has an invalid byte length",
                    ));
                }
                let bytes = self
                    .read_regular_sidecar_bytes(
                        &path,
                        &directory,
                        AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES,
                    )?
                    .ok_or_else(|| {
                        Self::invalid_lane_artifact_error(
                            path.clone(),
                            "autonomous lifecycle terminal outcome disappeared during inventory",
                        )
                    })?;
                let outcome = Self::decode_autonomous_lifecycle_terminal_outcome(&path, &bytes)?;
                if outcome.is_complete() {
                    complete_terminal_outcome_identities.insert(identity);
                }
                continue;
            }
            if Self::autonomous_one_height_coordinate(
                &name,
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
            )
            .is_some()
                || name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE
            {
                continue;
            }
            return Err(Self::invalid_lane_artifact_error(
                path,
                "unexpected or obsolete autonomous persistence artifact",
            ));
        }
        let missing_terminal_outcomes = lifecycle_identities
            .difference(&terminal_outcome_identities)
            .count();
        let conceptual_files = related_files
            .checked_add(missing_terminal_outcomes)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous attempt conceptual file count overflows",
                )
            })?;
        if conceptual_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous attempt inventory plus reserved terminal outcomes exceeds its hard file-count limit",
            ));
        }
        let reserved_terminal_bytes = u64::try_from(missing_terminal_outcomes)
            .ok()
            .and_then(|count| {
                count.checked_mul(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64)
            })
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous attempt reserved terminal-outcome byte count overflows",
                )
            })?;
        let conceptual_bytes = related_bytes
            .checked_add(reserved_terminal_bytes)
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    directory.clone(),
                    "autonomous attempt conceptual byte count overflows",
                )
            })?;
        if conceptual_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(Self::invalid_lane_artifact_error(
                directory,
                "autonomous attempt inventory plus reserved terminal outcomes exceeds the shared sidecar aggregate byte budget",
            ));
        }
        Ok(AutonomousLaneAttemptInventoryBudget {
            attempts_at_height,
            lifecycle_identities,
            terminal_outcome_identities,
            complete_terminal_outcome_identities,
            conceptual_files,
            conceptual_bytes,
        })
    }

    /// Return `(missing_terminal_count, incomplete_lifecycle_count)` across
    /// every active route while geometry and sidecar locks are held.
    ///
    /// The shared hard cardinality is also the startup recovery bound. This
    /// makes configured-capacity accounting independent of lane count and
    /// prevents an unbounded collection merely to protect deferred outcomes.
    fn autonomous_global_terminal_reservation_counts_locked(&self) -> Result<(usize, usize)> {
        self.autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(None)
    }

    fn autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(
        &self,
        allowed_view_temp: Option<&Path>,
    ) -> Result<(usize, usize)> {
        let entries = self
            .lane_storage_entries
            .lock()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let mut missing = 0_usize;
        let mut incomplete = 0_usize;
        for entry in entries {
            let inventory = self
                .autonomous_lane_attempt_inventory_counts_with_allowed_view_temp_locked(
                    &entry,
                    1,
                    allowed_view_temp,
                )?;
            missing = missing
                .checked_add(
                    inventory
                        .lifecycle_identities
                        .difference(&inventory.terminal_outcome_identities)
                        .count(),
                )
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "global autonomous terminal reservation count overflowed",
                    )
                })?;
            incomplete = incomplete
                .checked_add(
                    inventory
                        .lifecycle_identities
                        .difference(&inventory.complete_terminal_outcome_identities)
                        .count(),
                )
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        self.store_root.clone(),
                        "global autonomous incomplete lifecycle count overflowed",
                    )
                })?;
            if missing > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
                || incomplete > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
            {
                return Err(Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "global autonomous terminal reservation inventory exceeds its hard bound",
                ));
            }
        }
        Ok((missing, incomplete))
    }

    fn autonomous_global_terminal_outcome_reserved_bytes_locked(&self) -> Result<u64> {
        let (missing, incomplete) = self.autonomous_global_terminal_reservation_counts_locked()?;
        let terminal_max = u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES)?;
        u64::try_from(missing)
            .ok()
            .and_then(|count| count.checked_mul(terminal_max))
            .and_then(|stable| stable.checked_add(if incomplete == 0 { 0 } else { terminal_max }))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "global autonomous terminal reservation bytes overflowed",
                )
            })
    }

    fn autonomous_global_terminal_outcome_reserved_bytes(&self) -> Result<u64> {
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        self.autonomous_global_terminal_outcome_reserved_bytes_locked()
    }

    /// Preflight one autonomous atomic write against physical bytes, every
    /// admitted missing-terminal slot, the one serialized terminal-CAS
    /// transient, all pending canonical blocks, and all outstanding carrier
    /// receipt/frontier components. The caller snapshots
    /// `pending_canonical_bytes` while holding prune and canonical-chain locks,
    /// before acquiring geometry or sidecar locks.
    fn validate_configured_autonomous_mutation_disk_peak_locked(
        &self,
        pending_canonical_bytes: u64,
        additional_physical_peak_bytes: u64,
        creates_lifecycle_identity: bool,
        consumes_terminal_cas_transient: bool,
        path: &Path,
    ) -> Result<()> {
        self.validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked(
            pending_canonical_bytes,
            additional_physical_peak_bytes,
            creates_lifecycle_identity,
            consumes_terminal_cas_transient,
            path,
            None,
        )
    }

    fn validate_configured_autonomous_mutation_disk_peak_with_allowed_view_temp_locked(
        &self,
        pending_canonical_bytes: u64,
        additional_physical_peak_bytes: u64,
        creates_lifecycle_identity: bool,
        consumes_terminal_cas_transient: bool,
        path: &Path,
        allowed_view_temp: Option<&Path>,
    ) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let (missing, incomplete) = self
            .autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(
                allowed_view_temp,
            )?;
        let terminal_max = u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES)?;
        let resulting_missing = missing
            .checked_add(usize::from(creates_lifecycle_identity))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous mutation terminal reservation count overflowed",
                )
            })?;
        let resulting_incomplete = incomplete
            .checked_add(usize::from(creates_lifecycle_identity))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous mutation incomplete lifecycle count overflowed",
                )
            })?;
        if resulting_missing > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
            || resulting_incomplete > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous mutation exceeds the global terminal reservation bound",
            ));
        }
        let stable_terminal_reservations = u64::try_from(resulting_missing)
            .ok()
            .and_then(|count| count.checked_mul(terminal_max))
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous mutation global reservation bytes overflowed",
                )
            })?;
        let shared_terminal_transient = if resulting_incomplete == 0 {
            0
        } else {
            terminal_max
        };
        if consumes_terminal_cas_transient
            && additional_physical_peak_bytes > shared_terminal_transient
        {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous terminal mutation exceeds its globally reserved CAS transient",
            ));
        }
        let physical_and_transient = if consumes_terminal_cas_transient {
            shared_terminal_transient
        } else {
            additional_physical_peak_bytes
                .checked_add(shared_terminal_transient)
                .ok_or_else(|| {
                    Self::invalid_lane_artifact_error(
                        path.to_path_buf(),
                        "autonomous mutation transient accounting overflowed",
                    )
                })?
        };
        let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
        let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;
        let required = self
            .kura_disk_usage_bytes()?
            .checked_add(pending_canonical_bytes)
            .and_then(|bytes| bytes.checked_add(physical_and_transient))
            .and_then(|bytes| bytes.checked_add(stable_terminal_reservations))
            .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
            .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
            .and_then(|bytes| {
                bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
            })
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    path.to_path_buf(),
                    "autonomous mutation configured disk accounting overflowed",
                )
            })?;
        if required > self.max_disk_usage_bytes {
            return Err(Self::invalid_lane_artifact_error(
                path.to_path_buf(),
                "autonomous mutation would consume globally reserved terminal or carrier capacity",
            ));
        }
        Ok(())
    }

    /// Snapshot canonical blocks that are still represented only in memory.
    ///
    /// Callers hold `prune_lock` and `canonical_chain_lock`. This helper must
    /// run before either the lane-geometry or sidecar lock is acquired because
    /// the durable-index snapshot takes block-store metadata locks.
    fn pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&self) -> Result<u64> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let (persisted_count, unindexed_bytes) =
            self.persisted_count_and_unindexed_bytes()?;
        self.pending_block_bytes(persisted_count, unindexed_bytes)
    }

    fn validate_configured_kura_capacity_after_startup_recovery(&self) -> Result<()> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(());
        }
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let pending_canonical_bytes =
            self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
        let _geometry_guard = self.lane_geometry_lock.lock();
        let _sidecar_guard = self.sidecar_lock.lock();
        let used = self.kura_disk_usage_bytes()?;
        let terminal_reservations =
            self.autonomous_global_terminal_outcome_reserved_bytes_locked()?;
        let post_wsv_reservations = self.post_wsv_lane_artifact_budget_reserved_bytes()?;
        let certified_bundle_reservations = self.certified_bundle_capacity_reserved_bytes()?;
        let required = used
            .checked_add(pending_canonical_bytes)
            .and_then(|bytes| bytes.checked_add(terminal_reservations))
            .and_then(|bytes| bytes.checked_add(post_wsv_reservations))
            .and_then(|bytes| bytes.checked_add(certified_bundle_reservations))
            .and_then(|bytes| {
                bytes.checked_add(Self::canonical_prune_intent_maintenance_headroom_bytes())
            })
            .ok_or_else(|| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "startup configured Kura capacity accounting overflowed",
                )
            })?;
        if required > self.max_disk_usage_bytes {
            return Err(Error::StorageBudgetExceeded {
                limit: self.max_disk_usage_bytes,
                used,
                required,
            });
        }
        Ok(())
    }

    fn validate_autonomous_lifecycle_cursor_cas_budget(
        related_files: usize,
        related_bytes: u64,
        previous_len: u64,
        next_len: u64,
        replacing_existing: bool,
    ) -> std::result::Result<(), &'static str> {
        if replacing_existing != (previous_len != 0) {
            return Err("autonomous lifecycle cursor CAS replacement accounting is inconsistent");
        }
        let resulting_files = related_files
            .checked_add(usize::from(!replacing_existing))
            .ok_or("autonomous lifecycle cursor CAS file-count accounting overflowed")?;
        if resulting_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err(
                "autonomous lifecycle cursor CAS would exceed the hard namespace file-count limit",
            );
        }
        let resulting_bytes = related_bytes
            .checked_sub(previous_len)
            .and_then(|bytes| bytes.checked_add(next_len))
            .ok_or("autonomous lifecycle cursor CAS byte accounting overflowed")?;
        if resulting_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err(
                "autonomous lifecycle cursor CAS would exceed the shared sidecar aggregate byte budget",
            );
        }

        // Atomic synced replacement materializes one `.kura-sidecar-*` temp.
        // Bound that transient exposure separately from the resulting stable
        // namespace so replacing a cursor is not miscounted as a second stable
        // cursor, while a crash-staged temporary still has an explicit ceiling.
        let peak_files = related_files
            .checked_add(1)
            .ok_or("autonomous lifecycle cursor CAS temporary file-count accounting overflowed")?;
        if peak_files > MAX_AUTONOMOUS_LIFECYCLE_CURSOR_CAS_PEAK_FILES {
            return Err(
                "autonomous lifecycle cursor CAS would exceed its temporary file-count budget",
            );
        }
        let peak_bytes = related_bytes
            .checked_add(next_len)
            .ok_or("autonomous lifecycle cursor CAS temporary byte accounting overflowed")?;
        if peak_bytes > AUTONOMOUS_LIFECYCLE_CURSOR_CAS_PEAK_BYTES as u64 {
            return Err("autonomous lifecycle cursor CAS would exceed its temporary byte budget");
        }
        Ok(())
    }

    fn validate_autonomous_identity_artifact_cas_budget(
        inventory: &AutonomousLaneAttemptInventoryBudget,
        identity: (u64, u64),
        previous_len: u64,
        next_len: u64,
        replacing_existing: bool,
    ) -> std::result::Result<(), &'static str> {
        if replacing_existing != (previous_len != 0) {
            return Err("autonomous identity artifact replacement accounting is inconsistent");
        }
        let reserves_terminal = usize::from(
            !replacing_existing && inventory.needs_terminal_reservation_for_new_identity(identity),
        );
        let resulting_files = inventory
            .conceptual_files
            .checked_add(usize::from(!replacing_existing))
            .and_then(|files| files.checked_add(reserves_terminal))
            .ok_or("autonomous identity artifact conceptual file accounting overflowed")?;
        if resulting_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err("autonomous identity artifact exceeds the conceptual namespace bound");
        }
        let reserved_terminal_bytes = u64::try_from(reserves_terminal)
            .ok()
            .and_then(|count| {
                count.checked_mul(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES as u64)
            })
            .ok_or("autonomous identity artifact terminal reservation accounting overflowed")?;
        let resulting_bytes = inventory
            .conceptual_bytes
            .checked_sub(previous_len)
            .and_then(|bytes| bytes.checked_add(next_len))
            .and_then(|bytes| bytes.checked_add(reserved_terminal_bytes))
            .ok_or("autonomous identity artifact conceptual byte accounting overflowed")?;
        if resulting_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err("autonomous identity artifact exceeds the conceptual byte bound");
        }
        Ok(())
    }

    fn validate_autonomous_namespace_artifact_cas_budget(
        inventory: &AutonomousLaneAttemptInventoryBudget,
        previous_len: u64,
        next_len: u64,
        replacing_existing: bool,
    ) -> std::result::Result<(), &'static str> {
        if replacing_existing != (previous_len != 0) {
            return Err("autonomous namespace artifact replacement accounting is inconsistent");
        }
        let resulting_files = inventory
            .conceptual_files
            .checked_add(usize::from(!replacing_existing))
            .ok_or("autonomous namespace artifact conceptual file accounting overflowed")?;
        if resulting_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
            return Err("autonomous namespace artifact exceeds the conceptual namespace bound");
        }
        let resulting_bytes = inventory
            .conceptual_bytes
            .checked_sub(previous_len)
            .and_then(|bytes| bytes.checked_add(next_len))
            .ok_or("autonomous namespace artifact conceptual byte accounting overflowed")?;
        if resulting_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
            return Err("autonomous namespace artifact exceeds the conceptual byte bound");
        }
        Ok(())
    }
}
