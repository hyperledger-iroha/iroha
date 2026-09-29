

impl Kura {
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


    /// Snapshot canonical blocks that are still represented only in memory.
    ///
    /// Callers hold `prune_lock` and `canonical_chain_lock`. This helper must
    /// run before either the lane-geometry or sidecar lock is acquired because
    /// the durable-index snapshot takes block-store metadata locks and a cold
    /// pending-block scan resolves merge references through the sidecar lock.
    /// Nonblocking publication preparation retains its snapshot in the lease.
    fn pending_canonical_capacity_bytes_under_prune_and_canonical_guards(&self) -> Result<u64> {
        if self.max_disk_usage_bytes == 0 || self.store_root.as_os_str().is_empty() {
            return Ok(0);
        }
        let (persisted_count, unindexed_bytes) = self.persisted_count_and_unindexed_bytes()?;
        self.pending_block_bytes(persisted_count, unindexed_bytes)
    }
    fn validate_and_publish_configured_kura_capacity_after_startup_recovery(
        &self,
        capacity_recovery_complete: bool,
    ) -> Result<()> {
        let publication = (|| -> Result<()> {
            let _prune_guard = self.prune_lock.lock();
            self.ensure_prune_recovery_not_required()?;
            let _canonical_chain_guard = self.canonical_chain_lock.lock();
            let pending_canonical_bytes = if capacity_recovery_complete {
                self.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?
            } else {
                0
            };
            let _geometry_guard = self.lane_geometry_lock.lock();
            let _sidecar_guard = self.sidecar_lock.lock();
            let (used, total) = self.kura_disk_usage_bytes_with_total()?;
            if capacity_recovery_complete
                && self.max_disk_usage_bytes != 0
                && !self.store_root.as_os_str().is_empty()
            {
                let terminal_reservations =
                    self.autonomous_global_terminal_outcome_reserved_bytes_locked()?;
                let lane_publication_reservations =
                    self.all_publication_budget_reserved_bytes()?;
                let certified_bundle_reservations =
                    self.certified_bundle_capacity_reserved_bytes()?;
                let required = used
                    .checked_add(pending_canonical_bytes)
                    .and_then(|bytes| bytes.checked_add(terminal_reservations))
                    .and_then(|bytes| bytes.checked_add(lane_publication_reservations))
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
            }
            self.disk_usage.store(used, Ordering::Relaxed);
            self.disk_usage_initialized.store(true, Ordering::Relaxed);
            self.disk_usage_total.store(total, Ordering::Relaxed);
            self.disk_usage_total_initialized
                .store(true, Ordering::Relaxed);
            self.disk_usage_total_last_refresh
                .store(Self::now_unix_secs(), Ordering::Relaxed);
            Ok(())
        })();
        match publication {
            Ok(()) => Ok(()),
            Err(err) => {
                self.disk_usage_initialized.store(false, Ordering::Relaxed);
                self.disk_usage_total_initialized
                    .store(false, Ordering::Relaxed);
                self.invalidate_durable_budget_snapshot();
                Err(err)
            }
        }
    }
}
