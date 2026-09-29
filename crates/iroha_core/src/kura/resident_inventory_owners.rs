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
            self.incomplete_heights.len(),
            self.inventories_by_height.len(),
        ])?;
        self.nested_associations
            .get()?
            .checked_add(markers)
            .ok_or(resource_inventory::Unavailable::Arithmetic)
    }

    fn resident_complete(&self) -> bool {
        self.complete && self.incomplete_heights.is_empty()
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
                .native_amx_resident_recovery_complete
                .load(Ordering::Acquire)
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
        let certified_pairs = exact(&self.certified_pair_durability.lock())?;
        let receipt_namespaces = exact(&self.lane_receipt_namespace_durability.lock())?;
        let frontier_artifacts = exact(&self.certified_frontier_artifact_validation.lock())?;
        let native = exact(&self.native_amx_publication_capacity_reservations.lock())?;
        let post_wsv = exact(&self.post_wsv_lane_artifact_budget_reservations.lock())?;
        let certified = exact(&self.certified_bundle_capacity_reservations.lock())?;
        let frontier = [
            lane_entries,
            certified_pairs,
            receipt_namespaces,
            frontier_artifacts,
            native,
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
