// Receipt reads retain directory durability only. Regular-file descriptors
// never escape the read, so custody cannot keep an unlinked payload alive.
// LaneStorageIdentity constructs root/blocks/instances/<id>/lane_artifacts.
const LANE_RECEIPT_NAMESPACE_DEPTH: usize = 5;
const LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY: usize =
    CERTIFIED_ARTIFACT_ATTESTATION_CACHE_CAPACITY;
const LANE_RECEIPT_NAMESPACE_SLOTS: usize =
    LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY / LANE_RECEIPT_NAMESPACE_DEPTH;
// Includes actual retained Vec and path capacities; fixed slot storage is
// inline and bounded separately by size_of::<LaneReceiptNamespaceDurability>().
const LANE_RECEIPT_DURABILITY_HEAP_BYTES: usize = 256 * 1024;

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_RECEIPT_NAMESPACE_DIRECTORY_SYNC: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[derive(Debug)]
struct DurableReceiptNamespace {
    namespace: BoundProgressNamespace,
    mutation_epoch: u64,
    heap_bytes: usize,
}

#[derive(Debug)]
struct LaneReceiptNamespaceDurability {
    slots: [Option<DurableReceiptNamespace>; LANE_RECEIPT_NAMESPACE_SLOTS],
    next: usize,
}

impl Default for LaneReceiptNamespaceDurability {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
            next: 0,
        }
    }
}

impl LaneReceiptNamespaceDurability {
    fn directory_count(&self) -> usize {
        self.slots
            .iter()
            .flatten()
            .map(|entry| entry.namespace.directories.len())
            .sum()
    }

    fn heap_bytes(&self) -> usize {
        self.slots
            .iter()
            .flatten()
            .map(|entry| entry.heap_bytes)
            .sum()
    }

    fn namespace_heap_bytes(namespace: &BoundProgressNamespace) -> Option<usize> {
        let mut bytes = namespace
            .directories
            .capacity()
            .checked_mul(std::mem::size_of::<BoundProgressDirectory>())?;
        bytes = bytes
            .checked_add(namespace.data_path.capacity())?
            .checked_add(namespace.index_path.capacity())?;
        for directory in &namespace.directories {
            bytes = bytes
                .checked_add(directory.expected_path.capacity())?
                .checked_add(directory.canonical_path.capacity())?
                .checked_add(
                    directory
                        .entry_name
                        .as_ref()
                        .map_or(0, |name| name.capacity()),
                )?;
        }
        Some(bytes)
    }

    fn retain(&mut self, namespace: BoundProgressNamespace, mutation_epoch: u64) {
        // Depth derives from the sole current lane-instance constructor. This
        // limits both held FDs and unused capacity in the retained Vec.
        if namespace.directories.len() != LANE_RECEIPT_NAMESPACE_DEPTH
            || namespace.directories.capacity() > LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY
        {
            return;
        }
        let Some(heap_bytes) = Self::namespace_heap_bytes(&namespace)
            .filter(|bytes| *bytes <= LANE_RECEIPT_DURABILITY_HEAP_BYTES)
        else {
            return;
        };
        for slot in &mut self.slots {
            if slot.as_ref().is_some_and(|entry| {
                entry.mutation_epoch != mutation_epoch
                    || entry.namespace.data_path == namespace.data_path
            }) {
                *slot = None;
            }
        }
        self.slots[self.next] = None;
        while self.heap_bytes() > LANE_RECEIPT_DURABILITY_HEAP_BYTES - heap_bytes {
            self.next = (self.next + 1) % self.slots.len();
            self.slots[self.next] = None;
        }
        self.slots[self.next] = Some(DurableReceiptNamespace {
            namespace,
            mutation_epoch,
            heap_bytes,
        });
        self.next = (self.next + 1) % self.slots.len();
    }
}

impl resident_inventory::ResidentOwner for LaneReceiptNamespaceDurability {
    const FAMILY: resource_inventory::Family = resource_inventory::Family::ResidentFrontier;

    fn resident_associations(&self) -> std::result::Result<u64, resource_inventory::Unavailable> {
        resident_inventory::lengths([self.slots.iter().flatten().count(), self.directory_count()])
    }

    fn resident_complete(&self) -> bool {
        self.directory_count() <= LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY
            && self.heap_bytes() <= LANE_RECEIPT_DURABILITY_HEAP_BYTES
    }
}

