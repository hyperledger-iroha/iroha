use resident_inventory::ResidentOwner;

impl ResidentOwner for BlockData {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentCanonical;

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        // A deferred logical height is not the number of materialized hash slots.
        resident_inventory::lengths([match self {
            Self::Dense(entries) => entries.len(),
            Self::Deferred { entries, .. } => entries.len(),
        }])
    }

    fn resident_complete(&self) -> bool {
        matches!(self, Self::Dense(_))
    }
}

impl ResidentOwner for BlockHeightIndex {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentCanonical;

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([self.len()])
    }

    fn resident_complete(&self) -> bool {
        true
    }
}

impl ResidentOwner for TransactionEntrypointIndex {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentTransaction;

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        let markers = resident_inventory::lengths([
            self.indexed_heights.len(),
            self.incomplete_merge_heights.len(),
            self.incomplete_kaigi_signal_heights.len(),
            self.inventories_by_height.len(),
        ])?;
        self.nested_associations
            .get()?
            .checked_add(markers)
            .ok_or(resource_inventory::Unavailable::Arithmetic)
    }

    fn resident_complete(&self) -> bool {
        self.complete
            && self.incomplete_merge_heights.is_empty()
            && self.incomplete_kaigi_signal_heights.is_empty()
    }
}

impl ResidentOwner for MergeLedgerLog {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentMerge;

    fn failure_invalidation_mask() -> u32 {
        // A failed append can leave a tail repaired by later reads/preflights.
        // Their resident guard keeps snapshots busy through the repair and
        // invalidates the physical baseline before releasing the same log lock.
        Self::FAMILY.mask() | physical_resource_mask()
    }

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        // The bounded payload cache is not substituted for the full frame maps.
        resident_inventory::lengths([
            self.frames_by_hash.len(),
            self.frames_by_epoch.len(),
            self.in_memory_entries.len(),
            self.latest_execution_entries.len(),
        ])
    }

    fn resident_complete(&self) -> bool {
        !self.history_deferred
            && self.resident_inventory_valid
            && self.append_recovery_offset.is_none()
            && self.frames_by_hash.len() == self.total_entries
            && self.frames_by_epoch.len() == self.total_entries
            && (self.file.is_some() || self.in_memory_entries.len() == self.total_entries)
    }
}

impl ResidentOwner for MergeCarrierIndex {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentCarrier;

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([self.by_height.len(), self.by_entry.len()])
    }

    fn resident_complete(&self) -> bool {
        self.initialized && self.by_height.len() == self.by_entry.len()
    }
}

impl Kura {
    /// Register only fully reconstructed resident owners, without doing reconstruction.
    ///
    /// Reads existing owner locks independently in their usual direction. The shared
    /// generation rejects a mutation crossing these fixed-size observations. This is
    /// an explicit preparation/reconciliation operation, never the telemetry scrape.
    /// Other physical resource families remain unregistered until their own exact audits finish.
    pub(crate) fn reconcile_resident_resource_inventory(
        &self,
    ) -> std::result::Result<(), resource_inventory::Unavailable> {
        use resource_inventory::{Family, Unavailable, Usage};
        let generation = self.resource_inventory.reconciliation_generation()?;
        if self.auxiliary_history_deferred
            || self.provisional_snapshot_bootstrap_pending()
            || !self
                .post_wsv_resident_recovery_complete
                .load(Ordering::Acquire)
            || !self
                .certified_resident_recovery_complete
                .load(Ordering::Acquire)
        {
            return Err(Unavailable::InvalidInventory);
        }
        fn exact(owner: &impl ResidentOwner) -> std::result::Result<u64, Unavailable> {
            if !owner.resident_complete() {
                return Err(Unavailable::InvalidInventory);
            }
            owner.resident_associations()
        }
        let materialized_hashes = {
            let owner = self.block_data.lock();
            exact(&owner)?
        };
        let reverse_hashes = {
            let owner = self.block_height_index.lock();
            exact(&owner)?
        };
        let canonical = materialized_hashes
            .checked_add(reverse_hashes)
            .ok_or(Unavailable::Arithmetic)?;
        let transactions = exact(&self.transaction_entrypoint_index.lock())?;
        let merge = exact(&self.merge_log.lock())?;
        let carriers = exact(&self.merge_carrier_index.lock())?;
        let replicas = exact(&self.replica_registry.lock())?;
        let finality_cache = exact(&self.v2_finality_verification_cache.lock())?;
        let startup_allocations = exact(&self.startup_inventory_resident.lock())?;
        let verification = finality_cache
            .checked_add(startup_allocations)
            .ok_or(Unavailable::Arithmetic)?;
        let lane_entries = exact(&self.lane_storage_entries.lock())?;
        let frontier_pairs = exact(&self.certified_frontier_pair_durability.lock())?;
        let frontier_artifacts = exact(&self.certified_frontier_artifact_validation.lock())?;
        let post_wsv = exact(&self.post_wsv_lane_artifact_budget_reservations.lock())?;
        let certified = exact(&self.certified_bundle_capacity_reservations.lock())?;
        let frontier = [
            lane_entries,
            frontier_pairs,
            frontier_artifacts,
            post_wsv,
            certified,
        ]
        .into_iter()
        .try_fold(0_u64, |sum, value| {
            sum.checked_add(value).ok_or(Unavailable::Arithmetic)
        })?;
        let pipeline = exact(&self.pipeline_sidecar_queue.lock())?;
        let proofs = exact(&self.fastpq_proof_queue.lock())?;
        let queues = pipeline
            .checked_add(proofs)
            .ok_or(Unavailable::Arithmetic)?;
        let usage = |resident_associations| Usage {
            resident_associations,
            ..Usage::default()
        };
        self.resource_inventory.initialize(
            generation,
            &[
                (Family::ResidentCanonical, usage(canonical)),
                (Family::ResidentTransaction, usage(transactions)),
                (Family::ResidentMerge, usage(merge)),
                (Family::ResidentCarrier, usage(carriers)),
                (Family::ResidentReplica, usage(replicas)),
                (Family::ResidentVerification, usage(verification)),
                (Family::ResidentFrontier, usage(frontier)),
                (Family::ResidentQueue, usage(queues)),
            ],
        )
    }
}
