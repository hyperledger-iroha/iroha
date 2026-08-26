macro_rules! kura_autonomous_reservation_inventory_methods {
    () => {
    /// Inspect one touched lane exactly once for batch reservation
    /// reconciliation.
    ///
    /// Unlike the writer-side count helper, this observer charges every
    /// directory entry against one batch-wide scan budget. That prevents a
    /// large number of Queue groups from repeatedly walking the same namespace
    /// and also bounds hostile unrelated filenames in the shared artifact
    /// directory. No temporary is removed or promoted here.
    #[allow(clippy::too_many_lines)]
    fn autonomous_reservation_lane_inventory_locked(
        &self,
        entry: &LaneConfigEntry,
        scanned_entries: &mut usize,
    ) -> std::result::Result<
        AutonomousReservationLaneInventory,
        AutonomousLaneReservationEvidenceError,
    > {
        let directory = Self::lane_artifact_dir(&entry.blocks_dir(&self.store_root));
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Default::default()),
            Err(error) => return Err(Error::IO(error, directory).into()),
        };
        let mut inventory = AutonomousReservationLaneInventory {
            directory_present: true,
            ..Default::default()
        };
        let mut autonomous_files = 0_usize;
        let mut autonomous_bytes = 0_u64;
        for directory_entry in entries {
            *scanned_entries = scanned_entries
                .checked_add(1)
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if *scanned_entries > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
            let directory_entry =
                directory_entry.map_err(|error| Error::IO(error, directory.clone()))?;
            let path = directory_entry.path();
            let name = directory_entry.file_name().into_string().map_err(|_| {
                Self::invalid_lane_artifact_error(
                    path.clone(),
                    "autonomous reservation inventory contains a non-UTF-8 artifact",
                )
            })?;
            if name.starts_with(".kura-sidecar-") {
                return Err(AutonomousLaneReservationEvidenceError::UnresolvedTemporary { path });
            }
            if !name.starts_with("autonomous_") {
                continue;
            }
            let metadata = secure_file_metadata::from_path(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            if metadata.file_type().is_symlink()
                || !metadata.file_type().is_file()
                || !Self::sidecar_is_single_link(&metadata)
            {
                return Err(Self::invalid_lane_artifact_error(
                    path,
                    "autonomous reservation inventory contains a non-regular, linked, or symlinked artifact",
                )
                .into());
            }
            if name.ends_with(".tmp") {
                return Err(AutonomousLaneReservationEvidenceError::UnresolvedTemporary { path });
            }
            autonomous_files = autonomous_files
                .checked_add(1)
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if autonomous_files > MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
            autonomous_bytes = autonomous_bytes
                .checked_add(metadata.len())
                .ok_or(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded)?;
            if autonomous_bytes > AUTONOMOUS_LANE_ARTIFACT_AGGREGATE_BYTES as u64 {
                return Err(AutonomousLaneReservationEvidenceError::AggregateBudgetExceeded);
            }
            if let Some(coordinates) = Self::autonomous_lane_block_attempt_coordinates(&name) {
                if inventory
                    .attempts
                    .insert(coordinates, metadata.len())
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous reservation inventory contains duplicate attempt coordinates",
                    )
                    .into());
                }
                continue;
            }
            if let Some(coordinates) = Self::autonomous_two_height_coordinates(
                &name,
                AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX,
            ) {
                if inventory
                    .view_states
                    .insert(coordinates, metadata.len())
                    .is_some()
                {
                    return Err(Self::invalid_lane_artifact_error(
                        path,
                        "autonomous reservation inventory contains duplicate view coordinates",
                    )
                    .into());
                }
                continue;
            }
            if let Some(height) = Self::autonomous_one_height_coordinate(
                &name,
                AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX,
            ) {
                inventory.lane_latest.insert(height, metadata.len());
                continue;
            }
            if name == AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE {
                inventory.route_latest_bytes = Some(metadata.len());
                continue;
            }
            // Lifecycle cursors, bootstrap records, and terminal outcomes
            // legitimately share this directory with reservation artifacts.
            // Their dedicated readers validate their contents; this bounded
            // inventory must accept only their exact canonical filenames.
            if Self::autonomous_lifecycle_cursor_coordinates(&name).is_some()
                || Self::autonomous_lifecycle_bootstrap_coordinates(&name).is_some()
                || Self::autonomous_lifecycle_terminal_outcome_coordinates(&name).is_some()
            {
                continue;
            }
            return Err(Self::invalid_lane_artifact_error(
                path,
                "unexpected or obsolete autonomous reservation persistence artifact",
            )
            .into());
        }
        Ok(inventory)
    }
    };
}