impl Kura {
    /// Discard only receipt directory custody for matched current-reader tests.
    #[cfg(test)]
    pub(crate) fn clear_receipt_namespace_durability_for_tests(&self) {
        *self.lane_receipt_namespace_durability.lock() = LaneReceiptNamespaceDurability::default();
    }

    /// Set a receipt-only directory fault and return its previous pending state.
    #[cfg(test)]
    pub(crate) fn receipt_namespace_directory_failure_for_tests(pending: bool) -> bool {
        FAIL_NEXT_RECEIPT_NAMESPACE_DIRECTORY_SYNC.with(|fault| fault.replace(pending))
    }

    /// Acquire the original sidecar mutex using its private paired read permit.
    /// Only audited immutable readers call this; recovery retains ordinary
    /// acquisition, which invalidates every earlier namespace observation.
    fn lock_consensus_sidecar_read(&self) -> Result<PublicationGuard<'_>> {
        self.sidecar_lock
            .lock_read_only(&self.sidecar_read_permit)
            .map_err(|_| {
                Self::invalid_lane_artifact_error(
                    self.store_root.clone(),
                    "sidecar read permit belongs to a foreign storage mutex",
                )
            })
    }

    // The snapshots are from before the successful flush. Replacing them with
    // post-flush metadata could incorrectly bless a subsequent namespace write.
    fn receipt_namespace_generations_unchanged(namespace: &BoundProgressNamespace) -> bool {
        namespace.directories.iter().all(|directory| {
            secure_file_metadata::from_file(&directory.file).is_ok_and(|current| {
                current.is_dir()
                    && Self::sidecar_directory_metadata_unchanged(&directory.metadata, &current)
            })
        })
    }

    fn receipt_namespace_durability_matches(
        &self,
        retained: &BoundProgressNamespace,
        current: &BoundProgressNamespace,
    ) -> bool {
        retained.data_path == current.data_path
            && retained.index_path == current.index_path
            && retained.directories.len() == current.directories.len()
            && retained
                .directories
                .iter()
                .zip(&current.directories)
                .all(|(old, new)| {
                    old.expected_path == new.expected_path
                        && old.canonical_path == new.canonical_path
                        && old.entry_name == new.entry_name
                        && Self::sidecar_directory_metadata_unchanged(&old.metadata, &new.metadata)
                })
            && Self::receipt_namespace_generations_unchanged(retained)
            && self.bound_progress_namespace_unchanged(retained)
            && self.bound_progress_namespace_unchanged(current)
            && Self::receipt_namespace_generations_unchanged(retained)
    }

    // Called under the existing prune -> canonical -> geometry -> sidecar
    // guards. The durability-owner lock is innermost and never escapes this
    // read. Every call syncs the freshly read data/index files and checks their
    // exact identities; only unchanged directory generations reuse a flush.
    fn sync_lane_receipt_with_namespace_custody(
        &self,
        bound: BoundProgressSidecar,
        sidecar: &PublicationGuard<'_>,
    ) -> bool {
        if self.emergency_fast_startup_enabled() {
            return false;
        }
        if let Err(err) = sync_indexed_sidecar_data(&bound.data) {
            iroha_logger::warn!(?err, path = ?bound.namespace.data_path, "failed to sync lane receipt payload");
            return false;
        }
        if let Err(err) = sync_indexed_sidecar_index(&bound.index) {
            iroha_logger::warn!(?err, path = ?bound.namespace.index_path, "failed to sync lane receipt index");
            return false;
        }
        let mut custody = self.lane_receipt_namespace_durability.lock();
        let epoch = sidecar.mutation_epoch();
        let reused = custody.slots.iter().flatten().any(|retained| {
            epoch == Some(retained.mutation_epoch)
                && self.receipt_namespace_durability_matches(&retained.namespace, &bound.namespace)
        });
        if !reused {
            #[cfg(test)]
            if FAIL_NEXT_RECEIPT_NAMESPACE_DIRECTORY_SYNC.with(|fault| fault.replace(false)) {
                return false;
            }
            if !self.sync_bound_progress_namespace(&bound.namespace, "lane receipt") {
                return false;
            }
        }
        if !self.bound_progress_sidecar_unchanged(&bound) {
            return false;
        }
        // A concurrent ancestor mutation may still satisfy the ordinary
        // namespace binding check. Do not retain or reuse its older barrier.
        if !Self::receipt_namespace_generations_unchanged(&bound.namespace) {
            return false;
        }
        if !reused && let Some(epoch) = epoch {
            custody.retain(bound.namespace, epoch);
        }
        true
    }
}
